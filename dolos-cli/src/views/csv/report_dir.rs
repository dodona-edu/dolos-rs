use anyhow::{Context, Result, bail};
use dolos::Metadata;
use std::path::{Path, PathBuf};
use time::OffsetDateTime;
use time::macros::format_description;

/// A created, empty directory that holds the files of one report.
pub struct ReportDirectory {
    path: PathBuf,
}

impl ReportDirectory {
    /// Create the report directory.
    ///
    /// Uses `destination` when given, and an auto-named
    /// `dolos-report-<timestamp>-<name>` directory in the current directory
    /// otherwise.
    ///
    /// # Errors
    /// Fails when the directory already exists or cannot be created.
    pub fn create(destination: Option<PathBuf>, metadata: &Metadata) -> Result<Self> {
        let path = destination.unwrap_or_else(|| PathBuf::from(default_name(metadata)));
        if path.exists() {
            bail!(
                "directory '{}' already exists; choose another --output-destination",
                path.display()
            );
        }
        std::fs::create_dir_all(&path)
            .with_context(|| format!("could not create directory '{}'", path.display()))?;
        Ok(Self { path })
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    /// The full path of `file_name` inside this directory.
    pub fn file(&self, file_name: &str) -> PathBuf {
        self.path.join(file_name)
    }
}

/// Build the default directory name for a report.
fn default_name(metadata: &Metadata) -> String {
    format!(
        "dolos-report-{}-{}",
        timestamp(metadata.created_at),
        sanitize_name(&metadata.report_name),
    )
}

/// Render `time` as a compact timestamp with milliseconds, for example `20240102T030405678Z`.
fn timestamp(time: OffsetDateTime) -> String {
    let format =
        format_description!("[year][month][day]T[hour][minute][second][subsecond digits:3]Z");
    time.format(format)
        .expect("a UTC time of the current era formats as a timestamp")
}

/// Sanitize a report name for use in a directory name: spaces become dashes and
/// any character that is not ASCII alphanumeric or `-` is dropped.
fn sanitize_name(name: &str) -> String {
    name.chars()
        .map(|c| if c == ' ' { '-' } else { c })
        .filter(|c| c.is_ascii_alphanumeric() || *c == '-')
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use time::macros::datetime;

    #[test]
    fn timestamp_has_millisecond_precision() {
        let time = datetime!(2024-01-02 03:04:05.678_901 UTC);
        assert_eq!(timestamp(time), "20240102T030405678Z");
    }
}
