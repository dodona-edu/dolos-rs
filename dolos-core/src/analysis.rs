use crate::Symbol;
use crate::suffixtree::{AnalysisResult, SuffixTree};

/// Options controlling an [`analyze`] run.
///
/// * `min_match_length` - Minimum shared substring length (in symbols) to
///   record as a match.
/// * `keep_matches` - When `true`, the raw matches are kept in the result.
/// * `max_seq_count` - Suppress substrings that appear in more than these
///   many distinct sequences (boilerplate filter). `None` disables the cap.
pub struct AnalysisOptions {
    pub min_match_length: usize,
    pub keep_matches: bool,
    pub max_seq_count: Option<usize>,
}

/// Run the full suffix-tree analysis over a set of symbol sequences: build the
/// generalized suffix tree, mark every substring of `ignored_sequences` as
/// ignored, and collect all pairwise maximal exact matches.
///
/// # Parameters
/// * `sequences` - The sequences to analyze.
/// * `ignored_sequences` - Sequences whose substrings are marked as ignored.
/// * `options` - [`AnalysisOptions`] controlling match length, match retention,
///   and the boilerplate frequency cap.
pub fn analyze(
    sequences: &[Vec<Symbol>],
    ignored_sequences: &[Vec<Symbol>],
    options: &AnalysisOptions,
) -> AnalysisResult {
    let mut tree = SuffixTree::build(sequences);
    tree.add_ignored_sequences(sequences, ignored_sequences);
    let exclude_ignored = options.max_seq_count.is_some() || !ignored_sequences.is_empty();

    tree.analyze(
        sequences,
        options.min_match_length,
        options.keep_matches,
        exclude_ignored,
        options.max_seq_count,
    )
}
