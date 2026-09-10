//! The JavaScript binding for [`crate::analyze`].
//!
//! It lets a caller re-run one pair of an earlier analysis without the original
//! input — a report viewer recomputing the matches of a file pair in the
//! browser, for example, from the data the report exports.

use crate::AnalysisResult as CoreAnalysisResult;
use crate::ignore::IgnoredPositions;
use crate::{AnalysisOptions, Match, PairMetrics, Symbol};
use serde::{Deserialize, Serialize};
use tsify_next::Tsify;
use wasm_bindgen::prelude::*;

/// The sequences to compare, one inner array per sequence.
///
/// `Symbol` is a type alias, which tsify would render as the JavaScript
/// `Symbol` type, so the TypeScript type is spelled out here.
#[derive(Tsify, Deserialize)]
#[tsify(from_wasm_abi)]
#[serde(transparent)]
pub struct Sequences(#[tsify(type = "number[][]")] pub Vec<Vec<Symbol>>);

/// The ignored positions of each sequence, as `[start, length]` pairs. One
/// inner array per sequence, in the same order as [`Sequences`].
#[derive(Tsify, Deserialize)]
#[tsify(from_wasm_abi)]
#[serde(transparent)]
pub struct IgnoredIntervals(pub Vec<Vec<(usize, usize)>>);

/// Options for [`analyze`].
///
/// There is no frequency cap here. It counts occurrences over a whole corpus,
/// so on a subset it would silently do nothing; the ignored intervals already
/// carry its outcome.
#[derive(Tsify, Deserialize)]
#[tsify(from_wasm_abi)]
#[serde(rename_all = "camelCase")]
pub struct Options {
    pub min_match_length: usize,
}

/// The result for one unordered pair of sequences.
#[derive(Tsify, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PairResult {
    pub left_index: usize,
    pub right_index: usize,
    pub metrics: PairMetrics,
    pub matches: Vec<Match>,
}

/// The result of one [`analyze`] call: one entry per pair of sequences.
#[derive(Tsify, Serialize)]
#[tsify(into_wasm_abi)]
pub struct AnalysisResult {
    pub pairs: Vec<PairResult>,
}

/// Analyze `sequences` and return the matches and metrics of every pair.
///
/// `ignored` holds the ignored positions of each sequence, as the report
/// exports them. Pass an empty array to ignore nothing.
#[wasm_bindgen]
pub fn analyze(
    sequences: Sequences,
    ignored: IgnoredIntervals,
    options: Options,
) -> AnalysisResult {
    run(&sequences.0, &ignored.0, options.min_match_length)
}

/// The body of [`analyze`], callable from a native test.
fn run(
    sequences: &[Vec<Symbol>],
    ignored_intervals: &[Vec<(usize, usize)>],
    min_match_length: usize,
) -> AnalysisResult {
    // The analysis walks the tree's leaves and has none to walk without input.
    if sequences.is_empty() {
        return AnalysisResult { pairs: Vec::new() };
    }

    let ignored = IgnoredPositions::new(
        ignored_intervals
            .iter()
            .map(|runs| {
                runs.iter()
                    .map(|&(start, len)| start..start + len)
                    .collect()
            })
            .collect(),
    );

    let options = AnalysisOptions { min_match_length, keep_matches: true };
    let CoreAnalysisResult { metrics, matches } = crate::analyze(sequences, &ignored, &options);
    let mut matches = matches.expect("matches are kept");

    let pairs = metrics
        .iter_pairs()
        .map(|(left_index, right_index, metrics)| PairResult {
            left_index,
            right_index,
            metrics: metrics.clone(),
            matches: std::mem::take(matches.get_mut(left_index, right_index)),
        })
        .collect();

    AnalysisResult { pairs }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_utils::str_to_symbols;

    /// One sequence per string, one symbol per byte.
    fn to_sequences(inputs: &[&str]) -> Vec<Vec<Symbol>> {
        inputs.iter().map(|s| str_to_symbols(s)).collect()
    }

    /// Every pair gets an entry with its metrics and its matches.
    #[test]
    fn analyze_returns_one_entry_per_pair() {
        let result = run(&to_sequences(&["ABCD", "ABCD", "XY"]), &[], 2);

        assert_eq!(result.pairs.len(), 3);
        let indices: Vec<_> = result
            .pairs
            .iter()
            .map(|p| (p.left_index, p.right_index))
            .collect();
        assert_eq!(indices, vec![(0, 1), (0, 2), (1, 2)]);

        let ab = &result.pairs[0];
        assert_eq!(ab.metrics.longest_match, 4);
        assert_eq!(
            ab.matches,
            vec![Match { left_start: 0, right_start: 0, length: 4 }]
        );
        // "XY" shares nothing with "ABCD".
        assert!(result.pairs[1].matches.is_empty());
    }

    /// The ignored intervals split a match, exactly as a template does.
    #[test]
    fn ignored_intervals_split_a_match() {
        let sequences = to_sequences(&["ABCDE", "ABCDE"]);
        let ignored = vec![vec![(2, 1)], vec![(2, 1)]];

        let result = run(&sequences, &ignored, 1);
        let pair = &result.pairs[0];

        assert_eq!(
            pair.matches,
            vec![
                Match { left_start: 0, right_start: 0, length: 2 },
                Match { left_start: 3, right_start: 3, length: 2 },
            ]
        );
        assert_eq!(pair.metrics.longest_match, 2);
        // The ignored position drops out of the totals.
        assert_eq!(pair.metrics.total_left, 4);
    }

    /// One sequence forms no pair, and no input at all is not an error.
    #[test]
    fn fewer_than_two_sequences_give_no_pairs() {
        assert!(run(&to_sequences(&["ABC"]), &[], 1).pairs.is_empty());
        assert!(run(&[], &[], 1).pairs.is_empty());
    }
}
