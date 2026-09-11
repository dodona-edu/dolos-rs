use crate::config::DolosConfig;
use crate::file::{AnalysisData, File, FileSet};
use crate::fragment::resolve_fragments;
use crate::ignore;
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
    /// The files being compared, parallel to `hashes` and, when they are kept,
    /// `locations`. Their analysis data is filled in by [`Dolos::build_report`].
    files: Vec<File>,
    hashes: Vec<Vec<Fingerprint>>,
    ignore_hashes: Vec<Vec<Fingerprint>>,
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
        // The regions are needed both to resolve fragments and to export the
        // analysis data.
        let keep_locations = metadata.include_fragments || metadata.include_analysis_data;
        let locations = keep_locations.then_some(Vec::new());

        let mut dolos = Dolos {
            metadata,
            files: Vec::new(),
            hashes: Vec::new(),
            ignore_hashes: Vec::new(),
            locations,
            tokenizer,
        };

        dolos.add_files(dataset.file_set)?;

        if let Some(ignore_path) = dolos.metadata.ignore.clone() {
            dolos.add_ignore_file(ignore_path)?;
        }

        Ok(dolos)
    }

    /// Parse `content` into a fingerprint sequence (and optionally per-fingerprint
    /// source locations when `keep_locations` is `true`).
    fn fingerprint(
        &mut self,
        content: &str,
        keep_locations: bool,
    ) -> (Vec<Fingerprint>, Option<Vec<Region>>) {
        self.tokenizer
            .parse(content)
            .tokens(self.metadata.include_comments)
            .winnow(
                self.metadata.kgram_length,
                self.metadata.kgrams_in_window,
                keep_locations,
            )
    }

    /// Tokenize a source file and register it as a regular file in the analysis.
    ///
    /// The path and content are added to `self.paths` and `self.contents`, the
    /// fingerprints to `self.hashes`, and the locations to `self.locations`
    /// when they are kept.
    fn add_file(&mut self, base_dir: &Path, relative: &Path) -> Result<()> {
        // Only enforce the language-extension match when the language was
        // auto-detected.  If the user explicitly specified the language, they
        // know what they want (e.g., files exported without an extension).
        if !self.metadata.language_detected && !self.metadata.language.matches(relative) {
            return Err(Error::new(
                ErrorKind::InvalidInput,
                format!("Language does not match file: {}", relative.display()),
            ));
        }

        let content = std::fs::read_to_string(base_dir.join(relative))?;
        let (hashes, locations) = self.fingerprint(&content, self.locations.is_some());

        self.hashes.push(hashes);
        if let Some(locs) = self.locations.as_mut() {
            locs.push(locations.expect("locations should be present when they are kept"));
        }
        self.files.push(File {
            id: self.files.len(),
            relative_path: relative.to_path_buf(),
            content,
            analysis_data: None,
        });
        Ok(())
    }

    /// Tokenize a template/ignore file and append its fingerprints to the hash
    /// list so that the suffix tree can suppress common matches.
    ///
    /// Ignore files are never added to `self.files` or `self.locations`: they
    /// do not appear in the report, and no fragment resolution is needed for them.
    fn add_ignore_file(&mut self, path: PathBuf) -> Result<()> {
        let content = std::fs::read_to_string(&path).map_err(|e| {
            Error::new(
                e.kind(),
                format!("Could not read ignore file '{}': {}", path.display(), e),
            )
        })?;
        let (hashes, _) = self.fingerprint(&content, false);
        self.ignore_hashes.push(hashes);
        Ok(())
    }

    fn add_files(&mut self, file_set: FileSet) -> Result<()> {
        for relative in &file_set.relative_paths {
            self.add_file(&file_set.base_dir, relative)?;
        }
        Ok(())
    }

    /// Run the analysis and build a [`Report`].
    pub fn build_report(mut self) -> Report {
        let ignored = ignore::classify(
            &self.hashes,
            &self.ignore_hashes,
            self.metadata.max_fingerprint_file_count,
        );
        let options = AnalysisOptions {
            min_match_length: self.metadata.min_length_match,
            keep_matches: self.metadata.include_fragments,
        };
        let AnalysisResult { metrics, matches } =
            dolos_core::analyze(&self.hashes, &ignored, &options);

        // Resolve the fragments while the locations are still owned here, so
        // that they can be moved into the files afterwards.
        let fragments = matches
            .zip(self.locations.as_deref())
            .map(|(matches, locations)| {
                resolve_fragments(matches, locations, &self.metadata.fragment_sort_by)
            });

        if self.metadata.include_analysis_data {
            let locations = self
                .locations
                .expect("locations are kept when the analysis data is exported");
            for ((file, fingerprints), regions) in
                self.files.iter_mut().zip(self.hashes).zip(locations)
            {
                file.analysis_data = Some(AnalysisData {
                    ignored: ignored.ranges(file.id).to_vec(),
                    fingerprints,
                    regions,
                });
            }
        }

        let files = self.files.into_iter().map(Rc::new).collect();
        Report::new(metrics, fragments, files, self.metadata)
    }
}

impl fmt::Debug for Dolos {
    fn fmt(&self, fmt: &mut fmt::Formatter) -> fmt::Result {
        fmt.debug_struct("Dolos")
            .field("name", &self.metadata.report_name)
            .field("language", &self.metadata.language)
            .field("files", &self.files)
            .finish()
    }
}
