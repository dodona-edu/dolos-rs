use crate::views::csv::report_dir::ReportDirectory;
use crate::views::csv::table::{Column, write_table};
use crate::views::view::View;
use dolos::{AnalysisData, File, Fragment, Pair, Point, Region, Report};
use std::io::Result;
use std::ops::Range;
use std::path::PathBuf;
use std::rc::Rc;

/// Writes the report as a set of CSV files in one directory.
pub struct CsvView {
    destination: Option<PathBuf>,
}

impl CsvView {
    pub fn new(destination: Option<PathBuf>) -> Self {
        Self { destination }
    }

    /// Write the report and return the directory it was written to.
    ///
    /// Writes `metadata.csv`, `files.csv` and `pairs.csv`, and `fragments.csv`
    /// when the report holds fragments.
    pub fn write(&self, report: &Report) -> Result<ReportDirectory> {
        let dir = ReportDirectory::create(self.destination.clone(), &report.metadata)?;

        let properties = report.metadata.properties();
        write_table(
            &dir.file("metadata.csv"),
            &metadata_columns(),
            properties.iter(),
        )?;
        write_table(&dir.file("files.csv"), &file_columns(), report.files.iter())?;
        write_table(&dir.file("pairs.csv"), &pair_columns(), report.pairs.iter())?;

        if report.metadata.include_fragments {
            write_table(
                &dir.file("fragments.csv"),
                &fragment_columns(),
                fragment_rows(&report.pairs).iter(),
            )?;
        }

        Ok(dir)
    }
}

impl View for CsvView {
    fn show(&self, report: &Report) -> Result<()> {
        self.write(report)?;
        Ok(())
    }
}

// ── Tables ───────────────────────────────────────────────────────────

type MetadataProperty = (&'static str, String);

fn metadata_columns() -> [Column<MetadataProperty>; 2] {
    [
        Column::new("property", |p| p.0.to_string()),
        Column::new("value", |p| p.1.clone()),
    ]
}

/// The columns of `files.csv`.
///
/// The last three are empty unless `--include-analysis-data` was given:
/// - `fingerprints`: one hash per fingerprint, `[hash,...]`.
/// - `fingerprint_regions`: `[start_row,start_col,end_row,end_col]` per fingerprint.
/// - `ignored_intervals`: one half-open `[start,end)` interval per ignored run.
fn file_columns() -> [Column<Rc<File>>; 6] {
    [
        Column::new("id", |file| file.id.to_string()),
        Column::new("path", |file| file.relative_path.display().to_string()),
        Column::new("content", |file| file.content.clone()),
        Column::new("fingerprints", |file| {
            analysis(file, |d| bracketed(&d.fingerprints, usize::to_string))
        }),
        Column::new("fingerprint_regions", |file| {
            analysis(file, |d| bracketed(&d.regions, region_to_string))
        }),
        Column::new("ignored_intervals", |file| {
            analysis(file, |d| bracketed(&d.ignored, range_to_string))
        }),
    ]
}

fn bracketed<T>(items: &[T], fmt: impl Fn(&T) -> String) -> String {
    format!("[{}]", items.iter().map(fmt).collect::<Vec<_>>().join(","))
}

fn analysis(file: &File, fmt: impl Fn(&AnalysisData) -> String) -> String {
    file.analysis_data.as_ref().map(fmt).unwrap_or_default()
}

fn range_to_string(range: &Range<usize>) -> String {
    format!("[{},{}]", range.start, range.end)
}

fn region_to_string(region: &Region) -> String {
    format!(
        "[{},{},{},{}]",
        region.start_point.row,
        region.start_point.column,
        region.end_point.row,
        region.end_point.column,
    )
}

fn pair_columns() -> [Column<Pair>; 10] {
    [
        Column::new("file1_id", |p| p.left_file.id.to_string()),
        Column::new("file1_path", |p| {
            p.left_file.relative_path.display().to_string()
        }),
        Column::new("file2_id", |p| p.right_file.id.to_string()),
        Column::new("file2_path", |p| {
            p.right_file.relative_path.display().to_string()
        }),
        Column::new("similarity", |p| p.metrics.similarity.to_string()),
        Column::new("longest", |p| p.metrics.longest_match.to_string()),
        Column::new("totalLeft", |p| p.metrics.total_left.to_string()),
        Column::new("totalRight", |p| p.metrics.total_right.to_string()),
        Column::new("overlapLeft", |p| p.metrics.overlap_left.to_string()),
        Column::new("overlapRight", |p| p.metrics.overlap_right.to_string()),
    ]
}

/// One row of `fragments.csv`: a fragment together with the pair it belongs to.
struct FragmentRow<'a> {
    pair: &'a Pair,
    fragment: &'a Fragment,
}

fn fragment_columns<'a>() -> [Column<FragmentRow<'a>>; 9] {
    [
        Column::new("file1_id", |r| r.pair.left_file.id.to_string()),
        Column::new("file1_path", |r| {
            r.pair.left_file.relative_path.display().to_string()
        }),
        Column::new("file1_start_point", |r| {
            point(&r.fragment.left_region.start_point)
        }),
        Column::new("file1_end_point", |r| {
            point(&r.fragment.left_region.end_point)
        }),
        Column::new("file2_id", |r| r.pair.right_file.id.to_string()),
        Column::new("file2_path", |r| {
            r.pair.right_file.relative_path.display().to_string()
        }),
        Column::new("file2_start_point", |r| {
            point(&r.fragment.right_region.start_point)
        }),
        Column::new("file2_end_point", |r| {
            point(&r.fragment.right_region.end_point)
        }),
        Column::new("fingerprint_count", |r| {
            r.fragment.fingerprint_count.to_string()
        }),
    ]
}

/// Flatten the fragments of every pair into rows.
fn fragment_rows(pairs: &[Pair]) -> Vec<FragmentRow<'_>> {
    pairs
        .iter()
        .flat_map(|pair| {
            let fragments = pair.fragments.as_deref().unwrap_or_default();
            fragments
                .iter()
                .map(move |fragment| FragmentRow { pair, fragment })
        })
        .collect()
}

fn point(point: &Point) -> String {
    format!("{}:{}", point.row, point.column)
}
