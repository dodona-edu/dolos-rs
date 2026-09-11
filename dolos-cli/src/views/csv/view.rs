use crate::views::csv::report_dir::ReportDirectory;
use crate::views::csv::table::{Column, write_table};
use crate::views::view::View;
use dolos::{AnalysisData, File, Fragment, Pair, Point, Region, Report};
use std::io::Result;
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
/// The last three are always present. They hold the data needed to run the
/// analysis again without the source files, and stay empty unless
/// `--include-analysis-data` was given.
fn file_columns() -> [Column<Rc<File>>; 6] {
    [
        Column::new("id", |f| f.id.to_string()),
        Column::new("path", |f| f.relative_path.display().to_string()),
        Column::new("content", |f| f.content.clone()),
        Column::new("fingerprints", |f| {
            analysis_data(f, |data| {
                json_array(data.fingerprints.iter().map(usize::to_string))
            })
        }),
        Column::new("fingerprint_regions", |f| {
            analysis_data(f, |data| {
                json_array(data.regions.iter().flat_map(quad).map(|n| n.to_string()))
            })
        }),
        Column::new("ignored_intervals", |f| {
            analysis_data(f, |data| {
                json_array(
                    data.ignored
                        .iter()
                        .map(|r| format!("[{},{}]", r.start, r.len())),
                )
            })
        }),
    ]
}

/// Render one analysis-data cell, or an empty cell when the file carries none.
fn analysis_data(file: &File, render: impl Fn(&AnalysisData) -> String) -> String {
    file.analysis_data.as_ref().map(render).unwrap_or_default()
}

/// A region as the four numbers `start row, start column, end row, end column`.
fn quad(region: &Region) -> [usize; 4] {
    [
        region.start_point.row,
        region.start_point.column,
        region.end_point.row,
        region.end_point.column,
    ]
}

/// Format `items` as a JSON array: `[1,2,3]` or `[[1,2],[3,4]]`.
fn json_array(items: impl IntoIterator<Item = String>) -> String {
    format!("[{}]", items.into_iter().collect::<Vec<_>>().join(","))
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
