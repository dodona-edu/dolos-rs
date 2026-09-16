use crate::collections::bit_region::{BitRegion, BitRegionMut};
use crate::collections::bit_vec::BitVec;
use std::ops::Range;

/// A flat bitmap storing one independent bit-vector per indexed item.
///
/// Internally, a single contiguous [`BitVec`] is used; each item's bit-vector
/// starts at a precomputed offset so that no individual heap allocations are
/// needed. Reading and writing bits happens through the [`BitRegion`] an item
/// resolves to.
pub struct VecBitmap {
    /// Underlying packed bit storage.
    buf: BitVec,
    /// Bit length of each item.
    lengths: Vec<usize>,
    /// `word_bases[k]` = word index in `buf` at which item `k`'s bit-vector begins.
    word_bases: Vec<usize>,
}

impl VecBitmap {
    /// Create a new [`VecBitmap`] for items whose bit lengths are given by
    /// `lengths`. All bits are initialized to zero.
    pub fn new(lengths: &[usize]) -> Self {
        let mut word_bases = vec![0usize; lengths.len()];
        let mut total = 0;
        for (item, &length) in lengths.iter().enumerate() {
            word_bases[item] = total;
            total += length.div_ceil(64);
        }

        Self {
            buf: BitVec::new(total),
            lengths: lengths.to_vec(),
            word_bases,
        }
    }

    /// The number of items.
    pub fn item_count(&self) -> usize {
        self.lengths.len()
    }

    /// Whether the bitmap holds no items at all.
    pub fn is_empty(&self) -> bool {
        self.lengths.is_empty()
    }

    /// The bit-vector of the given item.
    pub fn item(&self, item: usize) -> BitRegion<'_> {
        self.buf.region(self.word_bases[item], self.lengths[item])
    }

    /// The bit-vector of the given item, for writing.
    pub fn item_mut(&mut self, item: usize) -> BitRegionMut<'_> {
        self.buf
            .region_mut(self.word_bases[item], self.lengths[item])
    }

    /// The maximal runs of `item` within `range`, in ascending order. `bit`
    /// selects which value the runs hold. A bit of the other value acts as a
    /// barrier, so no run spans one.
    pub fn runs(
        &self,
        item: usize,
        range: Range<usize>,
        bit: bool,
    ) -> impl Iterator<Item = Range<usize>> {
        let bits = self.item(item);
        let Range { mut start, end } = range;

        std::iter::from_fn(move || {
            let run_start = bits.next_bit(start, end, bit)?;
            let run_end = bits.next_bit(run_start, end, !bit).unwrap_or(end);

            start = run_end;
            Some(run_start..run_end)
        })
    }

    /// The runs where `left` and `right` both hold `bit` over `length`
    /// positions, as offsets from `left_start` and `right_start`.
    pub fn shared_runs(
        &self,
        left: usize,
        left_start: usize,
        right: usize,
        right_start: usize,
        length: usize,
        bit: bool,
    ) -> impl Iterator<Item = Range<usize>> {
        // Both sides are walked in offsets from their own start, so the two run
        // lists can be intersected directly.
        let offsets = |run: Range<usize>, start: usize| run.start - start..run.end - start;
        let mut left_runs = self.runs(left, left_start..left_start + length, bit);
        let mut right_runs = self.runs(right, right_start..right_start + length, bit);
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
    fn items_are_independent() {
        let mut bm = VecBitmap::new(&[10, 10]);
        assert_eq!(bm.item_count(), 2);
        assert_eq!(bm.item(0).count_ones(), 0);

        bm.item_mut(0).mark(0, 5);
        bm.item_mut(1).mark(3, 4);

        assert_eq!(bm.item(0).iter_ones().collect::<Vec<_>>(), [0, 1, 2, 3, 4]);
        assert_eq!(bm.item(1).iter_ones().collect::<Vec<_>>(), [3, 4, 5, 6]);
    }

    #[test]
    fn items_of_mixed_lengths_keep_their_own_bits() {
        let lengths = [20, 65, 130, 10, 200, 64, 99, 128, 300, 50];
        let mut bm = VecBitmap::new(&lengths);

        // Each item is marked in its middle third.
        for (index, &length) in lengths.iter().enumerate() {
            bm.item_mut(index).mark(length / 3, length / 3);
        }

        for (index, &length) in lengths.iter().enumerate() {
            let item = bm.item(index);
            assert_eq!(item.len(), length);
            assert_eq!(
                item.count_ones(),
                length / 3,
                "item {index} (length {length})"
            );
            assert_eq!(item.count_zeros(), length - length / 3);
        }
    }

    #[test]
    fn runs_stop_at_a_bit_of_the_other_value() {
        let mut bm = VecBitmap::new(&[7, 2]);
        bm.item_mut(0).mark(2, 1);
        bm.item_mut(0).mark(5, 2);

        assert_eq!(
            bm.runs(0, 0..7, false).collect::<Vec<_>>(),
            vec![0..2, 3..5]
        );
        assert_eq!(bm.runs(0, 0..7, true).collect::<Vec<_>>(), vec![2..3, 5..7]);
        // A range may start inside a run of the other value.
        assert_eq!(bm.runs(0, 2..4, false).collect::<Vec<_>>(), vec![3..4]);
        // An item without a single marked position is one run.
        assert_eq!(bm.runs(1, 0..2, false).collect::<Vec<_>>(), vec![0..2]);
        assert!(bm.runs(1, 0..2, true).next().is_none());
        // An empty range yields no run at all.
        assert!(bm.runs(1, 1..1, false).next().is_none());
    }

    #[test]
    fn shared_runs_intersect_both_items() {
        let mut bm = VecBitmap::new(&[10, 10]);
        bm.item_mut(0).mark(3, 1);
        bm.item_mut(1).mark(6, 1);

        // The marked bit of item 0 lands on offset 1, the one of item 1 on
        // offset 5. Both cut the shared range.
        assert_eq!(
            bm.shared_runs(0, 2, 1, 1, 8, false).collect::<Vec<_>>(),
            vec![0..1, 2..5, 6..8]
        );
    }

    #[test]
    fn scans_do_not_leak_into_the_neighbouring_item() {
        let mut bm = VecBitmap::new(&[100, 100, 100]);
        bm.item_mut(0).mark(0, 100);
        bm.item_mut(2).mark(0, 100);

        assert_eq!(bm.item(1).next_one_bit(0, 100), None);
        assert_eq!(bm.item(1).next_zero_bit(0, 100), Some(0));
        assert_eq!(bm.item(2).next_one_bit(0, 100), Some(0));
    }
}
