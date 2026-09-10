//! Classification of ignored fingerprint values.
//!
//! A fingerprint value is ignored when it occurs in the supplied template or
//! when it occurs in more than `max_file_count` distinct files. [`classify`]
//! turns that into the positions each file's fingerprint array loses, which is
//! both what the analysis needs and what the report exports.

use crate::winnowing::fingerprints::Fingerprint;
use dolos_core::IgnoredPositions;
use std::collections::HashMap;
use std::ops::Range;

/// Per-fingerprint statistics gathered while classifying.
#[derive(Default)]
struct Entry {
    /// `file + 1` of the most recent file this value was counted for, or `0`
    /// when it has not been counted yet.
    last_seen: usize,
    /// Number of distinct files containing this value.
    file_count: usize,
    /// The value occurs in the template or in more files than the cap allows.
    is_ignored: bool,
}

/// Classify the ignored parts in the fingerprints of `sequences`.
///
/// * `sequences` — the fingerprint sequences of the files being compared.
/// * `template` — template fingerprint sequences; every fingerprint occurring
///   in them is ignored.
/// * `max_file_count` — a fingerprint occurring in strictly more than this many
///   distinct files is ignored. `None` disables the cap.
pub fn classify(
    sequences: &[Vec<Fingerprint>],
    template: &[Vec<Fingerprint>],
    max_file_count: Option<usize>,
) -> IgnoredPositions {
    let mut entries: HashMap<Fingerprint, Entry> = HashMap::new();

    // insert template entries
    for &template_fingerprint in template.iter().flatten() {
        entries.entry(template_fingerprint).or_default().is_ignored = true;
    }

    // Count the number of distinct files each fingerprint occurs in if a cap is given.
    if let Some(max) = max_file_count {
        for (file, sequence) in sequences.iter().enumerate() {
            // `file + 1` keeps `0` free as "not counted yet"
            let stamp = file + 1;
            for &fingerprint in sequence {
                let entry = entries.entry(fingerprint).or_default();
                if entry.last_seen != stamp {
                    entry.last_seen = stamp;
                    entry.file_count += 1;
                    entry.is_ignored |= entry.file_count > max;
                }
            }
        }
    }

    let per_file = sequences
        .iter()
        .map(|sequence| ignored_ranges(sequence, &entries))
        .collect();

    IgnoredPositions::new(per_file)
}

/// The maximal ranges of `sequence` whose fingerprints are ignored.
fn ignored_ranges(
    sequence: &[Fingerprint],
    entries: &HashMap<Fingerprint, Entry>,
) -> Vec<Range<usize>> {
    let mut ranges: Vec<Range<usize>> = Vec::new();

    for (position, fingerprint) in sequence.iter().enumerate() {
        if !entries
            .get(fingerprint)
            .is_some_and(|entry| entry.is_ignored)
        {
            continue;
        }
        // Extend the previous range when this position continues it.
        match ranges.last_mut() {
            Some(last) if last.end == position => last.end = position + 1,
            _ => ranges.push(position..position + 1),
        }
    }

    ranges
}

#[cfg(test)]
mod tests {
    use super::*;
    use dolos_core::{AnalysisOptions, Match};

    /// Build fingerprint sequences from strings, one byte per fingerprint.
    fn seqs(sequences: &[&str]) -> Vec<Vec<Fingerprint>> {
        sequences
            .iter()
            .map(|s| s.bytes().map(|b| b as Fingerprint).collect())
            .collect()
    }

    /// The ignored positions of every file, as a plain nested `Vec`.
    fn marked(ignored: &IgnoredPositions, count: usize) -> Vec<Vec<usize>> {
        (0..count)
            .map(|file| ignored.ranges(file).iter().flat_map(Clone::clone).collect())
            .collect()
    }

    // ── Document frequency ────────────────────────────────────────────

    #[test]
    fn a_fingerprint_in_at_most_max_files_is_kept() {
        let files = seqs(&["XA", "XB", "YC", "YD"]);
        let ignored = classify(&files, &[], Some(2));
        assert!(marked(&ignored, files.len()).iter().all(Vec::is_empty));
    }

    #[test]
    fn a_fingerprint_in_more_than_max_files_is_ignored() {
        let files = seqs(&["XA", "XB", "XC", "YD"]);
        let ignored = classify(&files, &[], Some(2));
        assert_eq!(
            marked(&ignored, files.len()),
            vec![vec![0], vec![0], vec![0], vec![]]
        );
    }

    // ── Template ───────────────────────────────────────────────────

    #[test]
    fn template_fingerprints_are_ignored() {
        // Z occurs in a single file, so no cap would ever catch it; repeats, in
        // the template and in the input alike, change nothing.
        let files = seqs(&["ZZA", "B"]);
        let ignored = classify(&files, &seqs(&["Z"]), None);
        assert_eq!(marked(&ignored, files.len()), vec![vec![0, 1], vec![]]);
    }

    #[test]
    fn a_template_that_matches_nothing_ignores_nothing() {
        let files = seqs(&["ABC", "ABC"]);
        let ignored = classify(&files, &seqs(&["Z"]), None);
        assert!(marked(&ignored, files.len()).iter().all(Vec::is_empty));
    }

    // ── Shapes ────────────────────────────────────────────────────────

    /// Adjacent ignored positions become one range, separated ones do not.
    #[test]
    fn ignored_positions_are_merged_into_maximal_ranges() {
        let files = seqs(&["ABXXCDX", "AB"]);
        let ignored = classify(&files, &seqs(&["X"]), None);
        assert_eq!(ignored.ranges(0), [2..4, 6..7]);
    }

    #[test]
    fn empty_sequences_are_handled() {
        let files = seqs(&["", "AB", ""]);
        let ignored = classify(&files, &seqs(&["A"]), None);
        assert_eq!(marked(&ignored, files.len()), vec![vec![], vec![0], vec![]]);
    }

    #[test]
    fn no_input_at_all_is_handled() {
        // A template without a single input file marks nothing.
        let ignored = classify(&[], &seqs(&["ABC"]), Some(1));
        assert!(ignored.ranges(0).is_empty());
    }

    // ── Replaying a classification ────────────────────────────────────

    /// Six files. One block occurs in five of them, so the cap of four ignores
    /// it; another occurs in three, so it survives and produces matches. The
    /// rest of each file is unique to it.
    fn corpus() -> Vec<Vec<Fingerprint>> {
        let common: Vec<Fingerprint> = (100..108).collect();
        let shared: Vec<Fingerprint> = (200..208).collect();

        (0..6)
            .map(|file| {
                let mut sequence: Vec<Fingerprint> = (0..6).map(|i| 1000 + file * 10 + i).collect();
                if file < 5 {
                    sequence.extend(&common);
                }
                if file < 3 {
                    sequence.extend(&shared);
                }
                sequence
            })
            .collect()
    }

    fn options() -> AnalysisOptions {
        AnalysisOptions { min_match_length: 3, keep_matches: true }
    }

    /// Matches in a fixed order: tree traversal order depends on the whole
    /// corpus, so a rerun finds the same matches in a different order.
    fn sorted(matches: &[Match]) -> Vec<(usize, usize, usize)> {
        let mut sorted: Vec<_> = matches
            .iter()
            .map(|m| (m.left_start, m.right_start, m.length))
            .collect();
        sorted.sort_unstable();
        sorted
    }

    /// A pair analysed on its own, with the ignored ranges of the full run,
    /// finds the same metrics and matches as that run.
    ///
    /// This is what lets a viewer recompute one pair from the report: the
    /// frequency cap is corpus-wide, so it cannot be applied again to two
    /// files and has to arrive as the ignored ranges instead.
    #[test]
    fn a_pair_replayed_from_its_ranges_reproduces_the_full_run() {
        let files = corpus();
        let ignored = classify(&files, &[], Some(4));
        let full = dolos_core::analyze(&files, &ignored, &options());
        let full_matches = full.matches.as_ref().expect("matches are kept");

        // The cap must bite without swallowing the corpus, or this proves nothing.
        let marked: usize = (0..files.len())
            .map(|file| ignored.ranges(file).iter().map(Range::len).sum::<usize>())
            .sum();
        let total: usize = files.iter().map(Vec::len).sum();
        assert!((1..total).contains(&marked), "ignored {marked} of {total}");

        let mut pairs_with_matches = 0;
        for (left, right, metrics) in full.metrics.iter_pairs() {
            let pair = vec![files[left].clone(), files[right].clone()];
            let replayed = IgnoredPositions::new(vec![
                ignored.ranges(left).to_vec(),
                ignored.ranges(right).to_vec(),
            ]);

            let rerun = dolos_core::analyze(&pair, &replayed, &options());

            assert_eq!(
                rerun.metrics.get(0, 1),
                metrics,
                "metrics differ for ({left}, {right})"
            );
            assert_eq!(
                sorted(rerun.matches.as_ref().unwrap().get(0, 1)),
                sorted(full_matches.get(left, right)),
                "matches differ for ({left}, {right})"
            );
            pairs_with_matches += usize::from(!full_matches.get(left, right).is_empty());
        }

        // Files 0, 1 and 2 share a block the cap keeps, so those three pairs match.
        assert_eq!(pairs_with_matches, 3);
    }

    /// Without the ranges the same rerun disagrees: on two files a corpus-wide
    /// cap can never trigger.
    #[test]
    fn a_blind_pair_rerun_loses_the_frequency_cap() {
        let files = corpus();
        let pair = vec![files[0].clone(), files[1].clone()];

        let full = dolos_core::analyze(&files, &classify(&files, &[], Some(4)), &options());
        let blind = dolos_core::analyze(&pair, &classify(&pair, &[], Some(4)), &options());

        // Blind, the block shared by five files occurs in two of two and is kept.
        assert!(
            blind.metrics.get(0, 1).similarity > full.metrics.get(0, 1).similarity,
            "the cap changed nothing"
        );
    }
}
