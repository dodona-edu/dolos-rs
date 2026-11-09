use crate::error::context;
use crate::file::read_text;
use std::io::{Error, ErrorKind, Result};
use std::path::PathBuf;
use tree_sitter_grammars::Language;

use super::{Input, list_paths};

/// The read template files in one language, whose fingerprints the analysis ignores.
pub struct Template {
    /// The contents of the template files.
    pub contents: Vec<String>,
}

impl Template {
    /// Read the template from `paths`, in the forms that [`Input::read`]
    /// accepts, and keep only the files of `language`. Without paths, the
    /// template is empty.
    ///
    /// # Errors
    /// Fails when the paths or a template file cannot be read, or no file of
    /// `language` remains.
    pub fn create(paths: Vec<PathBuf>, language: Language) -> Result<Self> {
        if paths.is_empty() {
            return Ok(Self { contents: Vec::new() });
        }

        // `_temp_dir` keeps an extracted archive until its files are read.
        let Input { mut file_set, temp_dir: _temp_dir, .. } =
            Input::read(paths).map_err(|e| context(e, "could not read the template"))?;

        let other_language = file_set.retain_language(language);
        if !other_language.is_empty() {
            log::warn!(
                "skipped the template files that are not {}: {}",
                language.name(),
                list_paths(&other_language)
            );
        }
        if file_set.relative_paths.is_empty() {
            return Err(Error::new(
                ErrorKind::InvalidInput,
                format!("the template has no {} files", language.name()),
            ));
        }

        let contents = file_set
            .relative_paths
            .iter()
            .map(|relative| {
                read_text(&file_set.base_dir.join(relative)).map_err(|e| {
                    let message =
                        format!("could not read the template file '{}'", relative.display());
                    context(e, message)
                })
            })
            .collect::<Result<_>>()?;
        Ok(Self { contents })
    }
}
