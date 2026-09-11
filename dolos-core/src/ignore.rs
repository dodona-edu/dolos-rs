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
    /// The ignored ranges of the sequence with the same index. Ranges past a
    /// sequence's end, and empty ranges, are dropped when the mask is built.
    per_sequence: Vec<Vec<Range<usize>>>,
}

impl IgnoredPositions {
    /// Ignore the given ranges, one list per sequence.
    pub fn new(per_sequence: Vec<Vec<Range<usize>>>) -> Self {
        Self { per_sequence }
    }

    /// Ignore nothing.
    pub fn none() -> Self {
        Self::default()
    }

    /// The number of sequences the ranges cover. `0` when nothing is ignored.
    pub(crate) fn sequence_count(&self) -> usize {
        self.per_sequence.len()
    }

    /// The ignored ranges of `sequence`, in ascending order.
    pub fn ranges(&self, sequence: usize) -> &[Range<usize>] {
        self.per_sequence.get(sequence).map_or(&[], Vec::as_slice)
    }

    /// Pack the ranges into one bit-vector per sequence, sized by `lengths`.
    /// Nothing ignored means no bit-vector at all.
    pub(crate) fn mask(&self, lengths: &[usize]) -> IgnoreMask {
        let mut ignored_bitmap: Option<VecBitmap> = None;

        for (index, ranges) in self.per_sequence.iter().enumerate() {
            let length = lengths.get(index).copied().unwrap_or(0);
            for range in ranges {
                let end = range.end.min(length);
                if range.start >= end {
                    continue;
                }
                ignored_bitmap
                    .get_or_insert_with(|| VecBitmap::new(lengths))
                    .item_mut(index)
                    .mark(range.start, end - range.start);
            }
        }

        IgnoreMask { ignored_bitmap }
    }
}

/// The ignored positions of every sequence, as one packed bit-vector each.
pub(crate) struct IgnoreMask {
    /// Bit `p` of item `s` is set when position `p` of sequence `s` is ignored;
    /// `None` when no position of any sequence is ignored.
    ignored_bitmap: Option<VecBitmap>,
}

impl IgnoreMask {
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
        let mask = ignored(&[&[2..3, 5..7], &[]]).mask(&[7, 2]);

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
        let mask = IgnoredPositions::none().mask(&[5]);

        assert!(mask.is_empty());
        assert_eq!(mask.runs(0, 1..4).collect::<Vec<_>>(), vec![1..4]);
        // An empty range yields no run at all.
        assert!(mask.runs(0, 2..2).next().is_none());
    }

    /// Ranges reaching past a sequence, empty ranges, and ranges for sequences
    /// that do not exist are dropped rather than panicking.
    #[test]
    // Each sequence really does get a list holding one range here.
    #[allow(clippy::single_range_in_vec_init)]
    fn out_of_range_input_is_dropped() {
        let mask = ignored(&[&[1..99], &[3..3], &[0..1]]).mask(&[2, 2]);

        assert_eq!(mask.ignored_count(0), 1);
        assert_eq!(mask.ignored_count(1), 0);
    }

    #[test]
    fn padding_bits_of_the_last_word_are_not_counted() {
        // A sequence longer than one word, so the padding bits of the final
        // word would show up in the count if they were ever written.
        let ranges: Vec<Range<usize>> = (0..100).step_by(7).map(|i| i..i + 1).collect();
        let mask = IgnoredPositions::new(vec![ranges]).mask(&[100]);

        assert_eq!(mask.ignored_count(0), (0..100).step_by(7).count());
    }
}
