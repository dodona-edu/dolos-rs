use crate::winnowing::fingerprints::Fingerprint;
use crate::winnowing::region::Region;
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
    /// The extra columns of the `info.csv` row of the file. Empty when the file
    /// does not come from an `info.csv`.
    pub info: FileInfo,
    /// The data needed to rerun the analysis for this file. `Some` only when
    /// [`include_analysis_data`](crate::DolosConfigBuilder::include_analysis_data)
    /// is set.
    pub analysis_data: Option<AnalysisData>,
}

/// The columns of the `info.csv` row of a file, other than `filename`, as
/// uninterpreted strings in the column order of the CSV file.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct FileInfo {
    columns: Vec<(String, String)>,
}

impl FileInfo {
    /// Create a `FileInfo` from `(column, value)` pairs, in the given order.
    ///
    /// # Parameters
    ///
    /// - `columns`: the `(column, value)` pairs.
    ///
    /// # Examples
    ///
    /// ```
    /// use dolos::FileInfo;
    ///
    /// let info = FileInfo::new(vec![("full_name".into(), "Ada Lovelace".into())]);
    /// assert_eq!(info.get("full_name"), Some("Ada Lovelace"));
    /// ```
    pub fn new(columns: Vec<(String, String)>) -> Self {
        Self { columns }
    }

    /// Return the value of `column`, or `None` when the row has no such column.
    ///
    /// # Parameters
    ///
    /// - `column`: the column name in the header of the CSV file.
    ///
    /// # Examples
    ///
    /// ```
    /// use dolos::FileInfo;
    ///
    /// let info = FileInfo::new(vec![("labels".into(), "group-1".into())]);
    /// assert_eq!(info.get("labels"), Some("group-1"));
    /// assert_eq!(info.get("full_name"), None);
    /// ```
    pub fn get(&self, column: &str) -> Option<&str> {
        self.iter()
            .find(|(name, _)| *name == column)
            .map(|(_, value)| value)
    }

    /// Return the `(column, value)` pairs in the column order of the CSV file.
    ///
    /// # Examples
    ///
    /// ```
    /// use dolos::FileInfo;
    ///
    /// let info = FileInfo::new(vec![
    ///     ("full_name".into(), "Ada Lovelace".into()),
    ///     ("labels".into(), "group-1".into()),
    /// ]);
    /// let columns: Vec<_> = info.iter().collect();
    /// assert_eq!(columns, [("full_name", "Ada Lovelace"), ("labels", "group-1")]);
    /// ```
    pub fn iter(&self) -> impl Iterator<Item = (&str, &str)> {
        self.columns
            .iter()
            .map(|(column, value)| (column.as_str(), value.as_str()))
    }
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
