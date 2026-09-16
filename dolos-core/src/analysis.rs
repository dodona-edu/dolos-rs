use crate::Symbol;
use crate::ignore::ignore_ranges_to_mask;
use crate::suffixtree::{AnalysisResult, SuffixTree};
use crate::validation::{InputError, validate_input};
use std::ops::Range;

/// Options controlling an [`analyze`] run.
pub struct AnalysisOptions {
    /// Minimum shared substring length, in symbols. At least 1.
    pub min_match_length: usize,
    /// Keep the raw matches of every pair.
    pub keep_matches: bool,
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
/// Returns an [`InputError`] when the input is not valid.
pub fn analyze(
    sequences: &[Vec<Symbol>],
    ignored: Option<&[Vec<Range<usize>>]>,
    options: &AnalysisOptions,
) -> Result<AnalysisResult, InputError> {
    validate_input(sequences, ignored, options)?;

    let lengths: Vec<usize> = sequences.iter().map(Vec::len).collect();
    let mask = ignore_ranges_to_mask(ignored.unwrap_or_default(), &lengths);
    let tree = SuffixTree::build(sequences);

    Ok(tree.analyze(
        sequences,
        mask.as_ref(),
        options.min_match_length,
        options.keep_matches,
    ))
}

#[cfg(test)]
// The tests below build an empty range on purpose: `validate_input` accepts
// one, so the mask has to as well.
#[allow(clippy::reversed_empty_ranges, clippy::single_range_in_vec_init)]
mod tests {
    use super::*;

    fn options() -> AnalysisOptions {
        AnalysisOptions { min_match_length: 1, keep_matches: true }
    }

    /// The two sides of a match may have different positions ignored. A run is
    /// usable only where neither side is ignored, so the result must not depend
    /// on which side the collector walks.
    #[test]
    fn asymmetric_ignored_positions_give_one_stable_result() {
        let sequences = vec![vec![1, 2, 3, 4, 5], vec![1, 2, 3, 4, 5]];
        let ignored = [vec![2..3], vec![]];

        let seen: std::collections::BTreeSet<String> = (0..50)
            .map(|_| {
                let metrics = analyze(&sequences, Some(&ignored), &options()).unwrap();
                let metrics = metrics.metrics.get(0, 1).clone();
                format!("{:.4} {}", metrics.similarity, metrics.longest_match)
            })
            .collect();

        // Position 2 is ignored in the left sequence only. It still splits the
        // match, leaving the runs 0..2 and 3..5, so the longest run is 2 and
        // the similarity is (4 + 4) / (4 + 5).
        assert_eq!(seen.into_iter().collect::<Vec<_>>(), ["0.8889 2"]);
    }

    /// `validate_input` allows an empty range, so the mask must accept one too.
    #[test]
    fn analyze_accepts_an_empty_ignored_range() {
        let sequences = vec![vec![1, 2, 3], vec![1, 2, 3]];
        let ignored = [vec![1..1], vec![]];

        assert_eq!(
            validate_input(&sequences, Some(&ignored), &options()),
            Ok(())
        );
        let result = analyze(&sequences, Some(&ignored), &options()).unwrap();

        // Nothing is ignored, so the pair still matches over its full length.
        assert_eq!(result.metrics.get(0, 1).longest_match, 3);
        assert_eq!(result.metrics.get(0, 1).total_left, 3);
    }

    /// The analysis reports pairs, so it needs two sequences to report one.
    #[test]
    fn analyze_rejects_fewer_than_two_sequences() {
        assert_eq!(
            analyze(&[], None, &options()).unwrap_err(),
            InputError::TooFewSequences { given: 0 }
        );
        assert_eq!(
            analyze(&[vec![1, 2, 3]], None, &options()).unwrap_err(),
            InputError::TooFewSequences { given: 1 }
        );
    }
}
