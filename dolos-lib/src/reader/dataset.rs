use crate::file::{File, FileSet, read_text};
use std::io::{Error, ErrorKind, Result};
use std::path::PathBuf;
use tree_sitter_grammars::Language;

use super::{Input, list_paths, resolve};

/// A named collection of read source files in one language, ready for analysis.
pub struct Dataset {
    pub name: String,
    /// The files in input order. Their paths are relative to the input
    /// directory, the directory of the CSV file or the extracted archive.
    pub files: Vec<File>,
    pub language: Language,
}

impl Dataset {
    /// Create a `Dataset` from the given input paths, in the forms that
    /// [`Input::read`] accepts.
    ///
    /// Only the files of `language` are kept. When `language` is `None`, the
    /// language that most file extensions belong to is used. A file that cannot
    /// be read as UTF-8 text is skipped with a warning. Without a name from the
    /// input, the dataset is named after the files that remain.
    ///
    /// # Errors
    /// Fails when the paths cannot be read, the language cannot be detected or
    /// fewer than 2 readable files remain.
    pub fn create(paths: Vec<PathBuf>, language: Option<Language>) -> Result<Self> {
        // `_temp_dir` keeps an extracted archive until its files are read.
        let Input { name, mut file_set, temp_dir: _temp_dir } = Input::read(paths)?;

        let language = match language {
            Some(language) => language,
            None => file_set.detect_language()?,
        };

        let other_language = file_set.retain_language(language);
        if !other_language.is_empty() {
            tracing::warn!(
                "skipped the files that are not {}: {}",
                language.name(),
                list_paths(&other_language)
            );
        }

        let files = read_files(&file_set);
        if files.len() < 2 {
            return Err(Error::new(
                ErrorKind::InvalidInput,
                format!("need at least 2 {} files to compare", language.name()),
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
        match read_text(&file_set.base_dir.join(relative_path)) {
            Ok(content) => files.push(File {
                id: files.len(),
                relative_path: relative_path.clone(),
                content,
                analysis_data: None,
            }),
            Err(e) => tracing::warn!("skipped '{}': {e}", relative_path.display()),
        }
    }
    files
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
        let err = Dataset::create(vec!["fixtures/sample1.js".into()], None)
            .err()
            .unwrap();
        assert!(err.to_string().contains("at least 2"), "{err}");
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
}
