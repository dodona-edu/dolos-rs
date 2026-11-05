use crate::file::FileSet;
use std::io::{Error, ErrorKind, Result};
use std::path::PathBuf;
use tempfile::TempDir;
use tree_sitter_grammars::Language;

use super::Source;

/// The template files in one language, whose fingerprints the analysis ignores.
pub struct Template {
    pub file_set: FileSet,
    _temp_dir: Option<TempDir>,
}

impl Template {
    /// Read the template from `paths`, in the forms that [`Source::read`]
    /// accepts, and keep only the files of `language`. Without paths, the
    /// template is empty.
    ///
    /// # Errors
    /// Fails when the paths cannot be read or no file of `language` remains.
    pub fn create(paths: Vec<PathBuf>, language: Language) -> Result<Self> {
        if paths.is_empty() {
            return Ok(Self {
                file_set: FileSet::new(PathBuf::new(), Vec::new()),
                _temp_dir: None,
            });
        }

        let Source { mut file_set, temp_dir, .. } = Source::read(paths)
            .map_err(|e| Error::new(e.kind(), format!("Could not read the template: {e}")))?;

        file_set.retain_language(language);
        if file_set.relative_paths.is_empty() {
            return Err(Error::new(
                ErrorKind::InvalidInput,
                format!("The template has no {language:?} files."),
            ));
        }

        Ok(Self { file_set, _temp_dir: temp_dir })
    }
}
