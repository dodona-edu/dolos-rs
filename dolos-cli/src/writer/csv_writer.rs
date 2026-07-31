use crate::writer::output::OutputWriter;
use dolos::File as SourceFile;
use dolos::Metadata;
use dolos::Pair;
use dolos::Report;
use std::fs::File;
use std::io::{Error, ErrorKind, Result};
use std::path::PathBuf;
use std::rc::Rc;

const METADATA_HEADER: &[&str] = &["property", "value"];
const FILES_HEADER: &[&str] = &["id", "path", "content"];
/// Extra `files.csv` columns, present only when fingerprints are exported.
const FINGERPRINTS_COLUMNS: &[&str] = &["fingerprints", "fingerprint_regions"];
const FRAGMENTS_HEADER: &[&str] = &[
    "file1_id",
    "file1_path",
    "file1_start_point",
    "file1_end_point",
    "file2_id",
    "file2_path",
    "file2_start_point",
    "file2_end_point",
    "fingerprint_count",
    "ignored",
];
const PAIRS_HEADER: &[&str] = &[
    "file1_id",
    "file1_path",
    "file2_id",
    "file2_path",
    "similarity",
    "longest",
    "totalLeft",
    "totalRight",
    "overlapLeft",
    "overlapRight",
];

/// CSV writer that outputs similarity results to a CSV file.
pub struct CsvWriter {
    similarities_writer: csv::Writer<File>,
    /// Present only when `include_fragments` is set; streams rows into `fragments.csv`.
    fragments_writer: Option<csv::Writer<File>>,
}

impl CsvWriter {
    /// Create a new CSV writer.
    ///
    /// The report's CSV files are written directly into `output_destination`
    /// when given, or into an auto-named `dolos-report-{time}-{reportName}/`
    /// directory in the current directory when it is `None`. The directory must
    /// not yet exist. Writes `metadata.csv` and `files.csv` in one shot
    /// each, and opens a streamed `pairs.csv` (and optionally `fragments.csv`
    /// when `metadata.include_fragments` is set) for the pair rows.
    pub(super) fn new(output_destination: Option<PathBuf>, report: &Report) -> Result<Self> {
        let metadata = &report.metadata;
        let report_dir =
            output_destination.unwrap_or_else(|| PathBuf::from(report_dir_name(metadata)));
        if report_dir.exists() {
            return Err(Error::new(
                ErrorKind::AlreadyExists,
                format!(
                    "Directory {} already exists. Please specify a different output destination.",
                    report_dir.display()
                ),
            ));
        }
        std::fs::create_dir_all(&report_dir)?;

        // metadata.csv — fully known up front, written in one shot.
        let mut meta =
            csv::Writer::from_path(report_dir.join("metadata.csv")).map_err(Error::other)?;
        write_metadata(&mut meta, metadata)?;
        meta.flush().map_err(Error::other)?;

        // files.csv — fully known up front, written in one shot.
        let mut files_writer =
            csv::Writer::from_path(report_dir.join("files.csv")).map_err(Error::other)?;
        write_files(&mut files_writer, &report.files, metadata.include_core_data)?;
        files_writer.flush().map_err(Error::other)?;

        // fragments.csv — streamed per pair via write_pair; omitted when fragments are not stored.
        let fragments_writer = if metadata.include_fragments {
            let mut w =
                csv::Writer::from_path(report_dir.join("fragments.csv")).map_err(Error::other)?;
            w.write_record(FRAGMENTS_HEADER).map_err(Error::other)?;
            Some(w)
        } else {
            None
        };

        // pairs.csv — streamed per pair via write_pair.
        let mut similarities_writer =
            csv::Writer::from_path(report_dir.join("pairs.csv")).map_err(Error::other)?;
        similarities_writer
            .write_record(PAIRS_HEADER)
            .map_err(Error::other)?;

        Ok(Self { similarities_writer, fragments_writer })
    }
}

impl OutputWriter for CsvWriter {
    fn write_pair(&mut self, pair: &Pair) -> Result<()> {
        let m = &pair.metrics;
        self.similarities_writer
            .serialize((
                pair.left_file.id.to_string(),
                pair.left_file.relative_path.display().to_string(),
                pair.right_file.id.to_string(),
                pair.right_file.relative_path.display().to_string(),
                m.similarity,
                m.longest_fragment,
                m.total_left,
                m.total_right,
                m.overlap_left,
                m.overlap_right,
            ))
            .map_err(Error::other)?;

        if let Some(writer) = self.fragments_writer.as_mut() {
            for fragment in pair.fragments.as_ref().unwrap() {
                let left_start = &fragment.left_region.start_point;
                let left_end = &fragment.left_region.end_point;
                let right_start = &fragment.right_region.start_point;
                let right_end = &fragment.right_region.end_point;
                writer
                    .write_record([
                        pair.left_file.id.to_string(),
                        pair.left_file.relative_path.display().to_string(),
                        format!("{}:{}", left_start.row, left_start.column),
                        format!("{}:{}", left_end.row, left_end.column),
                        pair.right_file.id.to_string(),
                        pair.right_file.relative_path.display().to_string(),
                        format!("{}:{}", right_start.row, right_start.column),
                        format!("{}:{}", right_end.row, right_end.column),
                        fragment.fingerprint_count.to_string(),
                        fragment.ignored.to_string(),
                    ])
                    .map_err(Error::other)?;
            }
        }
        Ok(())
    }

    fn finish(mut self) -> Result<()> {
        self.similarities_writer.flush().map_err(Error::other)?;
        if let Some(mut w) = self.fragments_writer {
            w.flush().map_err(Error::other)?;
        }
        Ok(())
    }
}

fn report_dir_name(metadata: &Metadata) -> String {
    format!(
        "dolos-report-{}-{}",
        metadata.created_at.format("%Y%m%dT%H%M%S%3fZ"),
        sanitize_name(&metadata.report_name),
    )
}

/// Sanitize a report name for use in a directory name: spaces become dashes and
/// any character that is not ASCII alphanumeric or `-` is dropped.
fn sanitize_name(name: &str) -> String {
    name.chars()
        .map(|c| if c == ' ' { '-' } else { c })
        .filter(|c| c.is_ascii_alphanumeric() || *c == '-')
        .collect()
}

fn write_metadata(
    writer: &mut csv::Writer<impl std::io::Write>,
    metadata: &Metadata,
) -> Result<()> {
    writer.write_record(METADATA_HEADER).map_err(Error::other)?;
    for (property, value) in &metadata.properties() {
        writer
            .write_record([*property, value])
            .map_err(Error::other)?;
    }
    Ok(())
}

/// Write `files.csv`. The fingerprint-export columns are appended only when
/// `include_core_data` is set, so default reports keep the original layout.
fn write_files(
    writer: &mut csv::Writer<impl std::io::Write>,
    files: &[Rc<SourceFile>],
    include_core_data: bool,
) -> Result<()> {
    let mut header = FILES_HEADER.to_vec();
    if include_core_data {
        header.extend_from_slice(FINGERPRINTS_COLUMNS);
    }
    writer.write_record(&header).map_err(Error::other)?;

    // Fields are streamed with `write_field` so the (potentially large) file
    // content is written by reference instead of cloned.
    for file in files {
        writer
            .write_field(file.id.to_string())
            .map_err(Error::other)?;
        writer
            .write_field(file.relative_path.display().to_string())
            .map_err(Error::other)?;
        writer.write_field(&file.content).map_err(Error::other)?;

        if include_core_data {
            let fingerprints = file
                .fingerprints
                .as_ref()
                .expect("fingerprints are kept when include_core_data is set");
            writer
                .write_field(serde_json::to_string(fingerprints).expect("fingerprints serialize"))
                .map_err(Error::other)?;
            let regions = file
                .regions
                .as_ref()
                .expect("regions are kept when include_core_data is set");
            writer
                .write_field(serde_json::to_string(regions).expect("regions serialize"))
                .map_err(Error::other)?;
        }
        // Terminate the record (csv requires an explicit empty record after
        // a sequence of `write_field` calls).
        writer.write_record(None::<&[u8]>).map_err(Error::other)?;
    }
    Ok(())
}
