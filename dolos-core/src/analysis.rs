use crate::Symbol;
use crate::ignore::ignore_ranges_to_mask;
use crate::suffixtree::{AnalysisResult, SuffixTree};
use crate::validation::validate_input;
use std::ops::Range;

/// Options controlling an [`analyze`] run.
#[derive(Debug)]
#[cfg_attr(
    feature = "wasm",
    derive(serde::Deserialize, tsify::Tsify),
    serde(default, rename_all = "camelCase")
)]
pub struct AnalysisOptions {
    /// Minimum shared substring length, in symbols. At least 1.
    pub min_match_length: usize,
    /// Keep the raw matches of every pair.
    pub keep_matches: bool,
}

impl Default for AnalysisOptions {
    /// The shortest match length the analysis accepts, and no matches kept.
    fn default() -> Self {
        Self { min_match_length: 1, keep_matches: false }
    }
}

/// Build the generalized suffix tree over `sequences` and collect all pairwise
/// maximal exact matches.
///
/// Matches are split at the positions `ignored` marks, and those positions are
/// left out of the totals. Which positions those are is up to the caller.
/// `ignored` holds one list of ranges per sequence, or `None` to ignore
/// nothing.
///
/// # Errors
///
/// Returns an [`ErrorKind::InvalidInput`](std::io::ErrorKind::InvalidInput)
/// error when the input is not valid.
pub fn analyze(
    sequences: &[Vec<Symbol>],
    ignored: Option<&[Vec<Range<usize>>]>,
    options: &AnalysisOptions,
) -> std::io::Result<AnalysisResult> {
    validate_input(sequences, ignored, options)?;

    let lengths: Vec<usize> = sequences.iter().map(Vec::len).collect();
    let mask = ignored.and_then(|ignored| ignore_ranges_to_mask(ignored, &lengths));
    let tree = SuffixTree::build(sequences);

    Ok(tree.analyze(
        sequences,
        mask.as_ref(),
        options.min_match_length,
        options.keep_matches,
    ))
}
