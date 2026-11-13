use crate::file::{FileInfo, FileSet, InfoRow};
use chrono::{DateTime, FixedOffset};
use csv::{Reader, StringRecord};
use std::fs;
use std::io::{Error, ErrorKind, Result};
use std::path::{Path, PathBuf};

/// Resolve files from a directory. If an `info.csv` is present at the top
/// level, it is used to determine the file list; otherwise files are collected
/// recursively.
pub fn from_directory(dir: &Path) -> Result<(String, FileSet)> {
    let csv_path = dir.join("info.csv");
    let file_set = if csv_path.is_file() {
        FileSet::with_info(dir, files_from_csv(&csv_path)?)
    } else {
        FileSet::new(dir, collect_recursive(dir)?)
    };
    Ok((dir_name(dir)?, file_set))
}

/// Resolve files from a CSV file. The name is derived from the CSV's parent
/// directory.
pub fn from_csv(csv_path: &Path) -> Result<(String, FileSet)> {
    let files = files_from_csv(csv_path)?;
    let base_dir = csv_path.parent().unwrap_or(Path::new("."));
    let file_set = FileSet::with_info(base_dir, files);
    Ok((dir_name(csv_dir(csv_path))?, file_set))
}

/// Resolve files from an explicit list of paths. All paths must be existing
/// files.
pub fn from_files(paths: Vec<PathBuf>) -> Result<Vec<PathBuf>> {
    if let Some(path) = paths.iter().find(|p| !p.is_file()) {
        return Err(Error::new(
            ErrorKind::NotFound,
            format!("'{}' is not a file", path.display()),
        ));
    }
    Ok(paths)
}

/// Name a list of files: `a.js-b.js` for two files, `N files` otherwise.
pub fn files_name(paths: &[PathBuf]) -> String {
    match paths {
        [a, b] => format!(
            "{}-{}",
            a.file_name().unwrap_or_default().to_string_lossy(),
            b.file_name().unwrap_or_default().to_string_lossy(),
        ),
        _ => format!("{} files", paths.len()),
    }
}

/// Read the path and the [`InfoRow`] of each row of a CSV file.
fn files_from_csv(csv_path: &Path) -> Result<Vec<(PathBuf, InfoRow)>> {
    let dir = csv_dir(csv_path);
    let invalid = |e| Error::new(ErrorKind::InvalidData, e);
    let mut rdr = Reader::from_path(csv_path).map_err(invalid)?;

    let filename_col = rdr
        .headers()
        .map_err(invalid)?
        .iter()
        .position(|h| h == "filename")
        .ok_or_else(|| Error::new(ErrorKind::InvalidData, "CSV missing 'filename' column"))?;
    let info_columns = InfoColumns::find(rdr.headers().map_err(invalid)?);

    rdr.records()
        .map(|r| {
            let record = r.map_err(invalid)?;
            Ok((dir.join(&record[filename_col]), info_columns.read(&record)))
        })
        .collect()
}

/// The positions of the columns of a CSV file that fill an [`InfoRow`].
struct InfoColumns {
    full_name: Option<usize>,
    labels: Option<usize>,
    label: Option<usize>,
    created_at: Option<usize>,
    ignored: Option<usize>,
}

impl InfoColumns {
    fn find(headers: &StringRecord) -> Self {
        let position = |name| headers.iter().position(|header| header == name);
        Self {
            full_name: position("full_name"),
            labels: position("labels"),
            label: position("label"),
            created_at: position("created_at"),
            ignored: position("ignored"),
        }
    }

    /// Read the [`InfoRow`] of one row. An empty value gives `None`.
    fn read(&self, record: &StringRecord) -> InfoRow {
        let value = |column: Option<usize>| {
            let value = record.get(column?)?;
            (!value.is_empty()).then(|| value.to_owned())
        };
        let info = FileInfo {
            full_name: value(self.full_name),
            labels: value(self.labels).or_else(|| value(self.label)),
            created_at: value(self.created_at).and_then(|value| parse_created_at(&value)),
        };
        InfoRow {
            info,
            ignored: value(self.ignored).as_deref() == Some("true"),
        }
    }
}

/// Parse a timestamp in RFC 3339 or in the Dodona format `2019-07-23 17:12:33 +0200`.
fn parse_created_at(value: &str) -> Option<DateTime<FixedOffset>> {
    DateTime::parse_from_rfc3339(value)
        .or_else(|_| DateTime::parse_from_str(value, "%Y-%m-%d %H:%M:%S %z"))
        .ok()
}

fn collect_recursive(dir: &Path) -> Result<Vec<PathBuf>> {
    let mut files = Vec::new();
    let mut pending = vec![dir.to_path_buf()];
    while let Some(current) = pending.pop() {
        for entry in fs::read_dir(&current)? {
            let path = entry?.path();
            if path.is_dir() {
                pending.push(path);
            } else {
                files.push(path);
            }
        }
    }
    Ok(files)
}

/// Return the directory of a CSV file, or `.` for a bare file name.
fn csv_dir(csv_path: &Path) -> &Path {
    match csv_path.parent() {
        Some(dir) if !dir.as_os_str().is_empty() => dir,
        _ => Path::new("."),
    }
}

/// Return the last component of the canonical form of an existing directory.
fn dir_name(dir: &Path) -> Result<String> {
    let dir = dir.canonicalize().map_err(|e| {
        Error::new(
            e.kind(),
            format!("Could not resolve directory '{}': {e}", dir.display()),
        )
    })?;
    Ok(dir
        .file_name()
        .unwrap_or(dir.as_os_str())
        .to_string_lossy()
        .into_owned())
}

#[cfg(test)]
mod tests {
    use super::*;
    use rstest::rstest;
    use tempfile::tempdir;

    /// Write `content` to an `info.csv` and read the [`InfoRow`] of each row.
    fn rows_of_csv(content: &str) -> Vec<InfoRow> {
        let tmp = tempdir().unwrap();
        let csv_path = tmp.path().join("info.csv");
        fs::write(&csv_path, content).unwrap();
        let files = files_from_csv(&csv_path).unwrap();
        files.into_iter().map(|(_, row)| row).collect()
    }

    /// Write `content` to an `info.csv` and read the [`FileInfo`] of each row.
    fn info_of_csv(content: &str) -> Vec<FileInfo> {
        let rows = rows_of_csv(content);
        rows.into_iter().map(|row| row.info).collect()
    }

    #[test]
    fn only_the_value_true_marks_a_row_as_ignored() {
        let rows = rows_of_csv("filename,ignored\na.js,true\nb.js,false\nc.js,\nd.js,TRUE\n");
        let ignored: Vec<bool> = rows.iter().map(|row| row.ignored).collect();
        assert_eq!(ignored, [true, false, false, false]);
    }

    #[test]
    fn csv_row_fills_the_file_info() {
        let info = info_of_csv(
            "filename,full_name,created_at,labels\n\
             a.js,Ada Lovelace,2019-07-23 17:12:33 +0200,group-1\n\
             b.js,,,\n",
        );
        assert_eq!(info[0].full_name.as_deref(), Some("Ada Lovelace"));
        assert_eq!(info[0].labels.as_deref(), Some("group-1"));
        assert_eq!(
            info[0].created_at.unwrap().to_rfc3339(),
            "2019-07-23T17:12:33+02:00"
        );
        assert_eq!(info[1], FileInfo::default());
    }

    #[test]
    fn csv_without_info_columns_gives_empty_file_info() {
        assert_eq!(info_of_csv("filename\na.js\n"), [FileInfo::default()]);
    }

    #[rstest]
    #[case::label_column_only("filename,label\na.js,group-1\n", "group-1")]
    #[case::labels_first("filename,label,labels\na.js,group-1,group-2\n", "group-2")]
    #[case::labels_empty("filename,label,labels\na.js,group-1,\n", "group-1")]
    fn label_column_is_used_when_labels_has_no_value(#[case] csv: &str, #[case] expected: &str) {
        assert_eq!(info_of_csv(csv)[0].labels.as_deref(), Some(expected));
    }

    #[rstest]
    #[case::rfc3339("2019-07-23T17:12:33+02:00", Some("2019-07-23T17:12:33+02:00"))]
    #[case::rfc3339_utc("2019-07-23T15:12:33Z", Some("2019-07-23T15:12:33+00:00"))]
    #[case::dodona("2019-07-23 17:12:33 +0200", Some("2019-07-23T17:12:33+02:00"))]
    #[case::no_offset("2019-07-23 17:12:33", None)]
    #[case::not_a_timestamp("yesterday", None)]
    fn created_at_accepts_rfc3339_and_the_dodona_format(
        #[case] value: &str,
        #[case] expected: Option<&str>,
    ) {
        let created_at = parse_created_at(value).map(|timestamp| timestamp.to_rfc3339());
        assert_eq!(created_at.as_deref(), expected);
    }

    fn working_directory_name() -> String {
        let cwd = std::env::current_dir().unwrap();
        cwd.file_name().unwrap().to_string_lossy().into_owned()
    }

    #[test]
    fn bare_csv_path_is_named_after_the_working_directory() {
        let name = dir_name(csv_dir(Path::new("info.csv"))).unwrap();
        assert_eq!(name, working_directory_name());
    }

    #[test]
    fn collect_recursive_finds_nested_files() {
        let tmp = tempdir().unwrap();
        fs::write(tmp.path().join("b.txt"), "").unwrap();
        fs::write(tmp.path().join("a.txt"), "").unwrap();
        let sub = tmp.path().join("sub");
        fs::create_dir(&sub).unwrap();
        fs::write(sub.join("c.txt"), "").unwrap();

        let files = collect_recursive(tmp.path()).unwrap();
        let mut names: Vec<_> = files
            .iter()
            .map(|p| p.file_name().unwrap().to_str().unwrap())
            .collect();
        names.sort();
        assert_eq!(names, ["a.txt", "b.txt", "c.txt"]);
    }
}
