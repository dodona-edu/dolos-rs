use crate::Symbol;
use crate::ignore::IgnoredPositions;
use crate::suffixtree::{AnalysisResult, SuffixTree};

/// Options controlling an [`analyze`] run.
pub struct AnalysisOptions {
    /// Minimum shared substring length (in symbols) to record as a match.
    pub min_match_length: usize,
    /// When `true`, the raw matches are kept in the result.
    pub keep_matches: bool,
}

/// Build the generalized suffix tree over `sequences` and collect all pairwise
/// maximal exact matches.
///
/// Matches are split at the positions `ignored` marks, and those positions are
/// left out of the totals. Which positions those are is up to the caller;
/// pass [`IgnoredPositions::none`] to ignore nothing.
pub fn analyze(
    sequences: &[Vec<Symbol>],
    ignored: &IgnoredPositions,
    options: &AnalysisOptions,
) -> AnalysisResult {
    let lengths: Vec<usize> = sequences.iter().map(Vec::len).collect();
    let mask = ignored.mask(&lengths);
    let tree = SuffixTree::build(sequences);

    tree.analyze(
        sequences,
        &mask,
        options.min_match_length,
        options.keep_matches,
    )
}
