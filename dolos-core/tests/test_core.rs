use dolos_core::{AnalysisOptions, AnalysisResult, Match, PairMetrics, analyze};

fn options(min_match_length: usize, keep_matches: bool) -> AnalysisOptions {
    AnalysisOptions { min_match_length, keep_matches }
}

fn metrics(result: &AnalysisResult, i: usize, j: usize) -> &PairMetrics {
    result.metrics.get(i, j)
}

fn matches(result: &AnalysisResult, i: usize, j: usize) -> &[Match] {
    result.matches.as_ref().unwrap().get(i, j)
}

// The run `2 3 4` covers 3 of the 5 symbols on each side: 6 of 10.
#[test]
fn shared_run_gives_the_metrics_of_the_pair() {
    let sequences = vec![vec![1, 2, 3, 4, 5], vec![9, 9, 2, 3, 4]];
    let result = analyze(&sequences, None, &options(1, false)).unwrap();

    let expected = PairMetrics {
        similarity: 0.6,
        total_left: 5,
        total_right: 5,
        overlap_left: 3,
        overlap_right: 3,
        longest_match: 3,
    };
    assert_eq!(metrics(&result, 0, 1), &expected);
}

// Sequence 0 is the left side of the pair, also when the caller asks for (1, 0).
#[test]
fn match_starts_in_the_lower_indexed_sequence_first() {
    let sequences = vec![vec![7, 1, 2, 3], vec![1, 2, 3]];
    let result = analyze(&sequences, None, &options(1, true)).unwrap();

    let expected = Match { left_start: 1, right_start: 0, length: 3 };
    assert_eq!(matches(&result, 1, 0), [expected]);
}

// The longest match of each pair shows that the indices name the right pair.
#[test]
fn every_pair_is_reported_once_in_index_order() {
    let sequences = vec![vec![1, 2, 3], vec![1, 2], vec![5], vec![2, 3]];
    let result = analyze(&sequences, None, &options(1, false)).unwrap();

    let longest: Vec<(usize, usize, usize)> = result
        .metrics
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
fn keep_matches_off_leaves_out_the_matches_but_not_the_metrics() {
    let sequences = vec![vec![1, 2, 3, 4], vec![9, 2, 3, 4]];
    let with = analyze(&sequences, None, &options(1, true)).unwrap();
    let without = analyze(&sequences, None, &options(1, false)).unwrap();

    assert!(without.matches.is_none());
    assert_eq!(metrics(&without, 0, 1), metrics(&with, 0, 1));
}

#[test]
fn empty_sequences_have_no_similarity() {
    let sequences = vec![vec![], vec![]];
    let result = analyze(&sequences, None, &options(1, false)).unwrap();

    assert_eq!(metrics(&result, 0, 1), &PairMetrics::default());
}

// The ranges cover positions 0, 1 and 2, so 1 of the 4 positions is left.
#[test]
fn overlapping_ignored_ranges_count_each_position_once() {
    let sequences = vec![vec![1, 2, 3, 4], vec![5, 6]];
    let ignored = [vec![0..2, 1..3, 3..3], vec![]];
    let result = analyze(&sequences, Some(&ignored), &options(1, false)).unwrap();

    assert_eq!(metrics(&result, 0, 1).total_left, 1);
}
