mod archive;
mod dataset;
mod input;
mod resolve;
mod template;

pub use dataset::Dataset;
pub use input::Input;
pub use template::Template;

use std::path::PathBuf;

/// The number of skipped paths that a warning names.
const SHOWN_PATHS: usize = 3;

/// List the first [`SHOWN_PATHS`] paths, such as `a.md, b.txt, c.csv and 2 more`.
fn list_paths(paths: &[PathBuf]) -> String {
    let shown: Vec<String> = paths
        .iter()
        .take(SHOWN_PATHS)
        .map(|path| path.display().to_string())
        .collect();
    match paths.len().saturating_sub(SHOWN_PATHS) {
        0 => shown.join(", "),
        more => format!("{} and {more} more", shown.join(", ")),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rstest::rstest;

    #[rstest]
    #[case::few(&["a.md", "b.txt"], "a.md, b.txt")]
    #[case::many(&["a.md", "b.txt", "c.csv", "d", "e"], "a.md, b.txt, c.csv and 2 more")]
    fn list_paths_names_the_first_paths(#[case] paths: &[&str], #[case] expected: &str) {
        let paths: Vec<PathBuf> = paths.iter().map(PathBuf::from).collect();
        assert_eq!(list_paths(&paths), expected);
    }
}
