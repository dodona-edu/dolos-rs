//! What makes the input of an analysis valid.
//!
//! [`validate_input`] is the single statement of
//! [`analyze`](crate::analyze)'s contract. `analyze` runs it before it builds
//! anything and returns its [`InputError`], so the algorithm never sees an
//! input it cannot handle.
//!
//! Each rule is one named function over [`Input`]. [`RULES`] lists them in the
//! order they report, so adding a rule means writing it and naming it there.

use crate::Symbol;
use crate::analysis::AnalysisOptions;
use crate::suffixtree::SENTINEL_SYMBOL;
use std::fmt;
use std::ops::Range;

/// Why an analysis cannot accept its input.
#[derive(Debug, PartialEq, Eq)]
pub enum InputError {
    /// The analysis has fewer than the two sequences a pair needs.
    TooFewSequences { given: usize },
    /// A sequence holds the symbol the tree reserves as its end-of-sequence
    /// sentinel.
    ReservedSymbol { sequence: usize, position: usize },
    /// [`AnalysisOptions::min_match_length`] is `0`.
    MinMatchLengthZero,
    /// The ignored ranges cover a different number of sequences than the
    /// analysis has.
    IgnoredSequenceCount { given: usize, expected: usize },
    /// An ignored range starts after it ends.
    IgnoredRangeOrder { sequence: usize, range: Range<usize> },
    /// An ignored range reaches past the end of its sequence.
    IgnoredRangeOutOfBounds { sequence: usize, range: Range<usize>, length: usize },
}

impl fmt::Display for InputError {
    fn fmt(&self, fmt: &mut fmt::Formatter) -> fmt::Result {
        match self {
            Self::TooFewSequences { given } => write!(
                fmt,
                "the analysis needs at least 2 sequences to form a pair, it has {given}"
            ),
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

/// The input of one analysis, borrowed for the length of a
/// [`validate_input`] call.
///
/// It exists so every rule has the same shape and can go in [`RULES`].
struct Input<'a> {
    /// The sequences to compare.
    sequences: &'a [Vec<Symbol>],
    /// The ignored ranges of each sequence, or nothing at all.
    ignored: Option<&'a [Vec<Range<usize>>]>,
    /// The options of the run.
    options: &'a AnalysisOptions,
}

/// One rule the input of an analysis must satisfy.
type Rule = fn(&Input) -> Result<(), InputError>;

/// Every rule, in the order they report.
const RULES: &[Rule] = &[
    sequences_form_at_least_one_pair,
    match_length_is_positive,
    sequences_leave_the_sentinel_free,
    ignored_covers_every_sequence,
    ignored_ranges_stay_inside_their_sequence,
];

/// Check that [`analyze`](crate::analyze) accepts this input.
///
/// There must be at least two sequences, because the analysis compares pairs.
/// `ignored` is `None`, or one list of ranges per sequence. Its ranges must be
/// ordered and stay within the sequence they index. Empty and overlapping ranges
/// are allowed.
///
/// # Errors
///
/// Returns the first rule that the input breaks.
pub(crate) fn validate_input(
    sequences: &[Vec<Symbol>],
    ignored: Option<&[Vec<Range<usize>>]>,
    options: &AnalysisOptions,
) -> Result<(), InputError> {
    let input = Input { sequences, ignored, options };

    RULES.iter().try_for_each(|rule| rule(&input))
}

/// The analysis reports one result per pair, and a pair needs two sequences.
fn sequences_form_at_least_one_pair(input: &Input) -> Result<(), InputError> {
    match input.sequences.len() {
        given if given < 2 => Err(InputError::TooFewSequences { given }),
        _ => Ok(()),
    }
}

/// A match of no symbols is not a match.
fn match_length_is_positive(input: &Input) -> Result<(), InputError> {
    match input.options.min_match_length {
        0 => Err(InputError::MinMatchLengthZero),
        _ => Ok(()),
    }
}

/// The tree appends [`SENTINEL_SYMBOL`] to every sequence, so no input may hold
/// it: two sequences would compare equal where they do not match.
fn sequences_leave_the_sentinel_free(input: &Input) -> Result<(), InputError> {
    for (sequence, symbols) in input.sequences.iter().enumerate() {
        if let Some(position) = symbols.iter().position(|&symbol| symbol == SENTINEL_SYMBOL) {
            return Err(InputError::ReservedSymbol { sequence, position });
        }
    }

    Ok(())
}

/// An ignore list indexes the sequences by position, so a partial list would
/// silently ignore the wrong ones. `None` ignores nothing.
fn ignored_covers_every_sequence(input: &Input) -> Result<(), InputError> {
    let Some(ignored) = input.ignored else {
        return Ok(());
    };

    if ignored.len() == input.sequences.len() {
        return Ok(());
    }

    Err(InputError::IgnoredSequenceCount { given: ignored.len(), expected: input.sequences.len() })
}

/// A range that reaches past its sequence marks bits of the next one and
/// underflows the totals that subtract it.
fn ignored_ranges_stay_inside_their_sequence(input: &Input) -> Result<(), InputError> {
    let Some(ignored) = input.ignored else {
        return Ok(());
    };

    for (sequence, ranges) in ignored.iter().enumerate() {
        let length = input.sequences[sequence].len();

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

#[cfg(test)]
// The tests below build empty and reversed ranges on purpose: they are what
// `validate_input` has to accept and reject.
#[allow(clippy::reversed_empty_ranges, clippy::single_range_in_vec_init)]
mod tests {
    use super::*;

    fn options() -> AnalysisOptions {
        AnalysisOptions { min_match_length: 1, keep_matches: true }
    }

    /// An empty ignore list, an empty range and overlapping ranges are all legal.
    #[test]
    fn validate_input_accepts_valid_input() {
        let sequences = vec![vec![1, 2, 3, 4], vec![1, 2]];

        assert_eq!(validate_input(&sequences, None, &options()), Ok(()));
        assert_eq!(
            validate_input(
                &sequences,
                Some(&[vec![1..1, 0..3, 2..4], vec![0..2]]),
                &options()
            ),
            Ok(())
        );
    }

    #[test]
    fn validate_input_rejects_fewer_than_two_sequences() {
        assert_eq!(
            validate_input(&[], None, &options()),
            Err(InputError::TooFewSequences { given: 0 })
        );
        assert_eq!(
            validate_input(&[vec![1, 2]], None, &options()),
            Err(InputError::TooFewSequences { given: 1 })
        );
    }

    #[test]
    fn validate_input_rejects_the_reserved_sentinel() {
        let sequences = vec![vec![1, 2], vec![3, SENTINEL_SYMBOL]];

        assert_eq!(
            validate_input(&sequences, None, &options()),
            Err(InputError::ReservedSymbol { sequence: 1, position: 1 })
        );
    }

    #[test]
    fn validate_input_rejects_a_zero_min_match_length() {
        let options = AnalysisOptions { min_match_length: 0, keep_matches: false };

        assert_eq!(
            validate_input(&[vec![1, 2], vec![3, 4]], None, &options),
            Err(InputError::MinMatchLengthZero)
        );
    }

    /// A range that reaches past its sequence corrupts the totals, so it is
    /// rejected even though the mask would accept it.
    #[test]
    fn validate_input_rejects_ignored_ranges_outside_their_sequence() {
        let sequences = vec![vec![1, 2, 3], vec![1, 2]];

        assert_eq!(
            validate_input(&sequences, Some(&[vec![0..4], vec![]]), &options()),
            Err(InputError::IgnoredRangeOutOfBounds { sequence: 0, range: 0..4, length: 3 })
        );
        assert_eq!(
            validate_input(&sequences, Some(&[vec![2..1], vec![]]), &options()),
            Err(InputError::IgnoredRangeOrder { sequence: 0, range: 2..1 })
        );
    }

    #[test]
    fn validate_input_rejects_an_ignore_list_that_covers_the_wrong_number_of_sequences() {
        let sequences = vec![vec![1, 2], vec![3, 4]];

        assert_eq!(
            validate_input(&sequences, Some(&[vec![]]), &options()),
            Err(InputError::IgnoredSequenceCount { given: 1, expected: 2 })
        );
        // An empty list is not the way to ignore nothing. `None` is.
        assert_eq!(
            validate_input(&sequences, Some(&[]), &options()),
            Err(InputError::IgnoredSequenceCount { given: 0, expected: 2 })
        );
    }
}
