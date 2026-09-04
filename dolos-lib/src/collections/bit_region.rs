use crate::collections::word_slice::WordSlice;

/// A borrowed view over one bit-vector: the packed `u64` words plus the number
/// of bits they hold.
///
/// Every read operation on a bit-vector lives here; the containers in this
/// module only resolve an address to a region.
#[derive(Copy, Clone)]
pub struct BitRegion<'a> {
    words: &'a [u64],
    len: usize,
}

/// A mutable view over one bit-vector.
pub struct BitRegionMut<'a> {
    words: &'a mut [u64],
    len: usize,
}

impl<'a> BitRegion<'a> {
    /// View `words` as a bit-vector of `len` bits.
    #[inline]
    pub fn new(words: &'a [u64], len: usize) -> Self {
        debug_assert_eq!(
            words.len(),
            len.div_ceil(64),
            "word count does not fit {len} bits"
        );
        Self { words, len }
    }

    /// The number of bits in the region.
    pub fn len(self) -> usize {
        self.len
    }

    /// Whether the region holds no bits at all.
    pub fn is_empty(self) -> bool {
        self.len == 0
    }

    /// The bit at `position`.
    pub fn get(self, position: usize) -> bool {
        debug_assert!(position < self.len, "position {position} is out of range");
        (self.words[position / 64] >> (position % 64)) & 1 == 1
    }

    /// The number of set bits.
    pub fn count_ones(self) -> usize {
        self.words().count_ones()
    }

    /// The number of clear bits, padding excluded.
    pub fn count_zeros(self) -> usize {
        self.len - self.count_ones()
    }

    /// The index of the first set bit in `[start, end)`, or `None` when the
    /// range is empty or holds no set bit.
    ///
    /// Scans whole `u64` words, so the cost is per word inspected rather than
    /// per bit. `end` is clamped to [`Self::len`].
    pub fn next_set_bit(self, start: usize, end: usize) -> Option<usize> {
        next_bit(self.words, start, self.clamp(end), 0)
    }

    /// The index of the first clear bit in `[start, end)`, or `None` when the
    /// range is empty or entirely set.
    ///
    /// Paired with [`Self::next_set_bit`] this walks the region's alternating
    /// runs without visiting every bit. `end` is clamped to [`Self::len`], so
    /// the padding of the last word is never reported.
    pub fn next_clear_bit(self, start: usize, end: usize) -> Option<usize> {
        next_bit(self.words, start, self.clamp(end), u64::MAX)
    }

    /// The positions of the set bits, in ascending order.
    pub fn iter_ones(self) -> impl Iterator<Item = usize> + 'a {
        let mut cursor = 0;
        std::iter::from_fn(move || {
            let position = self.next_set_bit(cursor, self.len)?;
            cursor = position + 1;
            Some(position)
        })
    }

    /// The packed `u64` words backing the region.
    pub fn words(self) -> WordSlice<'a> {
        WordSlice::new(self.words)
    }

    /// Cut `end` down to the region's length.
    fn clamp(self, end: usize) -> usize {
        debug_assert!(end <= self.len, "range end {end} is out of range");
        end.min(self.len)
    }
}

impl<'a> BitRegionMut<'a> {
    /// View `words` as a bit-vector of `len` bits.
    #[inline]
    pub fn new(words: &'a mut [u64], len: usize) -> Self {
        debug_assert_eq!(
            words.len(),
            len.div_ceil(64),
            "word count does not fit {len} bits"
        );
        Self { words, len }
    }

    /// Set the bits `[start, start + length)`.
    #[inline]
    pub fn mark(&mut self, start: usize, length: usize) {
        debug_assert!(start + length <= self.len, "range end is out of range");
        update_range(self.words, start, length, true);
    }

    /// Clear the bits `[start, start + length)`.
    #[inline]
    pub fn clear(&mut self, start: usize, length: usize) {
        debug_assert!(start + length <= self.len, "range end is out of range");
        update_range(self.words, start, length, false);
    }

    /// Borrow the region for reading.
    pub fn as_region(&self) -> BitRegion<'_> {
        BitRegion::new(self.words, self.len)
    }
}

/// The index of the first bit in `[start, end)` that is set after `flip` is
/// XORed in: `0` finds a set bit, `u64::MAX` a clear one.
///
/// Range masking happens after the flip, so a clear-bit search never reports a
/// bit outside `[start, end)`.
fn next_bit(words: &[u64], start: usize, end: usize, flip: u64) -> Option<usize> {
    if start >= end {
        return None;
    }

    let last_word = (end - 1) / 64;
    let mut word_index = start / 64;
    // Clear the bits below `start`; `start % 64` is always < 64, so this shift
    // can never overflow.
    let mut word = (words[word_index] ^ flip) & (u64::MAX << (start % 64));

    loop {
        if word_index == last_word {
            // Clear the bits at or beyond `end`. The argument is in `1..=64`
            // and `mask_range(0, 64)` is `u64::MAX`, so the full-word case
            // needs no special handling.
            word &= mask_range(0, end - last_word * 64);
        }
        if word != 0 {
            return Some(word_index * 64 + word.trailing_zeros() as usize);
        }
        if word_index == last_word {
            return None;
        }
        word_index += 1;
        word = words[word_index] ^ flip;
    }
}

/// Set or clear the bits `[start, start + length)` of `words`.
#[inline]
pub(crate) fn update_range(words: &mut [u64], start: usize, length: usize, set: bool) {
    if length == 0 {
        return;
    }

    let end = start + length;
    let first_word = start / 64;
    let last_word = (end - 1) / 64;
    let start_bit = start % 64;
    let end_bit = end - last_word * 64;

    if first_word == last_word {
        apply(&mut words[first_word], mask_range(start_bit, end_bit), set);
        return;
    }

    // First partial word: bits [start_bit, 64)
    apply(&mut words[first_word], mask_range(start_bit, 64), set);
    // Full middle words
    words[first_word + 1..last_word].fill(if set { u64::MAX } else { 0 });
    // Last partial word: bits [0, end_bit)
    apply(&mut words[last_word], mask_range(0, end_bit), set);
}

/// Set or clear the masked bits of a single word.
#[inline]
fn apply(word: &mut u64, mask: u64, set: bool) {
    if set {
        *word |= mask;
    } else {
        *word &= !mask;
    }
}

/// A bitmask for bits `[low, high)` within a single `u64` word.
#[inline]
fn mask_range(low: usize, high: usize) -> u64 {
    debug_assert!(low <= high, "low ({low}) must be <= high ({high})");
    debug_assert!(high <= 64, "high ({high}) must be <= 64");

    if low == high {
        return 0;
    }

    // Shift a full mask right to get `high - low` set bits, then position at `low`.
    (u64::MAX >> (64 - (high - low))) << low
}

#[cfg(test)]
mod tests {
    use super::*;
    use rand::{RngExt, SeedableRng, rngs::StdRng};

    /// A `len`-bit buffer with exactly `positions` set.
    fn words_with(len: usize, positions: &[usize]) -> Vec<u64> {
        let mut words = vec![0u64; len.div_ceil(64)];
        let mut region = BitRegionMut::new(&mut words, len);
        for &position in positions {
            region.mark(position, 1);
        }
        words
    }

    /// The set bits in `[start, end)`, one bit at a time.
    fn set_bits(region: BitRegion<'_>, start: usize, end: usize) -> Vec<usize> {
        (start..end).filter(|&p| region.get(p)).collect()
    }

    #[test]
    fn mask_range_covers_empty_partial_and_full_words() {
        assert_eq!(mask_range(3, 3), 0);
        assert_eq!(mask_range(2, 5), 0b11100);
        assert_eq!(mask_range(0, 64), u64::MAX);
    }

    #[test]
    fn mark_sets_exactly_the_requested_range() {
        for length in [63, 64, 65] {
            for start in 0..128 {
                let mut words = vec![0u64; 3];
                let mut region = BitRegionMut::new(&mut words, 192);
                region.mark(start, length);
                assert_eq!(
                    set_bits(region.as_region(), 0, 192),
                    (start..start + length).collect::<Vec<_>>(),
                    "mark({start}, {length})"
                );
            }
        }
    }

    #[test]
    fn clear_undoes_mark_across_word_boundaries() {
        let mut words = vec![0u64; 3];
        let mut region = BitRegionMut::new(&mut words, 192);

        region.mark(0, 192);
        region.clear(60, 70); // spans all three words
        assert_eq!(region.as_region().count_ones(), 192 - 70);
        assert_eq!(region.as_region().next_clear_bit(0, 192), Some(60));

        region.mark(60, 70);
        assert_eq!(region.as_region().count_ones(), 192);
    }

    #[test]
    fn padding_of_the_last_word_is_never_reported() {
        // 10 bits in a 64-bit word: the 54 padding bits are clear but outside
        // the region.
        let words = words_with(10, &(0..10).collect::<Vec<_>>());
        let region = BitRegion::new(&words, 10);

        assert_eq!(region.count_ones(), 10);
        assert_eq!(region.count_zeros(), 0);
        assert_eq!(region.next_clear_bit(0, 10), None);
    }

    #[test]
    fn iter_ones_lists_the_set_positions() {
        let positions = [0, 63, 64, 130];
        let words = words_with(192, &positions);
        let region = BitRegion::new(&words, 192);

        assert_eq!(region.iter_ones().collect::<Vec<_>>(), positions);
        assert_eq!(region.count_zeros(), 192 - positions.len());
    }

    #[test]
    fn next_set_bit_matches_bit_by_bit_scan() {
        let mut rng = StdRng::seed_from_u64(20250819);
        const BITS: usize = 200;

        for _ in 0..50 {
            let positions: Vec<usize> = (0..BITS).filter(|_| rng.random::<u8>() < 64).collect();
            let words = words_with(BITS, &positions);
            let region = BitRegion::new(&words, BITS);

            for _ in 0..20 {
                let a = rng.random::<u32>() as usize % (BITS + 1);
                let b = rng.random::<u32>() as usize % (BITS + 1);
                let (start, end) = (a.min(b), a.max(b));

                let mut found = Vec::new();
                let mut at = start;
                while let Some(position) = region.next_set_bit(at, end) {
                    found.push(position);
                    at = position + 1;
                }
                assert_eq!(
                    found,
                    set_bits(region, start, end),
                    "range [{start}, {end}) over {positions:?}"
                );
            }
        }
    }

    #[test]
    fn next_clear_bit_matches_bit_by_bit_scan() {
        let mut rng = StdRng::seed_from_u64(20250819);
        const BITS: usize = 200;

        for _ in 0..50 {
            // Skewed dense so that long set runs — the case the splitter hits on
            // a template — actually occur.
            let positions: Vec<usize> = (0..BITS).filter(|_| rng.random::<u8>() < 200).collect();
            let words = words_with(BITS, &positions);
            let region = BitRegion::new(&words, BITS);

            for _ in 0..20 {
                let a = rng.random::<u32>() as usize % (BITS + 1);
                let b = rng.random::<u32>() as usize % (BITS + 1);
                let (start, end) = (a.min(b), a.max(b));

                assert_eq!(
                    region.next_clear_bit(start, end),
                    (start..end).find(|&p| !region.get(p)),
                    "range [{start}, {end}) over {positions:?}"
                );
            }
        }
    }
}
