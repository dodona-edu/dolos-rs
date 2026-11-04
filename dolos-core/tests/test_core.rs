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

// The suffix tree collects the matches in a `HashMap`, so without the sort the
// order changes from one run to the next.
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

/// A random pair over a small alphabet, with some positions ignored.
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
        let sequences = vec![sequence(rng), sequence(rng)];
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

    /// Whether position `i` of the left sequence and position `j` of the right
    /// sequence hold the same symbol, and neither is ignored.
    fn usable(&self, i: usize, j: usize) -> bool {
        self.sequences[0][i] == self.sequences[1][j] && !self.ignored[0][i] && !self.ignored[1][j]
    }

    /// Every run of usable positions on a diagonal that cannot be extended and
    /// is long enough, without the runs that another run contains.
    fn brute_force_matches(&self) -> Vec<Match> {
        let (left, right) = (self.sequences[0].len(), self.sequences[1].len());
        let mut runs = Vec::new();
        for i in 0..left {
            for j in 0..right {
                if !self.usable(i, j) || (i > 0 && j > 0 && self.usable(i - 1, j - 1)) {
                    continue;
                }
                let mut length = 0;
                while i + length < left && j + length < right && self.usable(i + length, j + length)
                {
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

        assert_eq!(
            matches(&result, 0, 1),
            case.brute_force_matches(),
            "sequences {:?}, ignored {:?}, min_match_length {}",
            case.sequences,
            case.ignored,
            case.min_match_length
        );
    }
}

// A match that another match contains covers no new position and is not
// longer, so the remaining matches give the metrics of a run without matches.
#[test]
fn remaining_matches_give_the_metrics_on_random_sequences() {
    let mut rng = StdRng::seed_from_u64(117);
    for _ in 0..5000 {
        let case = RandomCase::new(&mut rng);
        let with_matches = case.analyze(true);
        let without_matches = case.analyze(false);
        let metrics = &without_matches.metrics()[(0, 1)];

        let found = matches(&with_matches, 0, 1);
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
            "sequences {:?}, ignored {:?}",
            case.sequences,
            case.ignored
        );
    }
}
