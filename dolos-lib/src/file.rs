use crate::winnowing::fingerprints::Fingerprint;
use crate::winnowing::region::Region;
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
    /// The data needed to rerun the analysis for this file. `Some` only when
    /// [`include_analysis_data`](crate::DolosConfigBuilder::include_analysis_data)
    /// is set.
    pub analysis_data: Option<AnalysisData>,
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
        Self { base_dir, relative_paths }
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
                "could not detect the language: no file has a known extension; specify the language",
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
            _ => {
                let names: Vec<&str> = winners.into_iter().map(Language::name).collect();
                let files = if max == 1 { "file" } else { "files" };
                Err(Error::new(
                    ErrorKind::InvalidInput,
                    format!(
                        "could not detect the language: {} each match {max} {files}; specify the language",
                        names.join(" and ")
                    ),
                ))
            }
        }
    }

    /// Keep only the files whose extension belongs to `language`, and return the other files.
    pub fn retain_language(&mut self, language: Language) -> Vec<PathBuf> {
        let (kept, removed) = std::mem::take(&mut self.relative_paths)
            .into_iter()
            .partition(|path| language.matches(path));
        self.relative_paths = kept;
        removed
    }

    /// Keep the files of `language` and read them, in order.
    ///
    /// Skip, with a warning, the files of another language and the files that
    /// cannot be read. `noun` names the files in the warnings, such as
    /// `template file`.
    pub fn read_files(mut self, language: Language, noun: &str) -> Vec<File> {
        let other_language = self.retain_language(language);
        if !other_language.is_empty() {
            tracing::warn!(
                "skipped {noun}s that are not {}: {}",
                language.name(),
                list_paths(&other_language)
            );
        }

        let mut files = Vec::new();
        for relative_path in self.relative_paths {
            match std::fs::read_to_string(self.base_dir.join(&relative_path)) {
                Ok(content) => files.push(File {
                    id: files.len(),
                    relative_path,
                    content,
                    analysis_data: None,
                }),
                Err(e) => tracing::warn!("skipped {noun} '{}': {e}", relative_path.display()),
            }
        }
        files
    }
}

/// The number of paths that [`list_paths`] names.
const LISTED_PATHS: usize = 3;

/// List the first paths for a message, such as `a.md, b.txt, c.csv and 2 more`.
fn list_paths(paths: &[PathBuf]) -> String {
    let listed: Vec<String> = paths
        .iter()
        .take(LISTED_PATHS)
        .map(|path| path.display().to_string())
        .collect();
    match paths.len().saturating_sub(LISTED_PATHS) {
        0 => listed.join(", "),
        more => format!("{} and {more} more", listed.join(", ")),
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
        assert!(err.to_string().contains("C and C++ each match"), "{err}");
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
        let removed = files.retain_language(Language::Cpp);
        assert_eq!(
            files.relative_paths,
            [PathBuf::from("a.cpp"), PathBuf::from("b.h")]
        );
        assert_eq!(
            removed,
            [
                PathBuf::from("README.md"),
                PathBuf::from(".gitignore"),
                PathBuf::from("Makefile")
            ]
        );
    }

    #[rstest]
    #[case::few(&["a.md", "b.txt"], "a.md, b.txt")]
    #[case::many(&["a.md", "b.txt", "c.csv", "d", "e"], "a.md, b.txt, c.csv and 2 more")]
    fn list_paths_names_the_first_paths(#[case] paths: &[&str], #[case] expected: &str) {
        let paths: Vec<PathBuf> = paths.iter().map(PathBuf::from).collect();
        assert_eq!(list_paths(&paths), expected);
    }
}
