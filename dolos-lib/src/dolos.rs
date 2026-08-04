use crate::config::DolosConfig;
use crate::file::{CoreData, File, FileSet};
use crate::fragment::resolve_fragments;
use crate::metadata::Metadata;
use crate::reader::Dataset;
use crate::report::Report;
use crate::winnowing::fingerprints::{Fingerprint, Winnow};
use crate::winnowing::region::Region;
use crate::winnowing::tokenizer::{Tokenizer, Tokens};
use dolos_core::{AnalysisOptions, AnalysisResult};
use std::fmt;
use std::io::{Error, ErrorKind, Result};
use std::path::{Path, PathBuf};
use std::rc::Rc;

pub struct Dolos {
    metadata: Metadata,
    /// Relative display path of each regular file, parallel to `contents`,
    /// `fingerprints`, and (when kept) `locations`.
    paths: Vec<PathBuf>,
    contents: Vec<String>,
    fingerprints: Vec<Vec<Fingerprint>>,
    ignore_fingerprints: Vec<Vec<Fingerprint>>,
    locations: Option<Vec<Vec<Region>>>,
    tokenizer: Tokenizer,
}

impl Dolos {
    /// Create a new `Dolos` analysis from a list of input paths and index arguments.
    ///
    /// Accepted inputs for `files`:
    /// - **Multiple paths** → treated as individual files.
    /// - **One directory** → files collected recursively.
    /// - **One CSV file** → file list read from the `filename` column.
    /// - **One archive** → extracted and treated as a directory.
    pub fn new(files: Vec<PathBuf>, config: DolosConfig) -> Result<Self> {
        let dataset = Dataset::create(files)?;
        let metadata = Metadata::from_config(&config, &dataset);

        let tokenizer = Tokenizer::new(metadata.language);

        let locations = if metadata.include_fragments || metadata.include_core_data {
            Some(Vec::new())
        } else {
            None
        };

        let mut dolos = Dolos {
            metadata,
            paths: Vec::new(),
            contents: Vec::new(),
            fingerprints: Vec::new(),
            ignore_fingerprints: Vec::new(),
            locations,
            tokenizer,
        };

        dolos.add_files(dataset.file_set)?;

        if let Some(ignore_path) = dolos.metadata.ignore.clone() {
            dolos.add_ignore_file(ignore_path)?;
        }

        Ok(dolos)
    }

    /// Read and tokenize a source file, optionally retaining fingerprint locations.
    fn process_file(
        &mut self,
        path: &Path,
        keep_locations: bool,
    ) -> Result<(String, Vec<Fingerprint>, Option<Vec<Region>>)> {
        if self.metadata.language_detected && !self.metadata.language.matches(path) {
            return Err(Error::new(
                ErrorKind::InvalidInput,
                format!("Language does not match file: {}", path.display()),
            ));
        }

        let content = std::fs::read_to_string(path).map_err(|e| {
            Error::new(
                e.kind(),
                format!("Could not read file '{}': {}", path.display(), e),
            )
        })?;

        let (fingerprints, locations) = self
            .tokenizer
            .parse(&content)
            .tokens(self.metadata.include_comments)
            .winnow(
                self.metadata.kgram_length,
                self.metadata.kgrams_in_window,
                keep_locations,
            );

        Ok((content, fingerprints, locations))
    }

    /// Tokenize a template/ignore file and add its fingerprints to the ignore list.
    fn add_ignore_file(&mut self, path: PathBuf) -> Result<()> {
        let (_, fingerprints, _) = self.process_file(&path, false)?;
        self.ignore_fingerprints.push(fingerprints);
        Ok(())
    }

    fn add_files(&mut self, file_set: FileSet) -> Result<()> {
        for relative in &file_set.relative_paths {
            let (content, fingerprints, locations) =
                self.process_file(&file_set.base_dir.join(relative), self.locations.is_some())?;

            self.fingerprints.push(fingerprints);

            if let Some(stored_locations) = self.locations.as_mut() {
                stored_locations
                    .push(locations.expect("locations should be present when they are kept"));
            }

            self.paths.push(relative.to_path_buf());
            self.contents.push(content);
        }

        Ok(())
    }

    /// Run the suffix-tree analysis and build a [`Report`].
    pub fn build_report(self) -> Report {
        let options = AnalysisOptions {
            min_match_length: self.metadata.min_length_match,
            keep_matches: self.metadata.include_fragments,
            max_seq_count: self.metadata.max_fingerprint_file_count,
        };
        let AnalysisResult { metrics, matches } =
            dolos_core::analyze(&self.fingerprints, &self.ignore_fingerprints, &options);

        let fragments = matches
            .zip(self.locations.as_deref())
            .map(|(matches, locations)| {
                resolve_fragments(matches, locations, &self.metadata.fragment_sort_by)
            });

        let mut core_data = self.metadata.include_core_data.then(|| {
            self.fingerprints
                .into_iter()
                .zip(
                    self.locations
                        .expect("locations are present when core data is exported"),
                )
                .map(|(fingerprints, regions)| CoreData { fingerprints, regions })
        });

        let files: Vec<Rc<File>> = self
            .paths
            .into_iter()
            .zip(self.contents)
            .enumerate()
            .map(|(id, (relative_path, content))| {
                Rc::new(File {
                    id,
                    relative_path,
                    content,
                    core_data: core_data.as_mut().and_then(Iterator::next),
                })
            })
            .collect();

        Report::new(metrics, fragments, files, self.metadata)
    }
}

impl fmt::Debug for Dolos {
    fn fmt(&self, fmt: &mut fmt::Formatter) -> fmt::Result {
        fmt.debug_struct("Dolos")
            .field("name", &self.metadata.report_name)
            .field("language", &self.metadata.language)
            .field("paths", &self.paths)
            .finish()
    }
}
