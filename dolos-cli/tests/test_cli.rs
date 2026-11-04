use assert_cmd::Command;
use predicates::prelude::*;
use std::path::{Path, PathBuf};
use tempfile::TempDir;

const SAMPLE1: &str = "fixtures/sample1.js";
const SAMPLE2: &str = "fixtures/sample2.js";

fn dolos_run(files: &[&str], args: &[&str]) -> Command {
    let mut cmd = Command::cargo_bin("dolos").unwrap();
    cmd.arg("run").args(files).args(args);
    cmd
}

/// Run a CSV report of the two fixtures into a fresh directory and return that directory.
fn csv_report(tmp: &TempDir, extra: &[&str]) -> PathBuf {
    csv_report_of(tmp, &[SAMPLE1, SAMPLE2], extra)
}

/// Run a CSV report of `files` into a fresh directory and return that directory.
fn csv_report_of(tmp: &TempDir, files: &[&str], extra: &[&str]) -> PathBuf {
    let report_dir = tmp.path().join("report");
    #[rustfmt::skip]
    let args = [
        "-f", "csv",
        "-n", "report",
        "-o", report_dir.to_str().unwrap(),
    ];
    dolos_run(files, &[&args[..], extra].concat())
        .assert()
        .success();
    report_dir
}

/// Run a CSV report with one fingerprint per token of two copies of a line with non-ASCII text.
fn non_ascii_report(tmp: &TempDir) -> PathBuf {
    let line = r#"let s = "é😀"; let t = 1;"#;
    let paths = ["a.js", "b.js"].map(|name| tmp.path().join(name));
    for path in &paths {
        std::fs::write(path, format!("{line}\n")).unwrap();
    }
    let files = paths.each_ref().map(|path| path.to_str().unwrap());
    let args = ["-k", "1", "-w", "1", "--include-analysis-data"];
    csv_report_of(tmp, &files, &args)
}

fn assert_csv_header(path: &Path, expected: &[&str]) {
    assert!(path.exists(), "{} does not exist", path.display());
    let mut reader = csv::Reader::from_path(path).unwrap();
    assert_eq!(reader.headers().unwrap(), expected);
}

/// The rows of a CSV file, without the header.
fn read_rows(path: &Path) -> Vec<csv::StringRecord> {
    csv::Reader::from_path(path)
        .unwrap()
        .records()
        .map(Result::unwrap)
        .collect()
}

#[test]
fn terminal_output_shows_the_similarity() {
    dolos_run(&[SAMPLE1, SAMPLE2], &[])
        .assert()
        .success()
        .stdout(predicate::str::contains("sim:"));
}

#[test]
#[rustfmt::skip]
fn csv_output_writes_the_headers() {
    let tmp = TempDir::new().unwrap();
    // -o is the exact report directory; it must not exist yet.
    let report_dir = csv_report(&tmp, &[]);

    // Files are written directly into the given directory, no subfolder.
    assert_csv_header(&report_dir.join("metadata.csv"), &["property", "value"]);
    assert_csv_header(
        &report_dir.join("files.csv"),
        &["id", "path", "content", "fingerprints", "fingerprintRegions", "ignoredIntervals"],
    );
    assert_csv_header(
        &report_dir.join("pairs.csv"),
        &["id", "leftFileId", "rightFileId", "similarity", "longestFragment", "overlapLeft", "overlapRight", "totalLeft", "totalRight", "totalOverlap"],
    );
    assert_csv_header(
        &report_dir.join("fragments.csv"),
        &["leftFileId", "rightFileId", "leftRegion", "rightRegion", "fingerprintCount"],
    );
}

#[test]
fn pairs_csv_holds_the_pair_id_and_the_total_overlap() {
    let tmp = TempDir::new().unwrap();
    let report_dir = csv_report(&tmp, &[]);

    let rows = read_rows(&report_dir.join("pairs.csv"));
    assert_eq!(rows.len(), 1);
    let row = &rows[0];
    assert_eq!((&row[0], &row[1], &row[2]), ("0", "0", "1"));
    let number = |i: usize| row[i].parse::<usize>().unwrap();
    assert_eq!(number(9), number(5) + number(6));
}

#[test]
fn fragments_csv_writes_each_region_as_four_numbers() {
    let tmp = TempDir::new().unwrap();
    let report_dir = csv_report(&tmp, &[]);

    let region = |cell: &str| -> Vec<usize> {
        let numbers = cell.strip_prefix('[').unwrap().strip_suffix(']').unwrap();
        numbers.split(',').map(|n| n.parse().unwrap()).collect()
    };

    let rows = read_rows(&report_dir.join("fragments.csv"));
    assert!(!rows.is_empty());
    for row in rows {
        assert_eq!((&row[0], &row[1]), ("0", "1"));
        assert_eq!(region(&row[2]).len(), 4);
        assert_eq!(region(&row[3]).len(), 4);
    }
}

#[test]
fn fingerprint_regions_give_columns_in_utf16_code_units() {
    let tmp = TempDir::new().unwrap();
    let report_dir = non_ascii_report(&tmp);

    let rows = read_rows(&report_dir.join("files.csv"));
    // The second `let` starts at byte 18 and at UTF-16 code unit 15.
    assert!(rows[0][4].contains("[0,15,0,19]"), "{}", &rows[0][4]);
}

#[test]
fn fragment_regions_give_columns_in_utf16_code_units() {
    let tmp = TempDir::new().unwrap();
    let report_dir = non_ascii_report(&tmp);

    let columns = |region: &str| {
        let numbers: Vec<usize> = region
            .trim_matches(['[', ']'])
            .split(',')
            .map(|n| n.parse().unwrap())
            .collect();
        [numbers[1], numbers[3]]
    };

    let rows = read_rows(&report_dir.join("fragments.csv"));
    let max_column = rows
        .iter()
        .flat_map(|row| [columns(&row[2]), columns(&row[3])])
        .flatten()
        .max();
    // The line is 25 UTF-16 code units long, and 28 bytes.
    assert_eq!(max_column, Some(25));
}

#[test]
fn analysis_data_columns_are_empty_by_default() {
    let tmp = TempDir::new().unwrap();
    let report_dir = csv_report(&tmp, &[]);

    for row in read_rows(&report_dir.join("files.csv")) {
        assert_eq!(&row[3], "");
        assert_eq!(&row[4], "");
        assert_eq!(&row[5], "");
    }
}

#[test]
fn analysis_data_columns_hold_four_region_numbers_per_fingerprint() {
    let tmp = TempDir::new().unwrap();
    let report_dir = csv_report(&tmp, &["--include-analysis-data"]);

    // The fingerprint count of each fixture, with the default kgram and window.
    let expected = [(SAMPLE1, 96), (SAMPLE2, 108)];

    let count = |cell: &str| cell.trim_matches(['[', ']']).split(',').count();

    let rows = read_rows(&report_dir.join("files.csv"));
    assert_eq!(rows.len(), expected.len());
    for (row, (path, fingerprints)) in rows.iter().zip(expected) {
        assert_eq!(&row[1], path);
        assert_eq!(count(&row[3]), fingerprints);
        assert_eq!(count(&row[4]), 4 * fingerprints);
        // Nothing is ignored without a template or a frequency cap.
        assert_eq!(&row[5], "[]");
    }
}

#[test]
fn csv_output_without_destination_creates_a_report_directory() {
    let tmp = TempDir::new().unwrap();
    // Absolute fixture paths so they still resolve after changing the cwd.
    let manifest = env!("CARGO_MANIFEST_DIR");
    let sample1 = format!("{manifest}/{SAMPLE1}");
    let sample2 = format!("{manifest}/{SAMPLE2}");
    // No -o: an auto-named `dolos-report-*` directory is created in the cwd.
    dolos_run(&[&sample1, &sample2], &["-f", "csv"])
        .current_dir(tmp.path())
        .assert()
        .success();

    let report_dir = std::fs::read_dir(tmp.path())
        .unwrap()
        .map(|e| e.unwrap().path())
        .find(|p| {
            p.file_name()
                .and_then(|n| n.to_str())
                .is_some_and(|s| s.starts_with("dolos-report-"))
        })
        .expect("report directory not created");

    assert!(report_dir.join("pairs.csv").exists());
    assert!(report_dir.join("metadata.csv").exists());
    assert!(report_dir.join("files.csv").exists());
    assert!(report_dir.join("fragments.csv").exists());
}

#[test]
fn csv_output_rejects_an_existing_directory() {
    let tmp = TempDir::new().unwrap();
    // The destination already exists, so the run must fail.
    dolos_run(
        &[SAMPLE1, SAMPLE2],
        &["-f", "csv", "-o", tmp.path().to_str().unwrap()],
    )
    .assert()
    .failure()
    .stderr(predicate::str::contains("already exists"));
}

#[test]
fn invalid_options_fail_the_process() {
    dolos_run(&[SAMPLE1, SAMPLE2], &["-k", "0"])
        .assert()
        .failure()
        .stderr(predicate::str::contains("must be at least 1"));

    dolos_run(&[SAMPLE1, SAMPLE2], &["-l", "notalang"])
        .assert()
        .failure()
        .stderr(predicate::str::contains("unknown language"));
}
