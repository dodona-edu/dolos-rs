use dolos::{Dolos, DolosConfig, PairSortBy, Report};
use rstest::rstest;
use std::path::PathBuf;

// ── Fixture files ─────────────────────────────────────────────────────────────

const SAMPLE12: &[&str] = &["fixtures/sample1.js", "fixtures/sample2.js"];
const SAMPLE123: &[&str] = &[
    "fixtures/sample1.js",
    "fixtures/sample2.js",
    "fixtures/sample3.js",
];

// ── Helpers ───────────────────────────────────────────────────────────────────

fn to_path_buf(names: &[&str]) -> Vec<PathBuf> {
    names.iter().map(PathBuf::from).collect()
}

fn report(files: &[&str], config: DolosConfig) -> Report {
    Dolos::new(to_path_buf(files), config)
        .unwrap()
        .build_report()
}

fn pair_sim(files: &[&str], config: DolosConfig) -> f64 {
    let report = report(files, config);

    report
        .pairs
        .iter()
        .find(|p| {
            let lf = p
                .left_file
                .relative_path
                .file_name()
                .and_then(|n| n.to_str());
            let rf = p
                .right_file
                .relative_path
                .file_name()
                .and_then(|n| n.to_str());

            (lf == Some("sample1.js") && rf == Some("sample2.js"))
                || (lf == Some("sample2.js") && rf == Some("sample1.js"))
        })
        .unwrap_or_else(|| panic!("no pair (sample1.js, sample2.js) in results"))
        .metrics
        .similarity
}

fn is_sorted_desc<T, K: PartialOrd>(items: &[T], key: impl Fn(&T) -> K) -> bool {
    items.windows(2).all(|w| key(&w[0]) >= key(&w[1]))
}

// ── Parameterized similarity tests ────────────────────────────────────────────

/// Verifies the similarity produced by various configurations. Each row in the
/// table is the single source of truth for that config's expected output.
#[rstest]
#[case::default(SAMPLE12, DolosConfig::default(), 0.4803921568627451)]
#[case::kgram_length(SAMPLE12, DolosConfig::builder().kgram_length(10).build().unwrap(), 0.6842105263157895)]
#[case::kgrams_in_window(SAMPLE12, DolosConfig::builder().kgrams_in_window(5).build().unwrap(), 0.479020979020979)]
#[case::include_comments(SAMPLE12, DolosConfig::builder().include_comments(true).build().unwrap(), 0.47619047619047616)]
#[case::min_length_match(SAMPLE12, DolosConfig::builder().min_length_match(10).build().unwrap(), 0.20588235294117646)]
#[case::max_fingerprint_count(SAMPLE123, DolosConfig::builder().max_fingerprint_count(2).build().unwrap(), 0.03636363636363636)]
#[case::max_fingerprint_percentage(SAMPLE123, DolosConfig::builder().max_fingerprint_percentage(0.7).build().unwrap(), 0.03636363636363636)]
#[case::ignore(SAMPLE12, DolosConfig::builder().ignore("fixtures/sample_ignore.js").build().unwrap(), 0.45077720207253885)]
fn test_similarities(
    #[case] files: &[&str],
    #[case] config: DolosConfig,
    #[case] expected_sim: f64,
) {
    assert_eq!(pair_sim(files, config), expected_sim);
}

// ── Fragment presence ─────────────────────────────────────────────────────────

#[test]
fn test_two_files_have_fragments() {
    let report = report(SAMPLE12, DolosConfig::default());

    for pair in &report.pairs {
        assert!(
            pair.fragments.is_some(),
            "fragments should be present when exactly 2 files are given"
        );
    }
}

#[test]
fn test_three_files_no_fragments() {
    let report = report(SAMPLE123, DolosConfig::default());

    for pair in &report.pairs {
        assert!(
            pair.fragments.is_none(),
            "fragments should be None when more than 2 files are given"
        );
    }
}

// ── Pair sorting ──────────────────────────────────────────────────────────────

/// The library sorts `report.pairs` in descending order for each `sort_by` mode.
#[rstest]
#[case::similarity(PairSortBy::Similarity)]
#[case::total_overlap(PairSortBy::TotalOverlap)]
#[case::longest_fragment(PairSortBy::LongestFragment)]
fn test_sort_by(#[case] sort_by: PairSortBy) {
    let report = report(
        SAMPLE123,
        DolosConfig::builder().sort_by(sort_by).build().unwrap(),
    );

    let ordered = match sort_by {
        PairSortBy::Similarity => is_sorted_desc(&report.pairs, |p| p.metrics.similarity),
        PairSortBy::TotalOverlap => is_sorted_desc(&report.pairs, |p| {
            p.metrics.overlap_left + p.metrics.overlap_right
        }),
        PairSortBy::LongestFragment => {
            is_sorted_desc(&report.pairs, |p| p.metrics.longest_fragment)
        }
    };
    assert!(ordered, "pairs not in descending order for {sort_by:?}");
}

// ── Core data export ──────────────────────────────────────────────────────────

#[test]
fn test_core_data_absent_by_default() {
    let report = report(SAMPLE12, DolosConfig::default());

    for file in &report.files {
        assert!(
            file.core_data.is_none(),
            "core data must not be exported without include_core_data"
        );
    }
}

#[test]
fn test_core_data_present_with_flag() {
    let config = DolosConfig::builder()
        .include_core_data(true)
        .build()
        .unwrap();
    let report = report(SAMPLE123, config);

    assert_eq!(report.files.len(), 3);
    for file in &report.files {
        let core_data = file
            .core_data
            .as_ref()
            .expect("core data is present when include_core_data is set");
        assert!(!core_data.fingerprints.is_empty());
        assert_eq!(core_data.fingerprints.len(), core_data.regions.len());
    }
}

// ── Fragment ignored flags ────────────────────────────────────────────────────

/// A run with a template file must flag the template-covered fragments as
/// ignored while keeping genuine matches unflagged.
#[test]
fn test_fragments_carry_ignored_flags() {
    let config = DolosConfig::builder()
        .ignore("fixtures/sample_ignore.js")
        .build()
        .unwrap();
    let report = report(SAMPLE12, config);

    let fragments = report.pairs[0].fragments.as_ref().unwrap();
    assert!(fragments.iter().any(|f| f.ignored));
    assert!(fragments.iter().any(|f| !f.ignored));
}

// ── Input modes ───────────────────────────────────────────────────────────────

/// All input modes (directory, CSV manifest, zip, tar, tar.gz, tar.bz2) must
/// produce the same similarity as passing the two loose files directly.
#[test]
fn test_input_modes() {
    let base_sim = pair_sim(SAMPLE12, DolosConfig::default());

    let inputs = [
        "fixtures/reader",
        "fixtures/reader/info.csv",
        "fixtures/reader.zip",
        "fixtures/reader.tar",
        "fixtures/reader.tar.gz",
        "fixtures/reader.tar.bz2",
    ];

    for input in inputs {
        let sim = pair_sim(&[input], DolosConfig::default());
        assert_eq!(sim, base_sim, "{input}: similarity must match baseline");
    }
}
