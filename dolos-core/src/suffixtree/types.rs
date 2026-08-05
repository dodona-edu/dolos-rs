use crate::Symbol;
use crate::collections::pair_array::PairArray;

/// Sentinel symbol used to mark the end of sequences.
pub const SENTINEL_SYMBOL: Symbol = usize::MAX;

/// Index of a node in the arena part of the tree.
pub type NodeIndex = usize;

/// Represents a starting position of a match in a sequence.
///
/// Internal to the suffix-tree module; public consumers use [`Match`] which
/// stores only the normalized start offsets.
#[derive(Debug, Clone)]
pub struct StartPosition {
    /// Index of the sequence this position belongs to.
    pub sequence_index: usize,
    /// Offset within the sequence where the match starts.
    pub start: usize,
}

/// A maximal exact match between two positions in (possibly different) sequences.
///
/// `left_start` and `right_start` are offsets into the left (smaller-index)
/// and right (larger-index) sequence respectively. The owning [`PairArray`]
/// tracks *which* sequences the pair refers to.
#[derive(Debug, Clone, PartialEq)]
pub struct Match {
    /// Start offset in the left sequence.
    pub left_start: usize,
    /// Start offset in the right sequence.
    pub right_start: usize,
    /// Number of consecutive matching symbols.
    pub length: usize,
    /// Whether this match comes from an ignored or too-common substring.
    ///
    /// Ignored matches are excluded from similarity and `longest_fragment`
    /// metrics but are still stored so callers can inspect or visualise them.
    pub ignored: bool,
}

/// Per-pair metrics produced by the suffix-tree analysis.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct PairMetrics {
    /// Jaccard-style similarity: `(overlap_left + overlap_right) / (total_left + total_right)`.
    pub similarity: f64,
    /// Total number of symbols in the left sequence.
    pub total_left: usize,
    /// Total number of symbols in the right sequence.
    pub total_right: usize,
    /// Number of symbols in the left sequence covered by at least one match.
    pub overlap_left: usize,
    /// Number of symbols in the right sequence covered by at least one match.
    pub overlap_right: usize,
    /// Length of the longest common substring (in symbols).
    pub longest_fragment: usize,
}

/// Result of the suffix-tree analysis containing all per-pair metrics.
#[derive(Debug)]
pub struct AnalysisResult {
    /// All per-pair metrics (similarity, totals, overlaps, longest fragment).
    pub metrics: PairArray<PairMetrics>,
    /// Raw matches from the suffix tree (consumed during report construction).
    pub matches: Option<PairArray<Vec<Match>>>,
}
