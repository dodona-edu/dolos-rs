//! Which positions to leave out of an analysis.
//!
//! The caller passes a list of ranges per sequence. Deciding *which* positions
//! to ignore is the caller's business, because it depends on what the symbols
//! mean. [`analyze`](crate::analyze) turns that list into an [`IgnoreMask`], the
//! packed bit-vector the analysis reads while it collects matches.

use crate::collections::vec_bitmap::VecBitmap;
use std::ops::Range;

/// The ignored positions of every sequence, as one packed bit-vector each.
pub(crate) struct IgnoreMask {
    /// Bit `p` of item `s` is set when position `p` of sequence `s` is ignored;
    /// `None` when no position of any sequence is ignored.
    ignored_bitmap: Option<VecBitmap>,
}

impl IgnoreMask {
    /// Pack the ranges of `ignored` into one bit-vector per sequence, sized by
    /// `lengths`. Nothing ignored means no bit-vector at all.
    pub(crate) fn new(ignored: &[Vec<Range<usize>>], lengths: &[usize]) -> Self {
        let mut ignored_bitmap: Option<VecBitmap> = None;

        for (sequence, ranges) in ignored.iter().enumerate() {
            for range in ranges {
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

    /// The runs of a match where neither side is ignored, as offsets from the
    /// start of the match.
    ///
    /// The two sides hold equal symbols, but the caller may ignore different
    /// positions in each sequence, so both masks act as barriers.
    pub fn runs_pair(
        &self,
        left: usize,
        left_start: usize,
        right: usize,
        right_start: usize,
        length: usize,
    ) -> impl Iterator<Item = Range<usize>> + '_ {
        // Both sides are walked in offsets from the start of the match, so the
        // two run lists can be intersected directly.
        let offsets = |runs: Range<usize>, start: usize| runs.start - start..runs.end - start;
        let mut left_runs = self.runs(left, left_start..left_start + length);
        let mut right_runs = self.runs(right, right_start..right_start + length);
        let mut current_left = left_runs.next().map(|run| offsets(run, left_start));
        let mut current_right = right_runs.next().map(|run| offsets(run, right_start));

        std::iter::from_fn(move || {
            loop {
                let (left_run, right_run) = (current_left.clone()?, current_right.clone()?);
                let overlap = left_run.start.max(right_run.start)..left_run.end.min(right_run.end);

                // Advance the run that ends first. The other one may still
                // overlap the run that follows it.
                if left_run.end <= right_run.end {
                    current_left = left_runs.next().map(|run| offsets(run, left_start));
                } else {
                    current_right = right_runs.next().map(|run| offsets(run, right_start));
                }

                if !overlap.is_empty() {
                    return Some(overlap);
                }
            }
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn usable_runs_split_at_ignored_positions() {
        let mask = IgnoreMask::new(&[vec![2..3, 5..7], vec![]], &[7, 2]);

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
        let mask = IgnoreMask::new(&[], &[5]);

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
        let mask = IgnoreMask::new(&[ranges], &[100]);

        assert_eq!(mask.ignored_count(0), (0..100).step_by(7).count());
    }
}
