use crate::winnowing::fingerprints::Fingerprint;
use crate::winnowing::region::Region;
use chrono::{DateTime, FixedOffset};
use std::collections::HashMap;
use std::fmt;
use std::hash::{Hash, Hasher};
use std::io::{Error, ErrorKind, Result};
use std::ops::Range;
use std::path::PathBuf;
use tree_sitter_grammars::Language;

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
/// A value is `None` when the input has no `info.csv`, when the row has no
/// value or when the `info.csv` has no such column.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct FileInfo {
    /// The value of the `full_name` column.
    pub full_name: Option<String>,
    /// The value of the `labels` column, or of the `label` column when
    /// `labels` has no value.
    pub labels: Option<String>,
    /// The value of the `created_at` column. `None` also when the value is
    /// not a timestamp in RFC 3339 or in the format `2019-07-23 17:12:33 +0200`.
    pub created_at: Option<DateTime<FixedOffset>>,
}

/// What one row of an `info.csv` file says about its file.
#[derive(Debug, Clone, Default)]
pub struct InfoRow {
    pub info: FileInfo,
    /// Whether the `ignored` column holds `true`.
    pub ignored: bool,
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

/// A set of source files to analyze: a base directory and paths relative to it.
/// The relative paths serve as display paths in the output.
#[derive(Debug, Clone)]
pub struct FileSet {
    pub base_dir: PathBuf,
    pub relative_paths: Vec<PathBuf>,
    /// The `info.csv` rows of the files, by relative path.
    pub info: HashMap<PathBuf, InfoRow>,
}

impl FileSet {
    /// Create a `FileSet` from a base directory and a list of full paths.
    /// Each path is stripped of the `base_dir` prefix to produce the relative
    /// (display) paths stored in `relative_paths`.
    pub fn new(base_dir: impl Into<PathBuf>, files: Vec<PathBuf>) -> Self {
        let base_dir = base_dir.into();
        let relative_paths = files
            .into_iter()
            .map(|p| p.strip_prefix(&base_dir).unwrap_or(&p).to_path_buf())
            .collect();
        Self { base_dir, relative_paths, info: HashMap::new() }
    }

    /// Create a `FileSet` as [`FileSet::new`] does, and keep the `info.csv`
    /// row of each file.
    pub fn with_info(base_dir: impl Into<PathBuf>, files: Vec<(PathBuf, InfoRow)>) -> Self {
        let (paths, rows): (Vec<_>, Vec<_>) = files.into_iter().unzip();
        let mut file_set = Self::new(base_dir, paths);
        file_set.info = file_set.relative_paths.iter().cloned().zip(rows).collect();
        file_set
    }

    /// Remove the files of the `info.csv` rows with `ignored=true` and return
    /// them as a `FileSet` with the same base directory.
    pub fn take_ignored(&mut self) -> FileSet {
        let is_ignored = |path: &PathBuf| self.info.get(path).is_some_and(|row| row.ignored);
        let (ignored, kept) = std::mem::take(&mut self.relative_paths)
            .into_iter()
            .partition(is_ignored);
        self.relative_paths = kept;
        FileSet {
            base_dir: self.base_dir.clone(),
            relative_paths: ignored,
            info: HashMap::new(),
        }
    }

    /// Detect the language that the most file extensions belong to.
    /// [`Language::Char`] counts only when no file belongs to another language.
    ///
    /// # Errors
    /// Fails when no file has a known extension, or when several languages share the highest count.
    pub fn detect_language(&self) -> Result<Language> {
        let mut counts: HashMap<Language, usize> = HashMap::new();
        for path in &self.relative_paths {
            for language in Language::from_path(path) {
                *counts.entry(language).or_default() += 1;
            }
        }
        if counts.len() > 1 {
            counts.remove(&Language::Char);
        }

        let Some(&max) = counts.values().max() else {
            return Err(Error::new(
                ErrorKind::InvalidInput,
                "Could not detect the language: no file has a known extension. Specify the language explicitly.",
            ));
        };

        let mut winners: Vec<Language> = counts
            .into_iter()
            .filter(|&(_, count)| count == max)
            .map(|(language, _)| language)
            .collect();
        winners.sort();
        match winners[..] {
            [language] => Ok(language),
            _ => Err(Error::new(
                ErrorKind::InvalidInput,
                format!(
                    "Could not detect the language: {winners:?} each match {max} files. Specify the language explicitly."
                ),
            )),
        }
    }

    /// Keep only the files whose extension belongs to `language`.
    pub fn retain_language(&mut self, language: Language) {
        self.relative_paths.retain(|path| language.matches(path));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rstest::rstest;

    fn file_set(paths: &[&str]) -> FileSet {
        FileSet::new("", paths.iter().map(PathBuf::from).collect())
    }

    #[rstest]
    #[case::cpp_with_headers(&["a.cpp", "b.cpp", "c.cpp", "d.cpp", "a.h", "b.h", "c.h"], Language::Cpp)]
    #[case::c_with_headers(&["a.c", "b.c", "a.h", "b.h", "c.h"], Language::C)]
    #[case::unknown_majority(&["a.py", "b.py", "x.csv", "y.csv", "z.csv"], Language::Python)]
    #[case::text_only(&["a.txt", "b.md"], Language::Char)]
    #[case::code_before_text(&["a.py", "README.md", "notes.txt", "b.md"], Language::Python)]
    #[case::no_extension(&["Makefile", "a.py"], Language::Python)]
    fn detect_language_picks_majority(#[case] paths: &[&str], #[case] expected: Language) {
        assert_eq!(file_set(paths).detect_language().unwrap(), expected);
    }

    #[test]
    fn detect_language_rejects_a_tie() {
        let err = file_set(&["a.h", "b.h", "c.h", "README.md"])
            .detect_language()
            .unwrap_err();
        assert!(err.to_string().contains("[C, Cpp]"), "{err}");
    }

    #[test]
    fn detect_language_rejects_unknown_extensions() {
        let err = file_set(&["data.csv", "Makefile"])
            .detect_language()
            .unwrap_err();
        assert!(
            err.to_string().contains("no file has a known extension"),
            "{err}"
        );
    }

    #[test]
    fn retain_language_keeps_matching_files() {
        let mut files = file_set(&["a.cpp", "README.md", "b.h", ".gitignore", "Makefile"]);
        files.retain_language(Language::Cpp);
        assert_eq!(
            files.relative_paths,
            [PathBuf::from("a.cpp"), PathBuf::from("b.h")]
        );
    }
}
