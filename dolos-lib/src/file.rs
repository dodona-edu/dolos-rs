use crate::winnowing::fingerprints::Fingerprint;
use crate::winnowing::region::Region;
use chrono::{DateTime, FixedOffset};
use std::fmt;
use std::hash::{Hash, Hasher};
use std::ops::Range;
use std::path::PathBuf;

/// One input file of an analysis.
#[derive(Clone)]
pub struct File {
    /// Zero-based index assigned in the order the file was added to the analysis.
    pub id: usize,
    /// The path of the file, relative to the input directory, the directory of
    /// the CSV file or the extracted archive. A path that does not start with
    /// that directory stays as it is, for example an absolute path in the
    /// `filename` column of a CSV file. For separate input files, the path as
    /// given.
    pub relative_path: PathBuf,
    /// Full source text of the file.
    pub content: String,
    /// The data of the `info.csv` row of the file.
    pub info: FileInfo,
    /// The data needed to rerun the analysis for this file. `Some` only when
    /// [`include_analysis_data`](crate::DolosConfigBuilder::include_analysis_data)
    /// is set.
    pub analysis_data: Option<AnalysisData>,
}

/// The data of the `info.csv` row of a file.
///
/// A value is `None`, and `labels` is empty, when the input has no `info.csv`,
/// when the row has no value or when the `info.csv` has no such column.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct FileInfo {
    /// The value of the `full_name` column.
    pub full_name: Option<String>,
    /// The labels in the `labels` column, separated by `;`, in their order in the column.
    pub labels: Vec<String>,
    /// The value of the `created_at` column. `None` also when the value is
    /// not a timestamp in RFC 3339 or in the format `2019-07-23 17:12:33 +0200`.
    pub created_at: Option<DateTime<FixedOffset>>,
}

/// Everything the analysis needs for one file. With it, a pair can be analysed
/// again from the report alone.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AnalysisData {
    /// The file's winnowed fingerprint sequence.
    pub fingerprints: Vec<Fingerprint>,
    /// The source region of each fingerprint (same length as `fingerprints`).
    pub regions: Vec<Region>,
    /// The ignored fingerprint positions, as half-open intervals.
    pub ignored: Vec<Range<usize>>,
}

impl Hash for File {
    fn hash<H>(&self, state: &mut H)
    where
        H: Hasher,
    {
        self.relative_path.hash(state);
    }
}

/// Compares the [`relative_path`](File::relative_path) of the files.
impl PartialEq for File {
    fn eq(&self, other: &Self) -> bool {
        self.relative_path == other.relative_path
    }
}

impl Eq for File {}

impl PartialOrd for File {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

/// Orders the files by their [`relative_path`](File::relative_path).
impl Ord for File {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        self.relative_path.cmp(&other.relative_path)
    }
}

impl fmt::Debug for File {
    fn fmt(&self, fmt: &mut fmt::Formatter) -> fmt::Result {
        fmt.debug_struct("File")
            .field("path", &self.relative_path)
            .finish()
    }
}
