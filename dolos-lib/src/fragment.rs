use crate::config::FragmentSortBy;
use crate::winnowing::region::Region;
use dolos_core::{Match, PairArray};
use std::cmp::Reverse;

/// A resolved code fragment: the source regions that correspond to one
/// maximal exact match in both files.
#[derive(Debug, Clone)]
pub struct Fragment {
    /// The region in the left file covered by this match.
    pub left_region: Region,
    /// The region in the right file covered by this match.
    pub right_region: Region,
    /// Number of fingerprints in this match.
    pub fingerprint_count: usize,
    /// Whether this fragment comes from an ignored or too-common substring.
    ///
    /// Ignored fragments are excluded from similarity and `longest_fragment`
    /// metrics but are still reported so callers can inspect or visualise them.
    pub ignored: bool,
}

impl Fragment {
    /// Resolve a raw [`Match`] into a [`Fragment`] using per-file fingerprint
    /// location arrays.
    pub fn resolve(m: &Match, left_locs: &[Region], right_locs: &[Region]) -> Self {
        Fragment {
            left_region: Region::span(
                &left_locs[m.left_start],
                &left_locs[m.left_start + m.length - 1],
            ),
            right_region: Region::span(
                &right_locs[m.right_start],
                &right_locs[m.right_start + m.length - 1],
            ),
            fingerprint_count: m.length,
            ignored: m.ignored,
        }
    }
}

/// Resolve raw matches into [`Fragment`] lists using the per-file fingerprint
/// regions, sorted according to `sort_by`.
///
/// `locations` is indexed by file id, exactly like the sequences that were
/// handed to the analysis.
pub(crate) fn resolve_fragments(
    raw_matches: PairArray<Vec<Match>>,
    locations: &[Vec<Region>],
    sort_by: &Option<FragmentSortBy>,
) -> PairArray<Vec<Fragment>> {
    let mut fragments = PairArray::new(raw_matches.size(), Vec::new());

    for (left, right, pair_matches) in raw_matches.iter_pairs() {
        let mut resolved: Vec<Fragment> = pair_matches
            .iter()
            .map(|m| Fragment::resolve(m, &locations[left], &locations[right]))
            .collect();
        sort_fragments(&mut resolved, sort_by);
        fragments.set(left, right, resolved);
    }

    fragments
}

/// Sort fragments in-place according to `sort_by`.
///
/// `FileOrder` and `None` both sort by left-file source position (row, column).
fn sort_fragments(fragments: &mut [Fragment], sort_by: &Option<FragmentSortBy>) {
    match sort_by {
        Some(FragmentSortBy::KgramsAscending) => {
            fragments.sort_by_key(|f| f.fingerprint_count);
        }
        Some(FragmentSortBy::KgramsDescending) => {
            fragments.sort_by_key(|f| Reverse(f.fingerprint_count));
        }
        Some(FragmentSortBy::FileOrder) | None => {
            fragments.sort_by_key(|f| {
                (
                    f.left_region.start_point.row,
                    f.left_region.start_point.column,
                )
            });
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::winnowing::region::Point;
    use rstest::rstest;

    #[test]
    fn resolve_match_to_fragment() {
        let left_locs = vec![
            Region::new(Point::new(10, 0), Point::new(10, 10)),
            Region::new(Point::new(11, 0), Point::new(12, 12)),
        ];
        let right_locs = vec![
            Region::new(Point::new(20, 0), Point::new(20, 10)),
            Region::new(Point::new(21, 0), Point::new(22, 12)),
        ];
        let m = Match { left_start: 0, right_start: 0, length: 2, ignored: false };

        let frag = Fragment::resolve(&m, &left_locs, &right_locs);

        let expected_left_region = Region::new(Point::new(10, 0), Point::new(12, 12));
        let expected_right_region = Region::new(Point::new(20, 0), Point::new(22, 12));

        assert_eq!(frag.left_region, expected_left_region);
        assert_eq!(frag.right_region, expected_right_region);
        assert_eq!(frag.fingerprint_count, 2);
        assert!(!frag.ignored);
    }

    #[test]
    fn resolve_propagates_ignored_flag() {
        let left_locs = vec![Region::new(Point::new(0, 0), Point::new(0, 5))];
        let right_locs = vec![Region::new(Point::new(1, 0), Point::new(1, 5))];
        let m = Match { left_start: 0, right_start: 0, length: 1, ignored: true };

        let frag = Fragment::resolve(&m, &left_locs, &right_locs);

        assert!(frag.ignored);
    }

    #[test]
    fn test_resolve_fragments() {
        let locations = vec![
            vec![
                Region::new(Point::new(0, 0), Point::new(0, 5)),
                Region::new(Point::new(1, 0), Point::new(1, 5)),
            ],
            vec![
                Region::new(Point::new(10, 0), Point::new(10, 5)),
                Region::new(Point::new(11, 0), Point::new(11, 5)),
            ],
        ];

        let mut raw_matches: PairArray<Vec<Match>> = PairArray::new(2, Vec::new());
        raw_matches.set(
            0,
            1,
            vec![
                Match { left_start: 0, right_start: 0, length: 2, ignored: false },
                Match { left_start: 1, right_start: 1, length: 1, ignored: true },
            ],
        );

        let fragments = resolve_fragments(raw_matches, &locations, &None);
        let frags = fragments.get(0, 1);

        assert_eq!(frags.len(), 2);
        let f = &frags[0];
        assert_eq!(f.fingerprint_count, 2);
        // Spans from start of first loc to end of last loc
        assert_eq!(f.left_region.start_point, Point::new(0, 0));
        assert_eq!(f.left_region.end_point, Point::new(1, 5));
        assert_eq!(f.right_region.start_point, Point::new(10, 0));
        assert_eq!(f.right_region.end_point, Point::new(11, 5));

        let f = &frags[1];
        assert_eq!(f.left_region.start_point, Point::new(1, 0));
        assert_eq!(f.left_region.end_point, Point::new(1, 5));
        assert_eq!(f.right_region.start_point, Point::new(11, 0));
        assert_eq!(f.right_region.end_point, Point::new(11, 5));

        // The ignored flag must survive resolution.
        assert!(!frags[0].ignored);
        assert!(frags[1].ignored);
    }

    #[rstest]
    #[case::default(None, [1, 2])]
    #[case::file_order(Some(FragmentSortBy::FileOrder), [1, 2])]
    #[case::kgrams_ascending(Some(FragmentSortBy::KgramsAscending), [1, 2])]
    #[case::kgrams_descending(Some(FragmentSortBy::KgramsDescending), [2, 1])]
    fn test_fragment_sort(#[case] sort_by: Option<FragmentSortBy>, #[case] expected: [usize; 2]) {
        let locations = vec![
            vec![
                Region::new(Point::new(0, 0), Point::new(0, 5)),
                Region::new(Point::new(1, 0), Point::new(1, 5)),
                Region::new(Point::new(2, 0), Point::new(2, 5)),
            ],
            vec![
                Region::new(Point::new(10, 0), Point::new(10, 5)),
                Region::new(Point::new(11, 0), Point::new(11, 5)),
                Region::new(Point::new(12, 0), Point::new(12, 5)),
            ],
        ];

        let mut raw_matches: PairArray<Vec<Match>> = PairArray::new(2, Vec::new());
        raw_matches.set(
            0,
            1,
            vec![
                Match { left_start: 0, right_start: 0, length: 1, ignored: false },
                Match { left_start: 1, right_start: 1, length: 2, ignored: false },
            ],
        );

        let fragments = resolve_fragments(raw_matches, &locations, &sort_by);

        let counts: Vec<usize> = fragments
            .get(0, 1)
            .iter()
            .map(|f| f.fingerprint_count)
            .collect();
        assert_eq!(counts, expected);
    }
}
