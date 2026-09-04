use crate::ignore::IgnoredFingerprints;
use crate::suffixtree::maximal_match::MaximalMatchAnalyzer;
use crate::suffixtree::node::Node;
use crate::suffixtree::tree_builder::UkkonenBuilder;
use crate::suffixtree::types::AnalysisResult;
use crate::suffixtree::types::SymbolType;

/// A generalized suffix tree implementation.
#[derive(Debug, PartialEq)]
pub struct SuffixTree {
    /// Arena containing all nodes in the tree. The root is always at index 0.
    pub arena: Vec<Node>,
}

impl SuffixTree {
    /// Creates a new `SuffixTree` from the given sequences, building it immediately.
    pub fn build(sequences: &[Vec<SymbolType>]) -> Self {
        let mut tree = SuffixTree { arena: vec![Node::create_root()] };
        UkkonenBuilder::new().add_sequences(sequences, &mut tree);
        tree
    }

    /// Run maximal-match analysis on this suffix tree, returning pairwise
    /// similarity and longest-fragment results.
    ///
    /// * `sequences` — fingerprint sequences for the files being analyzed.
    /// * `ignored` — which fingerprint positions are ignored; matches are split
    ///   at those positions before being recorded.
    /// * `min_match_length` — minimum shared substring length to record as a match.
    /// * `keep_fragments` — when `true`, raw matches are retained for fragment resolution.
    pub fn analyze(
        &self,
        sequences: &[Vec<SymbolType>],
        ignored: &IgnoredFingerprints,
        min_match_length: usize,
        keep_fragments: bool,
    ) -> AnalysisResult {
        MaximalMatchAnalyzer::new(self, sequences, ignored, min_match_length, keep_fragments)
            .analyze()
    }
}

#[cfg(test)]
pub mod suffixtree_test_utils {
    use crate::suffixtree::tree::SuffixTree;
    use crate::suffixtree::types::SymbolType;

    pub fn str_to_nodes(s: &str) -> Vec<SymbolType> {
        s.as_bytes().iter().map(|&b| b as SymbolType).collect()
    }

    fn search_pattern(
        tree: &SuffixTree,
        sequences: &[Vec<SymbolType>],
        pattern: &[SymbolType],
    ) -> Option<usize> {
        if pattern.is_empty() {
            return Some(0);
        }

        let (mut node_index, mut edge_offset) = (0, 0);
        for &symbol in pattern {
            let node = &tree.arena[node_index];
            if edge_offset < node.range.length() {
                if sequences[node.range.sequence_index][node.range.start + edge_offset] != symbol {
                    return None;
                }
                edge_offset += 1;
                continue;
            }

            node_index = *node.get_child(symbol)?;
            edge_offset = 1;
        }

        Some(node_index)
    }

    pub fn tree_contains(
        tree: &SuffixTree,
        sequences: &[Vec<SymbolType>],
        pattern: &[SymbolType],
    ) -> bool {
        search_pattern(tree, sequences, pattern).is_some()
    }

    pub fn tree_all_suffix_indices(
        tree: &SuffixTree,
        sequences: &[Vec<SymbolType>],
        pattern: &[SymbolType],
    ) -> Vec<usize> {
        let Some(end_node) = search_pattern(tree, sequences, pattern) else {
            return vec![];
        };

        let mut suffix_indices_list = Vec::new();
        let mut stack = vec![end_node];
        while let Some(current) = stack.pop() {
            let node = &tree.arena[current];
            match (&node.sequence_indices, &node.children) {
                (Some(sequence_indices), _) => {
                    suffix_indices_list.extend(sequence_indices.iter().copied())
                }
                (None, Some(children)) => stack.extend(children.values().copied()),
                (None, None) => unreachable!("Node must have either sequence indices or children"),
            }
        }

        suffix_indices_list
    }

    pub fn test_all_substrings(tree: &SuffixTree, sequences: &[Vec<SymbolType>]) {
        for (i, sequence) in sequences.iter().enumerate() {
            for start in 0..sequence.len() {
                for end in start + 1..=sequence.len() {
                    assert!(tree_contains(tree, sequences, &sequence[start..end]));
                    let vec = tree_all_suffix_indices(tree, sequences, &sequence[start..end]);
                    assert!(vec.contains(&i));
                }
            }
        }
    }
}

#[cfg(test)]
mod tests_build_single_sequence {
    use crate::suffixtree::node::{Node, Range};
    use crate::suffixtree::tree::SuffixTree;
    use crate::suffixtree::tree::suffixtree_test_utils::{str_to_nodes, test_all_substrings};
    use crate::suffixtree::types::{SENTINEL_SYMBOL, SymbolType};
    use std::collections::{HashMap, HashSet};

    #[test]
    fn test_small() {
        let sequences = vec![str_to_nodes("ABCD")];
        let tree = SuffixTree::build(&sequences);
        let control_tree = SuffixTree {
            #[rustfmt::skip]
            arena: vec![
                Node::new(Range::new(0, 0, 0), None, Some(HashMap::from([(65, 1), (66, 2), (67, 3), (68, 4), (SENTINEL_SYMBOL, 5)])), None, None),
                Node::new(Range::new(0, 4, 0), Some(0), None, None, Some(HashSet::from([0]))),
                Node::new(Range::new(1, 4, 0), Some(0), None, None, Some(HashSet::from([0]))),
                Node::new(Range::new(2, 4, 0), Some(0), None, None, Some(HashSet::from([0]))),
                Node::new(Range::new(3, 4, 0), Some(0), None, None, Some(HashSet::from([0]))),
                Node::new(Range::new(4, 4, 0), Some(0), None, None, Some(HashSet::from([0]))),
            ],
        };
        assert_eq!(tree, control_tree);
    }

    #[test]
    fn test_single_sequence() {
        let sequences = vec![str_to_nodes("ACACACGT")];
        let tree = SuffixTree::build(&sequences);
        let control_tree = SuffixTree {
            #[rustfmt::skip]
            arena: vec![
                Node::new(Range::new(0, 0, 0), None, Some(HashMap::from([(SENTINEL_SYMBOL, 13), (65, 7), (67, 9), (71, 11), (84, 12)])), None, None),
                Node::new(Range::new(4, 8, 0), Some(3), None, None, Some(HashSet::from([0]))),
                Node::new(Range::new(4, 8, 0), Some(5), None, None, Some(HashSet::from([0]))),
                Node::new(Range::new(2, 4, 0), Some(7), Some(HashMap::from([(65, 1), (b'G' as SymbolType, 4)])), Some(5), None),
                Node::new(Range::new(6, 8, 0), Some(3), None, None, Some(HashSet::from([0]))),
                Node::new(Range::new(2, 4, 0), Some(9), Some(HashMap::from([(65, 2), (b'G' as SymbolType, 6)])), Some(7), None),
                Node::new(Range::new(6, 8, 0), Some(5), None, None, Some(HashSet::from([0]))),
                Node::new(Range::new(0, 2, 0), Some(0), Some(HashMap::from([(65, 3), (b'G' as SymbolType, 8)])), Some(9), None),
                Node::new(Range::new(6, 8, 0), Some(7), None, None, Some(HashSet::from([0]))),
                Node::new(Range::new(1, 2, 0), Some(0), Some(HashMap::from([(65, 5), (b'G' as SymbolType, 10)])), Some(0), None),
                Node::new(Range::new(6, 8, 0), Some(9), None, None, Some(HashSet::from([0]))),
                Node::new(Range::new(6, 8, 0), Some(0), None, None, Some(HashSet::from([0]))),
                Node::new(Range::new(7, 8, 0), Some(0), None, None, Some(HashSet::from([0]))),
                Node::new(Range::new(8, 8, 0), Some(0), None, None, Some(HashSet::from([0]))),
            ],
        };
        assert_eq!(tree, control_tree);
    }

    #[test]
    fn test_large_alphabet() {
        let sequences = vec![(11500..12000).map(|i| i as SymbolType).collect()];
        let tree = SuffixTree::build(&sequences);
        test_all_substrings(&tree, &sequences);
    }
}

#[cfg(test)]
mod tests_build_multiple_sequences {
    use crate::suffixtree::node::{Node, Range};
    use crate::suffixtree::tree::SuffixTree;
    use crate::suffixtree::tree::suffixtree_test_utils::{str_to_nodes, test_all_substrings};
    use crate::suffixtree::types::{SENTINEL_SYMBOL, SymbolType};
    use rand::{RngExt, SeedableRng, rngs::StdRng};
    use std::collections::{HashMap, HashSet};

    #[test]
    fn test_two_non_overlapping_sequences() {
        let sequences = vec![str_to_nodes("ABC"), str_to_nodes("DEF")];
        let tree = SuffixTree::build(&sequences);
        let control_tree = SuffixTree {
            #[rustfmt::skip]
            arena: vec![
                Node::new(Range::new(0, 0, 0), None, Some(HashMap::from([(65, 1), (66, 2), (67, 3), (68, 5), (69, 6), (70, 7), (SENTINEL_SYMBOL, 4)])), None, None),
                Node::new(Range::new(0, 3, 0), Some(0), None, None, Some(HashSet::from([0]))),
                Node::new(Range::new(1, 3, 0), Some(0), None, None, Some(HashSet::from([0]))),
                Node::new(Range::new(2, 3, 0), Some(0), None, None, Some(HashSet::from([0]))),
                Node::new(Range::new(3, 3, 0), Some(0), None, None, Some(HashSet::from([0, 1]))),
                Node::new(Range::new(0, 3, 1), Some(0), None, None, Some(HashSet::from([1]))),
                Node::new(Range::new(1, 3, 1), Some(0), None, None, Some(HashSet::from([1]))),
                Node::new(Range::new(2, 3, 1), Some(0), None, None, Some(HashSet::from([1]))),
            ],
        };
        assert_eq!(tree, control_tree);
    }

    #[test]
    fn test_two_overlapping_begin_sequences() {
        let sequences = vec![str_to_nodes("XYAB"), str_to_nodes("XYCD")];
        let tree = SuffixTree::build(&sequences);
        let control_tree = SuffixTree {
            #[rustfmt::skip]
            arena: vec![
                Node::new(Range::new(0, 0, 0), None, Some(HashMap::from([(65, 3), (66, 4), (67, 10), (68, 11), (88, 6), (89, 8), (SENTINEL_SYMBOL, 5)])), None, None),
                Node::new(Range::new(2, 4, 0), Some(6), None, None, Some(HashSet::from([0]))),
                Node::new(Range::new(2, 4, 0), Some(8), None, None, Some(HashSet::from([0]))),
                Node::new(Range::new(2, 4, 0), Some(0), None, None, Some(HashSet::from([0]))),
                Node::new(Range::new(3, 4, 0), Some(0), None, None, Some(HashSet::from([0]))),
                Node::new(Range::new(4, 4, 0), Some(0), None, None, Some(HashSet::from([0, 1]))),
                Node::new(Range::new(0, 2, 0), Some(0), Some(HashMap::from([(65, 1), (67, 7)])), Some(8), None),
                Node::new(Range::new(2, 4, 1), Some(6), None, None, Some(HashSet::from([1]))),
                Node::new(Range::new(1, 2, 0), Some(0), Some(HashMap::from([(65, 2), (67, 9)])), Some(0), None),
                Node::new(Range::new(2, 4, 1), Some(8), None, None, Some(HashSet::from([1]))),
                Node::new(Range::new(2, 4, 1), Some(0), None, None, Some(HashSet::from([1]))),
                Node::new(Range::new(3, 4, 1), Some(0), None, None, Some(HashSet::from([1]))),
            ],
        };
        assert_eq!(tree, control_tree);
    }

    #[test]
    fn test_two_overlapping_end_sequences() {
        let sequences = vec![str_to_nodes("ABXY"), str_to_nodes("CDXY")];
        let tree = SuffixTree::build(&sequences);
        let control_tree = SuffixTree {
            #[rustfmt::skip]
            arena: vec![
                Node::new(Range::new(0, 0, 0), None, Some(HashMap::from([(65, 1), (66, 2), (67, 6), (68, 7), (88, 3), (89, 4), (SENTINEL_SYMBOL, 5)])), None, None),
                Node::new(Range::new(0, 4, 0), Some(0), None, None, Some(HashSet::from([0]))),
                Node::new(Range::new(1, 4, 0), Some(0), None, None, Some(HashSet::from([0]))),
                Node::new(Range::new(2, 4, 0), Some(0), None, None, Some(HashSet::from([0, 1]))),
                Node::new(Range::new(3, 4, 0), Some(0), None, None, Some(HashSet::from([0, 1]))),
                Node::new(Range::new(4, 4, 0), Some(0), None, None, Some(HashSet::from([0, 1]))),
                Node::new(Range::new(0, 4, 1), Some(0), None, None, Some(HashSet::from([1]))),
                Node::new(Range::new(1, 4, 1), Some(0), None, None, Some(HashSet::from([1]))),
            ],
        };
        assert_eq!(tree, control_tree);
    }

    #[test]
    fn test_multiple_sequences() {
        let sequences = vec![
            str_to_nodes("MISSISSIPPI"),
            str_to_nodes("BANANA"),
            str_to_nodes("BANASSIPPI"),
        ];
        let tree = SuffixTree::build(&sequences);
        test_all_substrings(&tree, &sequences);
    }

    #[test]
    fn test_large_random() {
        let mut rng = StdRng::seed_from_u64(42);
        let sequences: Vec<Vec<SymbolType>> = (0..50)
            .map(|_| {
                (0..50)
                    .map(|_| (rng.random::<u8>() % 10 + 65) as SymbolType)
                    .collect()
            })
            .collect();
        let tree = SuffixTree::build(&sequences);
        test_all_substrings(&tree, &sequences);
    }
}

#[cfg(test)]
mod tests_analysis {
    use crate::ignore::IgnoredFingerprints;
    use crate::suffixtree::tree::SuffixTree;
    use crate::suffixtree::tree::suffixtree_test_utils::str_to_nodes;
    use crate::suffixtree::types::{AnalysisResult, SymbolType};
    use std::ops::Range;

    fn analyze(sequences: &[Vec<SymbolType>], min_match_length: usize) -> AnalysisResult {
        analyze_ignoring(sequences, &[], None, min_match_length).0
    }

    /// Analyze with an ignore configuration, keeping the raw matches so that the
    /// tests can check every recorded position against the mask.
    ///
    /// Asserts on the way out that no recorded match covers an ignored position.
    fn analyze_ignoring(
        sequences: &[Vec<SymbolType>],
        boilerplate: &[Vec<SymbolType>],
        max_file_count: Option<usize>,
        min_match_length: usize,
    ) -> (AnalysisResult, IgnoredFingerprints) {
        let ignored = crate::ignore::classify(sequences, boilerplate, max_file_count);
        let tree = SuffixTree::build(sequences);
        let result = tree.analyze(sequences, &ignored, min_match_length, true);
        assert_no_recorded_match_is_ignored(&result, &ignored);
        (result, ignored)
    }

    /// Assert that no stored match covers an ignored fingerprint.
    ///
    /// A match is ignore-free exactly when it is one single usable run.
    fn assert_no_recorded_match_is_ignored(result: &AnalysisResult, ignored: &IgnoredFingerprints) {
        let matches = result.matches.as_ref().expect("fragments are kept");
        for (left, right, pair_matches) in matches.iter_pairs() {
            for m in pair_matches {
                assert!(m.length > 0, "a zero-length match must never be stored");
                for (file, start) in [(left, m.left_start), (right, m.right_start)] {
                    let runs: Vec<Range<usize>> =
                        ignored.usable_runs(file, start..start + m.length).collect();
                    assert_eq!(
                        runs,
                        vec![start..start + m.length],
                        "match {m:?} covers an ignored position of file {file}"
                    );
                }
            }
        }
    }

    /// The usable runs of `file` over its whole length.
    fn usable_runs(ignored: &IgnoredFingerprints, file: usize, length: usize) -> Vec<Range<usize>> {
        ignored.usable_runs(file, 0..length).collect()
    }

    /// The stored matches of one pair as `(left_start, right_start, length)`,
    /// sorted so the assertions do not depend on traversal order.
    fn pair_matches(result: &AnalysisResult, i: usize, j: usize) -> Vec<(usize, usize, usize)> {
        let mut found: Vec<(usize, usize, usize)> = result
            .matches
            .as_ref()
            .expect("fragments are kept")
            .get(i, j)
            .iter()
            .map(|m| (m.left_start, m.right_start, m.length))
            .collect();
        found.sort_unstable();
        found
    }

    #[test]
    fn test_identical_sequences() {
        let sequences = vec![str_to_nodes("ABC"), str_to_nodes("ABC")];
        let result = analyze(&sequences, 1);
        let m = result.metrics.get(0, 1);
        assert_eq!(m.similarity, 1.0);
        assert_eq!(m.longest_fragment, 3);
        assert_eq!(m.total_left, 3);
        assert_eq!(m.total_right, 3);
        assert_eq!(m.overlap_left, 3);
        assert_eq!(m.overlap_right, 3);
    }

    #[test]
    fn test_non_overlapping_sequences() {
        let sequences = vec![str_to_nodes("ABC"), str_to_nodes("DEF")];
        let result = analyze(&sequences, 1);
        let m = result.metrics.get(0, 1);
        assert_eq!(m.similarity, 0.0);
        assert_eq!(m.longest_fragment, 0);
        assert_eq!(m.overlap_left, 0);
        assert_eq!(m.overlap_right, 0);
    }

    #[test]
    fn test_partial_overlapping_sequences() {
        // "ABC" is the only shared fragment
        let sequences = vec![str_to_nodes("ABCDEF"), str_to_nodes("XYZABC")];
        let result = analyze(&sequences, 1);
        let m = result.metrics.get(0, 1);
        assert_eq!(m.longest_fragment, 3);
        assert_eq!(m.similarity, 0.5);
        assert_eq!(m.total_left, 6);
        assert_eq!(m.total_right, 6);
        assert_eq!(m.overlap_left, 3);
        assert_eq!(m.overlap_right, 3);
    }

    #[test]
    fn test_three_sequence() {
        let sequences = vec![
            str_to_nodes("ABCD"),
            str_to_nodes("ABCE"),
            str_to_nodes("XYZW"),
        ];
        let result = analyze(&sequences, 1);

        // Sequences 0 and 1 share "ABC"
        let m01 = result.metrics.get(0, 1);
        assert_eq!(m01.longest_fragment, 3);
        assert_eq!(m01.similarity, 0.75);

        // Sequences 0 and 2 share nothing
        let m02 = result.metrics.get(0, 2);
        assert_eq!(m02.longest_fragment, 0);
        assert_eq!(m02.similarity, 0.0);

        // Sequences 1 and 2 share nothing
        let m12 = result.metrics.get(1, 2);
        assert_eq!(m12.longest_fragment, 0);
        assert_eq!(m12.similarity, 0.0);
    }

    #[test]
    fn test_matches_below_min_length() {
        // "ABC" has length 3, which is below min_match_length 5 → not counted
        let sequences = vec![str_to_nodes("ABCDEF"), str_to_nodes("XYZABC")];
        assert_eq!(analyze(&sequences, 5).metrics.get(0, 1).longest_fragment, 0);
    }

    // ── Ignoring ──────────────────────────────────────────────────────

    #[test]
    fn test_ignored_value_in_the_middle_splits_the_match() {
        // The single raw match "AXB"/"AXB" (length 3 at 0,0) is cut in two by
        // the ignored X at position 1.
        let sequences = vec![str_to_nodes("AXB"), str_to_nodes("AXB")];
        let (result, ignored) = analyze_ignoring(&sequences, &[str_to_nodes("X")], None, 1);

        assert_eq!(usable_runs(&ignored, 0, 3), vec![0..1, 2..3]);
        assert_eq!(pair_matches(&result, 0, 1), vec![(0, 0, 1), (2, 2, 1)]);

        let m = result.metrics.get(0, 1);
        assert_eq!(m.longest_fragment, 1);
        assert_eq!((m.overlap_left, m.overlap_right), (2, 2));
        assert_eq!((m.total_left, m.total_right), (2, 2));
        assert_eq!(m.similarity, 1.0);
    }

    #[test]
    fn test_ignored_suffix_truncates_the_match() {
        // C and D are ignored, so the length-4 match keeps only its "AB" prefix.
        let sequences = vec![str_to_nodes("ABCD"), str_to_nodes("ABCD")];
        let (result, ignored) = analyze_ignoring(&sequences, &[str_to_nodes("CD")], None, 1);

        assert_eq!(usable_runs(&ignored, 0, 4), vec![0..2]);
        assert_eq!(pair_matches(&result, 0, 1), vec![(0, 0, 2)]);

        let m = result.metrics.get(0, 1);
        assert_eq!(m.longest_fragment, 2);
        assert_eq!((m.overlap_left, m.overlap_right), (2, 2));
        assert_eq!((m.total_left, m.total_right), (2, 2));
        assert_eq!(m.similarity, 1.0);
    }

    #[test]
    fn test_core_without_a_node_of_its_own_is_recovered() {
        // The two "BBC" suffixes meet at one leaf at depth 3, so the tree has
        // no node for "BB"; only splitting the depth-4 match recovers it.
        let sequences = vec![str_to_nodes("ABBC"), str_to_nodes("ABBC")];
        let (result, ignored) =
            analyze_ignoring(&sequences, &[str_to_nodes("A"), str_to_nodes("C")], None, 1);

        assert_eq!(usable_runs(&ignored, 0, 4), vec![1..3]);
        // The length-2 core, plus the two single-B matches that the tree finds
        // at the "B" node (left symbols A and B differ, so they are maximal).
        assert_eq!(
            pair_matches(&result, 0, 1),
            vec![(1, 1, 2), (1, 2, 1), (2, 1, 1)]
        );

        let m = result.metrics.get(0, 1);
        assert_eq!(m.longest_fragment, 2);
        assert_eq!((m.overlap_left, m.overlap_right), (2, 2));
        assert_eq!((m.total_left, m.total_right), (2, 2));
        assert_eq!(m.similarity, 1.0);
    }

    #[test]
    fn test_min_match_length_applies_per_run() {
        // Five fingerprints match, but the ignored X leaves two runs of two.
        let sequences = vec![str_to_nodes("ABXBC"), str_to_nodes("ABXBC")];
        let (result, _) = analyze_ignoring(&sequences, &[str_to_nodes("X")], None, 3);

        assert!(pair_matches(&result, 0, 1).is_empty());

        let m = result.metrics.get(0, 1);
        assert_eq!(m.longest_fragment, 0);
        assert_eq!((m.overlap_left, m.overlap_right), (0, 0));
        assert_eq!((m.total_left, m.total_right), (4, 4));
        assert_eq!(m.similarity, 0.0);
    }

    #[test]
    fn test_fully_ignored_files_have_no_similarity() {
        // Both denominators collapse to zero.
        let sequences = vec![str_to_nodes("XX"), str_to_nodes("X")];
        let (result, ignored) = analyze_ignoring(&sequences, &[str_to_nodes("X")], None, 1);

        assert_eq!(ignored.effective_length(0), 0);
        assert_eq!(ignored.effective_length(1), 0);
        assert!(pair_matches(&result, 0, 1).is_empty());

        let m = result.metrics.get(0, 1);
        assert_eq!((m.total_left, m.total_right), (0, 0));
        assert_eq!((m.overlap_left, m.overlap_right), (0, 0));
        assert_eq!(m.similarity, 0.0);
    }

    #[test]
    fn test_frequency_cap_decides_per_corpus() {
        // X occurs in two of three files: at a cap of 2 it is still usable, so
        // all three of file 0's X's match file 1's single X.
        let sequences = vec![str_to_nodes("XXXA"), str_to_nodes("XB"), str_to_nodes("C")];
        let (result, ignored) = analyze_ignoring(&sequences, &[], Some(2), 1);

        assert_eq!(usable_runs(&ignored, 0, 4), vec![0..4]);
        let m = result.metrics.get(0, 1);
        assert_eq!(m.longest_fragment, 1);
        assert_eq!((m.overlap_left, m.overlap_right), (3, 1));
        assert_eq!((m.total_left, m.total_right), (4, 2));
        assert_eq!(m.similarity, 4.0 / 6.0);

        // Adding X to the third file pushes it over the cap; the same pair now
        // shares nothing, and only the non-X fingerprints remain in the totals.
        let sequences = vec![str_to_nodes("XXXA"), str_to_nodes("XB"), str_to_nodes("CX")];
        let (result, ignored) = analyze_ignoring(&sequences, &[], Some(2), 1);

        assert_eq!(usable_runs(&ignored, 0, 4), vec![3..4]);
        assert!(pair_matches(&result, 0, 1).is_empty());
        let m = result.metrics.get(0, 1);
        assert_eq!(m.longest_fragment, 0);
        assert_eq!((m.overlap_left, m.overlap_right), (0, 0));
        assert_eq!((m.total_left, m.total_right), (1, 1));
        assert_eq!(m.similarity, 0.0);
    }
}
