use crate::config::PairSortBy;
use crate::file::File;
use crate::fragment::Fragment;
use crate::metadata::Metadata;
use dolos_core::{PairArray, PairMetrics};
use std::cmp::Reverse;
use std::rc::Rc;

pub struct Pair {
    pub left_file: Rc<File>,
    pub right_file: Rc<File>,
    pub metrics: PairMetrics,
    /// Resolved source-line fragments, present when fragment storage was enabled.
    pub fragments: Option<Vec<Fragment>>,
}

pub struct Report {
    pub metadata: Metadata,
    pub files: Vec<Rc<File>>,
    pub pairs: Vec<Pair>,
}

impl Report {
    pub(crate) fn new(
        metrics: PairArray<PairMetrics>,
        mut frags: Option<PairArray<Vec<Fragment>>>,
        files: Vec<Rc<File>>,
        metadata: Metadata,
    ) -> Report {
        let mut pairs: Vec<Pair> = metrics
            .iter_pairs()
            .map(|(left, right, metric)| Pair {
                left_file: files[left].clone(),
                right_file: files[right].clone(),
                metrics: metric.clone(),
                fragments: frags
                    .as_mut()
                    .map(|f| std::mem::take(f.get_mut(left, right))),
            })
            .collect();

        sort_pairs(&mut pairs, &metadata.sort_by);

        Report { metadata, files, pairs }
    }
}

/// Sort pairs in-place according to `sort_by`, descending by the chosen metric.
///
/// When `sort_by` is `None` the natural index order is preserved.
fn sort_pairs(pairs: &mut [Pair], sort_by: &Option<PairSortBy>) {
    match sort_by {
        Some(PairSortBy::Similarity) => {
            pairs.sort_by(|a, b| {
                b.metrics
                    .similarity
                    .partial_cmp(&a.metrics.similarity)
                    .unwrap_or(std::cmp::Ordering::Equal)
            });
        }
        Some(PairSortBy::TotalOverlap) => {
            pairs.sort_by_key(|p| Reverse(p.metrics.overlap_left + p.metrics.overlap_right));
        }
        Some(PairSortBy::LongestFragment) => {
            pairs.sort_by_key(|p| Reverse(p.metrics.longest_fragment));
        }
        None => {}
    }
}
