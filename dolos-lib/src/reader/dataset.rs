use crate::file::{File, FileSet, language_name};
use std::io::{Error, ErrorKind, Result};
use std::path::{Path, PathBuf};
use tree_sitter_grammars::Language;

use super::{archive, resolve};

/// The number of skipped paths that a warning names.
const SHOWN_PATHS: usize = 3;

/// A named collection of read source files in one language, ready for analysis.
pub struct Dataset {
    pub name: String,
    /// The files in input order. Their paths are relative to the input
    /// directory, the directory of the CSV file or the extracted archive.
    pub files: Vec<File>,
    pub language: Language,
}

impl Dataset {
    /// Create a `Dataset` from the given input paths. Accepted inputs:
    ///
    /// - **Multiple paths** → treated as individual files.
    /// - **One directory** → files collected recursively; `info.csv` is honored
    ///   when present at the top level.
    /// - **One CSV file** → file list read from the `filename` column.
    /// - **One archive** (`.zip`, `.tar`, `.tar.gz`, `.tgz`, `.tar.bz2`, `.tbz`,
    ///   `.tbz2`) → extracted to a temporary directory, then treated as a directory.
    ///
    /// Only the files of `language` are kept. When `language` is `None`, the
    /// language that most file extensions belong to is used. A file that cannot
    /// be read as UTF-8 text is skipped with a warning.
    pub fn create(paths: Vec<PathBuf>, language: Option<Language>) -> Result<Self> {
        match paths.as_slice() {
            [] => Err(Error::new(ErrorKind::InvalidInput, "no input paths given")),
            [path] => {
                log::info!(step = "read"; "reading '{}'", path.display());
                Self::from_single_path(path, language)
            }
            _ => {
                log::info!(step = "read"; "reading {} files", paths.len());
                Self::from_multi_path(paths, language)
            }
        }
    }

    fn from_multi_path(paths: Vec<PathBuf>, language: Option<Language>) -> Result<Self> {
        let files = resolve::from_files(paths)?;
        Self::new(None, FileSet::new(PathBuf::new(), files), language)
    }

    fn from_single_path(path: &Path, language: Option<Language>) -> Result<Self> {
        // directory
        if path.is_dir() {
            let (name, files) = resolve::from_directory(path)?;
            return Self::new(Some(name), FileSet::new(path, files), language);
        }

        // CSV file
        if path.extension().is_some_and(|ext| ext == "csv") {
            let (name, files) = resolve::from_csv(path)?;
            let base_dir = path.parent().unwrap_or(Path::new("."));
            return Self::new(Some(name), FileSet::new(base_dir, files), language);
        }

        // archive: the files are read before the extracted directory is removed
        if let Some(extracted) = archive::try_extract(path)? {
            let dir = extracted.temp_dir.path();
            let (_, files) = resolve::from_directory(dir)?;
            return Self::new(Some(extracted.name), FileSet::new(dir, files), language);
        }

        Err(Error::new(
            ErrorKind::InvalidInput,
            "a single path must be a directory, a CSV file or a supported archive \
                 (.zip, .tar, .tar.gz, .tgz, .tar.bz2, .tbz, .tbz2)",
        ))
    }

    /// Select the language, keep only the files of that language and read them.
    /// Without a `name`, the dataset is named after the files that remain.
    ///
    /// # Errors
    /// Fails when the language cannot be detected or fewer than 2 files remain.
    fn new(
        name: Option<String>,
        mut file_set: FileSet,
        language: Option<Language>,
    ) -> Result<Self> {
        let language = match language {
            Some(language) => language,
            None => file_set.detect_language()?,
        };

        let other_language = file_set.retain_language(language);
        if !other_language.is_empty() {
            log::warn!(
                "skipped the files that are not {}: {}",
                language_name(language),
                list_paths(&other_language)
            );
        }

        let files = read_files(&file_set);
        if files.len() < 2 {
            return Err(Error::new(
                ErrorKind::InvalidInput,
                format!(
                    "need at least 2 {} files to compare",
                    language_name(language)
                ),
            ));
        }

        let name = name.unwrap_or_else(|| {
            let paths: Vec<PathBuf> = files.iter().map(|f| f.relative_path.clone()).collect();
            resolve::files_name(&paths)
        });
        Ok(Self { name, files, language })
    }
}

/// Read every file of `file_set`. Skip a file that cannot be read as UTF-8 text, with a warning.
fn read_files(file_set: &FileSet) -> Vec<File> {
    let mut files = Vec::new();
    for relative_path in &file_set.relative_paths {
        let path = file_set.base_dir.join(relative_path);
        match std::fs::read_to_string(&path) {
            Ok(content) => files.push(File {
                id: files.len(),
                relative_path: relative_path.clone(),
                content,
                analysis_data: None,
            }),
            Err(e) => log::warn!("skipped '{}': {e}", path.display()),
        }
    }
    files
}

/// List the first [`SHOWN_PATHS`] paths, such as `a.md, b.txt, c.csv and 2 more`.
fn list_paths(paths: &[PathBuf]) -> String {
    let shown: Vec<String> = paths
        .iter()
        .take(SHOWN_PATHS)
        .map(|path| path.display().to_string())
        .collect();
    match paths.len().saturating_sub(SHOWN_PATHS) {
        0 => shown.join(", "),
        more => format!("{} and {more} more", shown.join(", ")),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rstest::rstest;

    fn relative_paths(dataset: &Dataset) -> Vec<PathBuf> {
        dataset
            .files
            .iter()
            .map(|f| f.relative_path.clone())
            .collect()
    }

    #[test]
    fn create_rejects_empty_input() {
        assert!(Dataset::create(vec![], None).is_err());
    }

    #[test]
    fn create_rejects_a_single_regular_file() {
        assert!(Dataset::create(vec!["fixtures/sample1.js".into()], None).is_err());
    }

    #[test]
    fn create_reads_loose_files() {
        let dataset = Dataset::create(
            vec!["fixtures/sample1.js".into(), "fixtures/sample2.js".into()],
            None,
        )
        .unwrap();
        assert_eq!(
            relative_paths(&dataset),
            vec![
                PathBuf::from("fixtures/sample1.js"),
                PathBuf::from("fixtures/sample2.js"),
            ]
        );
    }

    #[test]
    fn create_names_many_files_by_count() {
        let dataset = Dataset::create(
            vec![
                "fixtures/sample1.js".into(),
                "fixtures/sample2.js".into(),
                "fixtures/sample3.js".into(),
            ],
            None,
        )
        .unwrap();
        assert_eq!(dataset.name, "3 files");
        assert_eq!(dataset.files.len(), 3);
    }

    #[test]
    fn create_names_the_files_left_after_filtering() {
        let dataset = Dataset::create(
            vec![
                "fixtures/sample1.js".into(),
                "fixtures/sample2.js".into(),
                "fixtures/sample1.py".into(),
            ],
            None,
        )
        .unwrap();
        assert_eq!(dataset.name, "sample1.js-sample2.js");
    }

    #[test]
    fn create_reads_a_directory() {
        let dataset = Dataset::create(vec!["fixtures/reader".into()], None).unwrap();
        assert_eq!(dataset.name, "reader");
        assert_eq!(
            relative_paths(&dataset),
            vec![PathBuf::from("sample1.js"), PathBuf::from("sample2.js"),]
        );
    }

    #[test]
    fn create_reads_a_csv_file() {
        let dataset = Dataset::create(vec!["fixtures/reader/info.csv".into()], None).unwrap();
        assert_eq!(dataset.name, "reader");
        assert_eq!(
            relative_paths(&dataset),
            vec![PathBuf::from("sample1.js"), PathBuf::from("sample2.js"),]
        );
    }

    #[rstest]
    #[case::zip("fixtures/reader.zip")]
    #[case::tar("fixtures/reader.tar")]
    #[case::tar_gz("fixtures/reader.tar.gz")]
    #[case::tar_bz2("fixtures/reader.tar.bz2")]
    fn create_reads_an_archive(#[case] path: &str) {
        let dataset = Dataset::create(vec![path.into()], None).unwrap();
        assert_eq!(dataset.name, "reader");

        let mut files = relative_paths(&dataset);
        files.sort();
        assert_eq!(
            files,
            vec![PathBuf::from("sample1.js"), PathBuf::from("sample2.js")]
        );
    }

    #[test]
    fn create_skips_a_file_that_is_not_utf8() {
        let dir = tempfile::TempDir::new().unwrap();
        std::fs::copy("fixtures/sample1.js", dir.path().join("a.js")).unwrap();
        std::fs::copy("fixtures/sample2.js", dir.path().join("b.js")).unwrap();
        std::fs::write(dir.path().join("c.js"), b"let x = \"\xff\";").unwrap();

        let dataset = Dataset::create(vec![dir.path().into()], None).unwrap();

        let mut files = relative_paths(&dataset);
        files.sort();
        assert_eq!(files, vec![PathBuf::from("a.js"), PathBuf::from("b.js")]);
    }

    #[rstest]
    #[case::few(&["a.md", "b.txt"], "a.md, b.txt")]
    #[case::many(&["a.md", "b.txt", "c.csv", "d", "e"], "a.md, b.txt, c.csv and 2 more")]
    fn list_paths_names_the_first_paths(#[case] paths: &[&str], #[case] expected: &str) {
        let paths: Vec<PathBuf> = paths.iter().map(PathBuf::from).collect();
        assert_eq!(list_paths(&paths), expected);
    }
}
