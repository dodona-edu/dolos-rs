//! Which positions to leave out of an analysis.
//!
//! [`IgnoredPositions`] is the caller's side of this: a list of ranges per
//! sequence, and nothing more. Deciding *which* positions to ignore is the
//! caller's business, because it depends on what the symbols mean.
//!
//! [`analyze`](crate::analyze) turns that list into an [`IgnoreMask`], the
//! packed bit-vector the analysis reads while it collects matches.

use crate::collections::vec_bitmap::VecBitmap;
use std::ops::Range;

/// The positions of each sequence to leave out of the analysis.
///
/// Ignored positions act as barriers: no match spans one, and they do not
/// count towards a sequence's total. Ranges are half-open and index a
/// sequence, not the source the symbols were derived from.
#[derive(Debug, Clone, Default)]
pub struct IgnoredPositions {
    /// The ignored ranges of the sequence with the same index. Every range must
    /// be non-empty and stay within the sequence it indexes.
    ranges: Vec<Vec<Range<usize>>>,
}

impl IgnoredPositions {
    /// Ignore the given ranges, one list per sequence.
    pub fn new(per_sequence: Vec<Vec<Range<usize>>>) -> Self {
        Self { ranges: per_sequence }
    }

    /// The number of sequences the ranges cover. `0` when nothing is ignored.
    pub(crate) fn sequence_count(&self) -> usize {
        self.ranges.len()
    }

    /// The ignored ranges of every sequence, in order.
    pub fn ranges(self) -> Vec<Vec<Range<usize>>> {
        self.ranges
    }

    /// Every ignored range, paired with the index of its sequence.
    pub(crate) fn iter(&self) -> impl Iterator<Item = (usize, &Vec<Range<usize>>)> {
        self.ranges.iter().enumerate()
    }
}

/// The ignored positions of every sequence, as one packed bit-vector each.
pub(crate) struct IgnoreMask {
    /// Bit `p` of item `s` is set when position `p` of sequence `s` is ignored;
    /// `None` when no position of any sequence is ignored.
    ignored_bitmap: Option<VecBitmap>,
}

impl IgnoreMask {
    /// Pack the ranges of `ignored` into one bit-vector per sequence, sized by
    /// `lengths`. Nothing ignored means no bit-vector at all.
    pub(crate) fn new(ignored: &IgnoredPositions, lengths: &[usize]) -> Self {
        let mut ignored_bitmap: Option<VecBitmap> = None;

        for (sequence, ranges) in ignored.iter() {
            let length = lengths.get(sequence).copied().unwrap_or(0);

            for range in ranges {
                debug_assert!(
                    range.start < range.end,
                    "The start of ignored range {range:?} of sequence {sequence} should be before the end"
                );
                debug_assert!(
                    range.end <= length,
                    "The end of ignored range {range:?} of sequence {sequence} should not exceed the length of the sequence, which is {length}"
                );

                ignored_bitmap
                    .get_or_insert_with(|| VecBitmap::new(lengths))
                    .item_mut(sequence)
                    .mark(range.start, range.end - range.start);
            }
        }

        Self { ignored_bitmap }
    }

    /// The number of ignored positions in `sequence`.
    pub fn ignored_count(&self, sequence: usize) -> usize {
        match self.ignored_bitmap.as_ref() {
            Some(mask) => mask.item(sequence).count_ones(),
            None => 0,
        }
    }

    /// Whether no position of any sequence is ignored.
    pub fn is_empty(&self) -> bool {
        self.ignored_bitmap.is_none()
    }

    /// The maximal ignore-free runs of `sequence` within `range`, in ascending
    /// order. Ignored positions act as barriers, so no run spans one.
    pub fn runs(
        &self,
        sequence: usize,
        range: Range<usize>,
    ) -> impl Iterator<Item = Range<usize>> + '_ {
        let Range { mut start, end } = range;

        std::iter::from_fn(move || {
            if start >= end {
                return None;
            }

            let run = match &self.ignored_bitmap {
                None => start..end,
                Some(mask) => {
                    let item = mask.item(sequence);
                    let start = item.next_zero_bit(start, end)?;
                    let end = item.next_one_bit(start, end).unwrap_or(end);
                    start..end
                }
            };

            start = run.end;
            Some(run)
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ignored(per_sequence: &[&[Range<usize>]]) -> IgnoredPositions {
        IgnoredPositions::new(per_sequence.iter().map(|r| r.to_vec()).collect())
    }

    #[test]
    fn usable_runs_split_at_ignored_positions() {
        let mask = IgnoreMask::new(&ignored(&[&[2..3, 5..7], &[]]), &[7, 2]);

        assert_eq!(mask.runs(0, 0..7).collect::<Vec<_>>(), vec![0..2, 3..5]);
        // A range may start inside an ignored stretch and end inside a run.
        assert_eq!(mask.runs(0, 2..4).collect::<Vec<_>>(), vec![3..4]);
        // A sequence without a single ignored position is one run.
        assert_eq!(mask.runs(1, 0..2).collect::<Vec<_>>(), vec![0..2]);
        assert_eq!(mask.ignored_count(0), 3);
        assert_eq!(mask.ignored_count(1), 0);
    }

    #[test]
    fn without_a_mask_a_whole_range_is_one_usable_run() {
        let mask = IgnoreMask::new(&IgnoredPositions::default(), &[5]);

        assert!(mask.is_empty());
        assert_eq!(mask.runs(0, 1..4).collect::<Vec<_>>(), vec![1..4]);
        // An empty range yields no run at all.
        assert!(mask.runs(0, 2..2).next().is_none());
    }

    #[test]
    fn padding_bits_of_the_last_word_are_not_counted() {
        // A sequence longer than one word, so the padding bits of the final
        // word would show up in the count if they were ever written.
        let ranges: Vec<Range<usize>> = (0..100).step_by(7).map(|i| i..i + 1).collect();
        let mask = IgnoreMask::new(&IgnoredPositions::new(vec![ranges]), &[100]);

        assert_eq!(mask.ignored_count(0), (0..100).step_by(7).count());
    }
}
