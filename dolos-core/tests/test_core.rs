use dolos_core::{AnalysisOptions, AnalysisResult, Match, PairMetrics, analyze};
use rand::{RngExt, SeedableRng, rngs::StdRng};
use std::collections::HashSet;
use std::ops::Range;

fn matches(result: &AnalysisResult, i: usize, j: usize) -> &[Match] {
    &result.matches().unwrap()[(i, j)]
}

// The run `2 3 4` covers 3 of the 5 symbols on each side: 6 of 10.
#[test]
fn shared_run_gives_the_metrics_of_the_pair() {
    let sequences = vec![vec![1, 2, 3, 4, 5], vec![9, 9, 2, 3, 4]];
    let options = AnalysisOptions { min_match_length: 1, keep_matches: false };
    let result = analyze(&sequences, None, &options).unwrap();

    let expected = PairMetrics {
        similarity: 0.6,
        total_left: 5,
        total_right: 5,
        overlap_left: 3,
        overlap_right: 3,
        longest_match: 3,
    };
    assert_eq!(result.metrics()[(0, 1)], expected);
}

#[test]
fn reversed_pair_query_keeps_the_lower_index_on_the_left() {
    let sequences = vec![vec![7, 1, 2, 3], vec![1, 2, 3]];
    let options = AnalysisOptions { min_match_length: 1, keep_matches: true };
    let result = analyze(&sequences, None, &options).unwrap();

    let expected = Match { left_start: 1, right_start: 0, length: 3 };
    assert_eq!(matches(&result, 1, 0), [expected]);
}

// The longest match of each pair shows that the indices name the right pair.
#[test]
fn iter_pairs_yields_every_pair_once_in_index_order() {
    let sequences = vec![vec![1, 2, 3], vec![1, 2], vec![5], vec![2, 3]];
    let options = AnalysisOptions { min_match_length: 1, keep_matches: false };
    let result = analyze(&sequences, None, &options).unwrap();

    let longest: Vec<(usize, usize, usize)> = result
        .metrics()
        .iter_pairs()
        .map(|(i, j, m)| (i, j, m.longest_match))
        .collect();
    assert_eq!(
        longest,
        [
            (0, 1, 2),
            (0, 2, 0),
            (0, 3, 2),
            (1, 2, 0),
            (1, 3, 1),
            (2, 3, 0)
        ]
    );
}

#[test]
fn matches_come_sorted_by_position_in_every_run() {
    let sequences = vec![
        vec![1, 2, 3, 9, 4, 5, 6, 8, 7, 1, 2, 3, 0, 4, 5],
        vec![4, 5, 6, 7, 1, 2, 3, 1, 2, 3, 4, 5],
    ];
    let options = AnalysisOptions { min_match_length: 1, keep_matches: true };

    let expected: Vec<Match> = [
        (0, 4, 3),
        (0, 7, 3),
        (4, 0, 3),
        (4, 10, 2),
        (8, 3, 4),
        (9, 7, 3),
        (13, 0, 2),
        (13, 10, 2),
    ]
    .into_iter()
    .map(|(left_start, right_start, length)| Match { left_start, right_start, length })
    .collect();
    for _ in 0..2 {
        let result = analyze(&sequences, None, &options).unwrap();
        assert_eq!(matches(&result, 0, 1), expected);
    }
}

#[test]
fn keep_matches_off_leaves_out_the_matches() {
    let sequences = vec![vec![1, 2, 3, 4], vec![9, 2, 3, 4]];
    let options = AnalysisOptions { min_match_length: 1, keep_matches: false };
    let result = analyze(&sequences, None, &options).unwrap();

    assert!(result.matches().is_none());
}

#[test]
fn keep_matches_does_not_change_the_metrics() {
    let sequences = vec![vec![1, 2, 3, 4], vec![9, 2, 3, 4]];
    let with_options = AnalysisOptions { min_match_length: 1, keep_matches: true };
    let without_options = AnalysisOptions { min_match_length: 1, keep_matches: false };
    let with_matches = analyze(&sequences, None, &with_options).unwrap();
    let without_matches = analyze(&sequences, None, &without_options).unwrap();

    assert_eq!(
        without_matches.metrics()[(0, 1)],
        with_matches.metrics()[(0, 1)]
    );
}

#[test]
fn empty_sequences_have_no_similarity() {
    let sequences = vec![vec![], vec![]];
    let options = AnalysisOptions { min_match_length: 1, keep_matches: false };
    let result = analyze(&sequences, None, &options).unwrap();

    assert_eq!(result.metrics()[(0, 1)], PairMetrics::default());
}

// The ranges cover positions 0, 1 and 2, so 1 of the 4 positions is left.
#[test]
fn overlapping_ignored_ranges_count_each_position_once() {
    let sequences = vec![vec![1, 2, 3, 4], vec![5, 6]];
    let ignored = [vec![0..2, 1..3], vec![]];
    let options = AnalysisOptions { min_match_length: 1, keep_matches: false };
    let result = analyze(&sequences, Some(&ignored), &options).unwrap();

    assert_eq!(result.metrics()[(0, 1)].total_left, 1);
}

/// The pairs of the three sequences of a [`RandomCase`].
const PAIRS: [(usize, usize); 3] = [(0, 1), (0, 2), (1, 2)];

/// Three random sequences over a small alphabet, with some positions ignored.
struct RandomCase {
    sequences: Vec<Vec<usize>>,
    ignored: Vec<Vec<bool>>,
    min_match_length: usize,
}

impl RandomCase {
    fn new(rng: &mut StdRng) -> Self {
        let alphabet = rng.random_range(1..=3);
        let sequence = |rng: &mut StdRng| -> Vec<usize> {
            let length = rng.random_range(0..=12);
            (0..length).map(|_| rng.random_range(0..alphabet)).collect()
        };
        let sequences = vec![sequence(rng), sequence(rng), sequence(rng)];
        let ignored = sequences
            .iter()
            .map(|s| s.iter().map(|_| rng.random_range(0..8) == 0).collect())
            .collect();

        Self {
            sequences,
            ignored,
            min_match_length: rng.random_range(1..=3),
        }
    }

    fn analyze(&self, keep_matches: bool) -> AnalysisResult {
        let ranges: Vec<Vec<Range<usize>>> = self
            .ignored
            .iter()
            .map(|ignored| {
                ignored
                    .iter()
                    .enumerate()
                    .filter(|&(_, &is_ignored)| is_ignored)
                    .map(|(i, _)| i..i + 1)
                    .collect()
            })
            .collect();
        let options = AnalysisOptions { min_match_length: self.min_match_length, keep_matches };

        analyze(&self.sequences, Some(&ranges), &options).unwrap()
    }

    /// Whether position `i` of sequence `a` and position `j` of sequence `b`
    /// hold the same symbol, and neither is ignored.
    fn usable(&self, (a, i): (usize, usize), (b, j): (usize, usize)) -> bool {
        self.sequences[a][i] == self.sequences[b][j] && !self.ignored[a][i] && !self.ignored[b][j]
    }

    /// Every run of usable positions of sequences `a` and `b` on a diagonal that
    /// cannot be extended and is long enough, without the runs that another run
    /// contains.
    fn brute_force_matches(&self, a: usize, b: usize) -> Vec<Match> {
        let usable = |i, j| self.usable((a, i), (b, j));
        let (left, right) = (self.sequences[a].len(), self.sequences[b].len());
        let mut runs = Vec::new();
        for i in 0..left {
            for j in 0..right {
                if !usable(i, j) || (i > 0 && j > 0 && usable(i - 1, j - 1)) {
                    continue;
                }
                let mut length = 0;
                while i + length < left && j + length < right && usable(i + length, j + length) {
                    length += 1;
                }
                if length >= self.min_match_length {
                    runs.push(Match { left_start: i, right_start: j, length });
                }
            }
        }

        runs.iter()
            .filter(|m| !runs.iter().any(|n| n != *m && contains(n, m)))
            .copied()
            .collect()
    }
}

/// Whether `outer` contains `inner` in the left and in the right sequence.
fn contains(outer: &Match, inner: &Match) -> bool {
    outer.left_start <= inner.left_start
        && inner.left_start + inner.length <= outer.left_start + outer.length
        && outer.right_start <= inner.right_start
        && inner.right_start + inner.length <= outer.right_start + outer.length
}

#[test]
fn matches_equal_a_brute_force_search_on_random_sequences() {
    let mut rng = StdRng::seed_from_u64(117);
    for _ in 0..5000 {
        let case = RandomCase::new(&mut rng);
        let result = case.analyze(true);

        for (a, b) in PAIRS {
            assert_eq!(
                matches(&result, a, b),
                case.brute_force_matches(a, b),
                "pair ({a}, {b}), sequences {:?}, ignored {:?}, min_match_length {}",
                case.sequences,
                case.ignored,
                case.min_match_length
            );
        }
    }
}

// The kept matches cover the positions and give the longest match that the
// metrics of the same run without `keep_matches` report.
#[test]
fn remaining_matches_give_the_metrics_on_random_sequences() {
    let mut rng = StdRng::seed_from_u64(117);
    for _ in 0..5000 {
        let case = RandomCase::new(&mut rng);
        let with_matches = case.analyze(true);
        let without_matches = case.analyze(false);

        for (a, b) in PAIRS {
            let metrics = &without_matches.metrics()[(a, b)];
            let found = matches(&with_matches, a, b);
            let left: HashSet<usize> = found
                .iter()
                .flat_map(|m| m.left_start..m.left_start + m.length)
                .collect();
            let right: HashSet<usize> = found
                .iter()
                .flat_map(|m| m.right_start..m.right_start + m.length)
                .collect();
            let longest = found.iter().map(|m| m.length).max().unwrap_or(0);
            assert_eq!(
                (left.len(), right.len(), longest),
                (
                    metrics.overlap_left,
                    metrics.overlap_right,
                    metrics.longest_match
                ),
                "pair ({a}, {b}), sequences {:?}, ignored {:?}",
                case.sequences,
                case.ignored
            );
        }
    }
}
