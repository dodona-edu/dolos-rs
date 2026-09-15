//! The JavaScript binding for [`crate::analyze`].
//!
//! The conversion and the pair lookups live in plain `impl` blocks, so they can
//! be tested on the native target. The `#[wasm_bindgen]` block only maps their
//! errors to [`JsError`], which exists on the wasm target alone.

use crate::{AnalysisOptions, AnalysisResult, Match, PairMetrics, Symbol};
use std::fmt;
use std::ops::Range;
use tsify::{Ts, Tsify};
use wasm_bindgen::prelude::*;

#[wasm_bindgen(typescript_custom_section)]
const INTERVAL: &str = r#"
/** A half-open range of positions, `[start, end)`. */
export type Interval = [start: number, end: number];
"#;

/// Why a pair lookup names no pair.
#[derive(Debug, PartialEq, Eq)]
pub enum PairError {
    /// Both indices name the same sequence.
    SameSequence(usize),
    /// An index is not below the number of sequences.
    OutOfRange { left: usize, right: usize, count: usize },
}

impl fmt::Display for PairError {
    fn fmt(&self, fmt: &mut fmt::Formatter) -> fmt::Result {
        match self {
            Self::SameSequence(index) => {
                write!(fmt, "sequence {index} does not form a pair with itself")
            }
            Self::OutOfRange { left, right, count } => write!(
                fmt,
                "pair ({left}, {right}) is out of range, the analysis has {count} sequences"
            ),
        }
    }
}

impl std::error::Error for PairError {}

/// The result of one `analyze` run.
///
/// It holds the analysis in WebAssembly memory and converts one pair per call.
/// Release it with `free()`, or declare it with `using`.
#[wasm_bindgen]
#[derive(Debug)]
pub struct Analysis(AnalysisResult);

impl Analysis {
    /// Build the suffix tree over `sequences` and analyse every pair.
    fn run(
        sequences: Vec<Vec<Symbol>>,
        ignored: Option<Vec<Vec<(usize, usize)>>>,
        options: AnalysisOptions,
    ) -> std::io::Result<Self> {
        let ignored: Option<Vec<Vec<Range<usize>>>> = ignored.map(|sequences| {
            sequences
                .into_iter()
                .map(|intervals| {
                    intervals
                        .into_iter()
                        .map(|(start, end)| start..end)
                        .collect()
                })
                .collect()
        });

        Ok(Self(crate::analyze(
            &sequences,
            ignored.as_deref(),
            &options,
        )?))
    }

    /// The metrics of the pair `(left, right)`.
    fn pair_metrics(&self, left: usize, right: usize) -> Result<PairMetrics, PairError> {
        self.check_pair(left, right)?;

        Ok(self.0.metrics.get(left, right).clone())
    }

    /// The matches of the pair `(left, right)`, or `None` when the run did not
    /// keep them.
    fn pair_matches(&self, left: usize, right: usize) -> Result<Option<&Vec<Match>>, PairError> {
        self.check_pair(left, right)?;

        Ok(self
            .0
            .matches
            .as_ref()
            .map(|matches| matches.get(left, right)))
    }

    /// Check that `(left, right)` names a pair, before [`PairArray`] asserts it.
    ///
    /// [`PairArray`]: crate::PairArray
    fn check_pair(&self, left: usize, right: usize) -> Result<(), PairError> {
        if left == right {
            return Err(PairError::SameSequence(left));
        }

        let count = self.0.metrics.item_count();
        if left >= count || right >= count {
            return Err(PairError::OutOfRange { left, right, count });
        }

        Ok(())
    }
}

#[wasm_bindgen]
impl Analysis {
    /// The number of sequences the analysis covers.
    #[wasm_bindgen(getter, js_name = sequenceCount)]
    pub fn sequence_count(&self) -> usize {
        self.0.metrics.item_count()
    }

    /// The number of pairs the analysis holds.
    #[wasm_bindgen(getter, js_name = pairCount)]
    pub fn pair_count(&self) -> usize {
        self.0.metrics.pair_count()
    }

    /// Whether the run kept the raw matches.
    #[wasm_bindgen(getter, js_name = hasMatches)]
    pub fn has_matches(&self) -> bool {
        self.0.matches.is_some()
    }

    /// The metrics of the pair `(left, right)`.
    ///
    /// @throws when `left` and `right` name no pair.
    pub fn metrics(&self, left: usize, right: usize) -> Result<Ts<PairMetrics>, JsError> {
        Ok(self.pair_metrics(left, right)?.into_ts()?)
    }

    /// The matches of the pair `(left, right)`.
    ///
    /// `undefined` when the run did not keep matches. Empty when the pair has
    /// none.
    ///
    /// @throws when `left` and `right` name no pair.
    #[wasm_bindgen(unchecked_return_type = "Match[] | undefined")]
    pub fn matches(&self, left: usize, right: usize) -> Result<JsValue, JsError> {
        let matches = self.pair_matches(left, right)?;

        Ok(serde_wasm_bindgen::to_value(&matches)?)
    }
}

/// Analyse every pair of `sequences`.
///
/// `ignored` holds the positions to leave out, one list of intervals per
/// sequence, in the same order as `sequences`. Pass `null` to ignore nothing.
/// An empty array is not the same thing: it covers no sequence, and is
/// rejected.
///
/// @throws when an argument does not match its type, or when the input is
/// unusable. There must be at least two sequences, `ignored` must cover every
/// one of them, an interval must stay within its sequence, no sequence may hold
/// the reserved value 4294967295, and `minMatchLength` must be at least 1.
#[wasm_bindgen]
pub fn analyze(
    #[wasm_bindgen(unchecked_param_type = "number[][]")] sequences: JsValue,
    #[wasm_bindgen(unchecked_param_type = "Interval[][] | null")] ignored: JsValue,
    options: Option<Ts<AnalysisOptions>>,
) -> Result<Analysis, JsError> {
    let sequences: Vec<Vec<Symbol>> = serde_wasm_bindgen::from_value(sequences)?;
    let ignored: Option<Vec<Vec<(usize, usize)>>> = serde_wasm_bindgen::from_value(ignored)?;
    let options = match options {
        Some(options) => options.to_rust()?,
        None => AnalysisOptions::default(),
    };

    Ok(Analysis::run(sequences, ignored, options)?)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// One sequence per string, one symbol per byte.
    fn sequences(inputs: &[&str]) -> Vec<Vec<Symbol>> {
        inputs
            .iter()
            .map(|s| s.bytes().map(Symbol::from).collect())
            .collect()
    }

    fn options(min_match_length: usize, keep_matches: bool) -> AnalysisOptions {
        AnalysisOptions { min_match_length, keep_matches }
    }

    /// Bad input is reported, not asserted: a panic would trap the module.
    #[test]
    fn run_rejects_an_invalid_input() {
        let with_sentinel = vec![vec![1, 2], vec![Symbol::MAX]];
        let error = Analysis::run(with_sentinel, None, options(1, false)).unwrap_err();

        assert_eq!(error.kind(), std::io::ErrorKind::InvalidInput);
    }

    /// Both failing index shapes reach the caller as an error.
    #[test]
    fn pair_lookups_reject_indices_that_name_no_pair() {
        let analysis = Analysis::run(sequences(&["ABC", "ABC"]), None, options(1, true)).unwrap();

        assert_eq!(analysis.pair_metrics(1, 1), Err(PairError::SameSequence(1)));
        assert_eq!(
            analysis.pair_matches(0, 5),
            Err(PairError::OutOfRange { left: 0, right: 5, count: 2 })
        );
    }

    /// The `[start, end)` intervals become ignored ranges, so they split a match.
    #[test]
    fn intervals_split_a_match() {
        let ignored = vec![vec![(2, 3)], vec![(2, 3)]];
        let analysis = Analysis::run(
            sequences(&["ABCDE", "ABCDE"]),
            Some(ignored),
            options(1, true),
        )
        .unwrap();

        assert_eq!(
            analysis.pair_matches(0, 1).unwrap().unwrap(),
            &vec![
                Match { left_start: 0, right_start: 0, length: 2 },
                Match { left_start: 3, right_start: 3, length: 2 },
            ]
        );
        // The ignored position drops out of the total.
        assert_eq!(analysis.pair_metrics(0, 1).unwrap().total_left, 4);
    }

    #[test]
    fn matches_are_absent_unless_the_run_keeps_them() {
        let kept = Analysis::run(sequences(&["ABC", "ABC"]), None, options(1, true)).unwrap();
        let dropped = Analysis::run(sequences(&["ABC", "ABC"]), None, options(1, false)).unwrap();

        assert!(kept.pair_matches(0, 1).unwrap().is_some());
        assert!(dropped.pair_matches(0, 1).unwrap().is_none());
    }

    /// An empty options object means a minimum length of 1 and no matches kept.
    #[test]
    fn defaults_apply_when_no_option_is_given() {
        let analysis =
            Analysis::run(sequences(&["AB", "AB"]), None, AnalysisOptions::default()).unwrap();

        assert_eq!(analysis.pair_metrics(0, 1).unwrap().longest_match, 2);
        assert!(analysis.pair_matches(0, 1).unwrap().is_none());
    }
}
