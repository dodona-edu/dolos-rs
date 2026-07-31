use assert_cmd::Command;
use predicates::prelude::*;
use tempfile::TempDir;

const SAMPLE1: &str = "fixtures/sample1.js";
const SAMPLE2: &str = "fixtures/sample2.js";

fn dolos_run(files: &[&str], args: &[&str]) -> Command {
    let mut cmd = Command::cargo_bin("dolos").unwrap();
    cmd.arg("run").args(files).args(args);
    cmd
}

// ── Smoke tests ───────────────────────────────────────────────────────────────

#[test]
fn smoke_terminal() {
    dolos_run(&[SAMPLE1, SAMPLE2], &[])
        .assert()
        .success()
        .stdout(predicate::str::contains("sim:"));
}

#[test]
fn smoke_csv_output() {
    let tmp = TempDir::new().unwrap();
    // -o is the exact report directory; it must not exist yet.
    let report_dir = tmp.path().join("report");
    #[rustfmt::skip]
    dolos_run(
        &[SAMPLE1, SAMPLE2],
        &[
            "-f", "csv",
            "-n", "report",
            "-o", report_dir.to_str().unwrap(),
        ],
    )
    .assert()
    .success();

    // Files are written directly into the given directory, no subfolder.
    assert!(report_dir.join("pairs.csv").exists());
    assert!(report_dir.join("metadata.csv").exists());
    assert!(report_dir.join("files.csv").exists());
    assert!(report_dir.join("fragments.csv").exists());

    let pairs_csv = std::fs::read_to_string(report_dir.join("pairs.csv")).unwrap();
    let pairs_header = pairs_csv.lines().next().unwrap();
    assert_eq!(
        pairs_header,
        "file1_id,file1_path,file2_id,file2_path,similarity,longest,totalLeft,totalRight,overlapLeft,overlapRight"
    );

    let metadata_csv = std::fs::read_to_string(report_dir.join("metadata.csv")).unwrap();
    let metadata_header = metadata_csv.lines().next().unwrap();
    assert_eq!(metadata_header, "property,value");

    let files_csv = std::fs::read_to_string(report_dir.join("files.csv")).unwrap();
    let files_header = files_csv.lines().next().unwrap();
    assert_eq!(files_header, "id,path,content");

    let fragments_csv = std::fs::read_to_string(report_dir.join("fragments.csv")).unwrap();
    let fragments_header = fragments_csv.lines().next().unwrap();
    assert_eq!(
        fragments_header,
        "file1_id,file1_path,file1_start_point,file1_end_point,file2_id,file2_path,file2_start_point,file2_end_point,fingerprint_count,ignored"
    );
}

#[test]
fn csv_output_with_fingerprints() {
    let tmp = TempDir::new().unwrap();
    let report_dir = tmp.path().join("report");
    #[rustfmt::skip]
    dolos_run(
        &[SAMPLE1, SAMPLE2],
        &[
            "-f", "csv",
            "-o", report_dir.to_str().unwrap(),
            "--include-core-data",
        ],
    )
    .assert()
    .success();

    let files_csv = std::fs::read_to_string(report_dir.join("files.csv")).unwrap();
    let files_header = files_csv.lines().next().unwrap();
    assert!(files_header.ends_with("fingerprints,fingerprint_regions"));

    let metadata_csv = std::fs::read_to_string(report_dir.join("metadata.csv")).unwrap();
    assert!(metadata_csv.contains("includeCoreData,true"));
}

#[test]
fn csv_output_default_destination() {
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
fn csv_output_errors_when_directory_exists() {
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

// ── Error surfacing ───────────────────────────────────────────────────────────

#[test]
fn test_errors_reach_process() {
    dolos_run(&[SAMPLE1, SAMPLE2], &["-k", "0"])
        .assert()
        .failure()
        .stderr(predicate::str::contains("must be at least 1"));

    dolos_run(&[SAMPLE1, SAMPLE2], &["-l", "notalang"])
        .assert()
        .failure()
        .stderr(predicate::str::contains("unknown language"));
}
