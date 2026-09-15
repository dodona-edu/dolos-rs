use crate::Symbol;
use crate::ignore::IgnoreMask;
use crate::suffixtree::{AnalysisResult, SENTINEL_SYMBOL, SuffixTree};
use std::fmt;
use std::ops::Range;

/// Options controlling an [`analyze`] run.
pub struct AnalysisOptions {
    /// Minimum shared substring length, in symbols. At least 1.
    pub min_match_length: usize,
    /// Keep the raw matches of every pair.
    pub keep_matches: bool,
}

/// Why [`analyze`] cannot accept an input.
#[derive(Debug, PartialEq, Eq)]
pub enum InputError {
    /// A sequence holds [`SENTINEL_SYMBOL`], which the tree reserves.
    ReservedSymbol { sequence: usize, position: usize },
    /// [`AnalysisOptions::min_match_length`] is `0`.
    MinMatchLengthZero,
    /// `ignored` covers a different number of sequences than the analysis has.
    IgnoredSequenceCount { given: usize, expected: usize },
    /// An ignored range starts after it ends.
    IgnoredRangeOrder { sequence: usize, range: Range<usize> },
    /// An ignored range reaches past the end of its sequence.
    IgnoredRangeOutOfBounds { sequence: usize, range: Range<usize>, length: usize },
}

impl fmt::Display for InputError {
    fn fmt(&self, fmt: &mut fmt::Formatter) -> fmt::Result {
        match self {
            Self::ReservedSymbol { sequence, position } => write!(
                fmt,
                "position {position} of sequence {sequence} holds the reserved end-of-sequence symbol"
            ),
            Self::MinMatchLengthZero => write!(fmt, "the minimum match length must be at least 1"),
            Self::IgnoredSequenceCount { given, expected } => write!(
                fmt,
                "the ignored positions cover {given} sequences, the analysis has {expected}"
            ),
            Self::IgnoredRangeOrder { sequence, range } => write!(
                fmt,
                "ignored range {range:?} of sequence {sequence} starts after it ends"
            ),
            Self::IgnoredRangeOutOfBounds { sequence, range, length } => write!(
                fmt,
                "ignored range {range:?} of sequence {sequence} reaches past its length {length}"
            ),
        }
    }
}

impl std::error::Error for InputError {}

/// Check that [`analyze`] accepts this input.
///
/// `ignored` must cover either every sequence or none at all. Its ranges must be
/// ordered and stay within the sequence they index. Empty and overlapping ranges
/// are allowed.
pub fn check(
    sequences: &[Vec<Symbol>],
    ignored: &[Vec<Range<usize>>],
    options: &AnalysisOptions,
) -> Result<(), InputError> {
    if options.min_match_length == 0 {
        return Err(InputError::MinMatchLengthZero);
    }

    for (sequence, symbols) in sequences.iter().enumerate() {
        if let Some(position) = symbols.iter().position(|&s| s == SENTINEL_SYMBOL) {
            return Err(InputError::ReservedSymbol { sequence, position });
        }
    }

    if !ignored.is_empty() && ignored.len() != sequences.len() {
        return Err(InputError::IgnoredSequenceCount {
            given: ignored.len(),
            expected: sequences.len(),
        });
    }

    for (sequence, ranges) in ignored.iter().enumerate() {
        let length = sequences[sequence].len();

        for range in ranges {
            if range.start > range.end {
                return Err(InputError::IgnoredRangeOrder { sequence, range: range.clone() });
            }
            if range.end > length {
                return Err(InputError::IgnoredRangeOutOfBounds {
                    sequence,
                    range: range.clone(),
                    length,
                });
            }
        }
    }

    Ok(())
}

/// Build the generalized suffix tree over `sequences` and collect all pairwise
/// maximal exact matches.
///
/// Matches are split at the positions `ignored` marks, and those positions are
/// left out of the totals. Which positions those are is up to the caller.
///
/// # Errors
///
/// Returns the reason [`check`] rejects the input.
pub fn analyze(
    sequences: &[Vec<Symbol>],
    ignored: &[Vec<Range<usize>>],
    options: &AnalysisOptions,
) -> Result<AnalysisResult, InputError> {
    check(sequences, ignored, options)?;

    let lengths: Vec<usize> = sequences.iter().map(Vec::len).collect();
    let mask = IgnoreMask::new(ignored, &lengths);
    let tree = SuffixTree::build(sequences);

    Ok(tree.analyze(
        sequences,
        &mask,
        options.min_match_length,
        options.keep_matches,
    ))
}

#[cfg(test)]
// The tests below build empty and reversed ranges on purpose: they are what
// `check` has to accept and reject.
#[allow(clippy::reversed_empty_ranges, clippy::single_range_in_vec_init)]
mod tests {
    use super::*;

    fn options() -> AnalysisOptions {
        AnalysisOptions { min_match_length: 1, keep_matches: true }
    }

    /// An empty ignore list, an empty range and overlapping ranges are all legal.
    #[test]
    fn check_accepts_valid_input() {
        let sequences = vec![vec![1, 2, 3, 4], vec![1, 2]];

        assert_eq!(check(&sequences, &[], &options()), Ok(()));
        assert_eq!(
            check(
                &sequences,
                &[vec![1..1, 0..3, 2..4], vec![0..2]],
                &options()
            ),
            Ok(())
        );
    }

    #[test]
    fn check_rejects_the_reserved_sentinel() {
        let sequences = vec![vec![1, 2], vec![3, SENTINEL_SYMBOL]];

        assert_eq!(
            check(&sequences, &[], &options()),
            Err(InputError::ReservedSymbol { sequence: 1, position: 1 })
        );
    }

    #[test]
    fn check_rejects_a_zero_min_match_length() {
        let options = AnalysisOptions { min_match_length: 0, keep_matches: false };

        assert_eq!(
            check(&[vec![1, 2]], &[], &options),
            Err(InputError::MinMatchLengthZero)
        );
    }

    /// A range that reaches past its sequence corrupts the totals, so it is
    /// rejected even though the mask would accept it.
    #[test]
    fn check_rejects_ignored_ranges_outside_their_sequence() {
        let sequences = vec![vec![1, 2, 3]];

        assert_eq!(
            check(&sequences, &[vec![0..4]], &options()),
            Err(InputError::IgnoredRangeOutOfBounds { sequence: 0, range: 0..4, length: 3 })
        );
        assert_eq!(
            check(&sequences, &[vec![2..1]], &options()),
            Err(InputError::IgnoredRangeOrder { sequence: 0, range: 2..1 })
        );
    }

    #[test]
    fn check_rejects_an_ignore_list_that_covers_the_wrong_number_of_sequences() {
        let sequences = vec![vec![1, 2], vec![3, 4]];

        assert_eq!(
            check(&sequences, &[vec![]], &options()),
            Err(InputError::IgnoredSequenceCount { given: 1, expected: 2 })
        );
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
                let metrics = analyze(&sequences, &ignored, &options()).unwrap();
                let metrics = metrics.metrics.get(0, 1).clone();
                format!("{:.4} {}", metrics.similarity, metrics.longest_match)
            })
            .collect();

        // Position 2 is ignored in the left sequence only. It still splits the
        // match, leaving the runs 0..2 and 3..5, so the longest run is 2 and
        // the similarity is (4 + 4) / (4 + 5).
        assert_eq!(seen.into_iter().collect::<Vec<_>>(), ["0.8889 2"]);
    }

    /// `check` allows an empty range, so the mask must accept one too.
    #[test]
    fn analyze_accepts_an_empty_ignored_range() {
        let sequences = vec![vec![1, 2, 3], vec![1, 2, 3]];
        let ignored = [vec![1..1], vec![]];

        assert_eq!(check(&sequences, &ignored, &options()), Ok(()));
        let result = analyze(&sequences, &ignored, &options()).unwrap();

        // Nothing is ignored, so the pair still matches over its full length.
        assert_eq!(result.metrics.get(0, 1).longest_match, 3);
        assert_eq!(result.metrics.get(0, 1).total_left, 3);
    }

    /// The analysis walks the tree's leaves and has none to walk without input.
    #[test]
    fn analyze_returns_an_empty_result_for_empty_input() {
        let result = analyze(&[], &[], &options()).unwrap();

        assert_eq!(result.metrics.item_count(), 0);
        assert!(result.metrics.is_empty());
        assert!(result.matches.is_some_and(|m| m.is_empty()));
    }
}
