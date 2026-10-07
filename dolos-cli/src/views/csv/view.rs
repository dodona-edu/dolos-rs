use crate::views::csv::report_dir::ReportDirectory;
use crate::views::csv::rows::{FileRow, FragmentRow, MetadataRow, PairRow};
use crate::views::view::View;
use dolos::Report;
use serde::Serialize;
use std::io::{Error, Result};
use std::path::{Path, PathBuf};

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
    /// when the report holds fragments and at least one pair has fragments.
    pub fn write(&self, report: &Report) -> Result<ReportDirectory> {
        let dir = ReportDirectory::create(self.destination.clone(), &report.metadata)?;

        let properties = report.metadata.properties();
        let metadata = properties
            .iter()
            .map(|(property, value)| MetadataRow::new(property, value));
        write_table(&dir.file("metadata.csv"), metadata)?;

        let files = report.files.iter().map(|file| FileRow::new(file));
        write_table(&dir.file("files.csv"), files)?;

        let pairs = report.pairs.iter().map(PairRow::new);
        write_table(&dir.file("pairs.csv"), pairs)?;

        if report.metadata.include_fragments {
            let fragments = report.pairs.iter().flat_map(|pair| {
                pair.fragments
                    .iter()
                    .flatten()
                    .map(move |fragment| FragmentRow::new(pair, fragment))
            });

            write_table(&dir.file("fragments.csv"), fragments)?;
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

/// Write `rows` to `path` as a CSV file. The header row holds the field names
/// of `R`. Writes no file when there are no rows.
fn write_table<R: Serialize>(path: &Path, rows: impl IntoIterator<Item = R>) -> Result<()> {
    let mut rows = rows.into_iter().peekable();
    if rows.peek().is_none() {
        return Ok(());
    }
    let mut writer = csv::Writer::from_path(path).map_err(Error::other)?;
    for row in rows {
        writer.serialize(row).map_err(Error::other)?;
    }
    writer.flush()
}
