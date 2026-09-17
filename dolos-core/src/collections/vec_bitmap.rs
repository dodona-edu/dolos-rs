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

    /// The maximal runs over `length` positions where neither `left` nor
    /// `right` holds a set bit, in ascending order. The runs are offsets from
    /// `left_start` and `right_start`.
    ///
    /// Both windows are read 64 bits at a time and combined with a bitwise OR,
    /// so the two sides are walked in one pass.
    pub fn shared_zero_runs(
        &self,
        left: usize,
        left_start: usize,
        right: usize,
        right_start: usize,
        length: usize,
    ) -> impl Iterator<Item = Range<usize>> {
        let (left_bits, right_bits) = (self.item(left), self.item(right));

        // The two windows combined over up to 64 offsets from `from` on. A set
        // bit marks an offset that at least one side ignores. The bits above
        // the returned count are clear.
        let combine = move |from: usize| {
            let count = (length - from).min(64);
            let bits = left_bits.chunk(left_start + from, count)
                | right_bits.chunk(right_start + from, count);
            (bits, count)
        };

        // The first offset at or after `at` that neither side ignores. A chunk
        // of nothing but set bits reports no offset, because the bits above
        // `count` are clear.
        let next_free = move |mut at: usize| {
            while at < length {
                let (bits, count) = combine(at);
                let ignored = bits.trailing_ones() as usize;
                if ignored < count {
                    return Some(at + ignored);
                }
                at += count;
            }
            None
        };

        // The first offset at or after `at` that one of the sides ignores.
        let next_ignored = move |mut at: usize| {
            while at < length {
                let (bits, count) = combine(at);
                if bits != 0 {
                    return Some(at + bits.trailing_zeros() as usize);
                }
                at += count;
            }
            None
        };

        let mut start = 0;
        std::iter::from_fn(move || {
            let run_start = next_free(start)?;
            let run_end = next_ignored(run_start).unwrap_or(length);

            start = run_end;
            Some(run_start..run_end)
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rand::{RngExt, SeedableRng, rngs::StdRng};

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
    fn shared_zero_runs_cut_at_both_items() {
        let mut bm = VecBitmap::new(&[10, 10]);
        bm.item_mut(0).mark(3, 1);
        bm.item_mut(1).mark(6, 1);

        // The marked bit of item 0 lands on offset 1, the one of item 1 on
        // offset 5. Both cut the shared range.
        assert_eq!(
            bm.shared_zero_runs(0, 2, 1, 1, 8).collect::<Vec<_>>(),
            vec![0..1, 2..5, 6..8]
        );
    }

    /// The runs where neither side is set, one position at a time.
    fn naive_shared_zero_runs(
        bm: &VecBitmap,
        left: usize,
        left_start: usize,
        right: usize,
        right_start: usize,
        length: usize,
    ) -> Vec<Range<usize>> {
        let (left_bits, right_bits) = (bm.item(left), bm.item(right));
        let free = |offset: usize| {
            !left_bits.get(left_start + offset) && !right_bits.get(right_start + offset)
        };

        let mut runs: Vec<Range<usize>> = Vec::new();
        for offset in (0..length).filter(|&offset| free(offset)) {
            match runs.last_mut() {
                Some(run) if run.end == offset => run.end = offset + 1,
                _ => runs.push(offset..offset + 1),
            }
        }
        runs
    }

    #[test]
    fn shared_zero_runs_match_a_position_by_position_scan() {
        let mut rng = StdRng::seed_from_u64(20250902);
        const BITS: usize = 512;
        const WINDOWS: [usize; 11] = [1, 2, 63, 64, 65, 127, 128, 129, 191, 192, 193];

        let mut bm = VecBitmap::new(&[BITS, BITS]);
        for trial in 0..2000 {
            // Alternate sparse and dense patterns so both long free runs and
            // long ignored runs occur.
            let threshold = if trial % 2 == 0 { 16u8 } else { 224 };
            for item in 0..2 {
                let mut bits = bm.item_mut(item);
                bits.clear(0, BITS);
                for position in (0..BITS).filter(|_| rng.random::<u8>() < threshold) {
                    bits.mark(position, 1);
                }
            }

            let length = if trial % 3 == 0 {
                WINDOWS[trial % WINDOWS.len()]
            } else {
                rng.random::<u32>() as usize % (BITS / 2) + 1
            };
            let left_start = rng.random::<u32>() as usize % (BITS - length + 1);
            let right_start = rng.random::<u32>() as usize % (BITS - length + 1);

            assert_eq!(
                bm.shared_zero_runs(0, left_start, 1, right_start, length)
                    .collect::<Vec<_>>(),
                naive_shared_zero_runs(&bm, 0, left_start, 1, right_start, length),
                "trial {trial}: starts {left_start}/{right_start}, length {length}"
            );
        }
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
