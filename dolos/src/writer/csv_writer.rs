use crate::report::Pair;
use crate::winnowing::region::Point;
use crate::writer::output::OutputWriter;
use std::fs::File;
use std::io::{Error, Result};
use std::path::PathBuf;

const PAIRS_HEADER: &[&str] = &[
    "file1",
    "file2",
    "similarity",
    "longest",
    "totalLeft",
    "totalRight",
    "overlapLeft",
    "overlapRight",
];

const MATCHES_HEADER: &[&str] = &[
    "file1",
    "file2",
    "file1_start_point",
    "file1_end_point",
    "file2_start_point",
    "file2_end_point",
    "fingerprint_count",
];

/// Format a [`Point`] as a single `row:column` string for CSV output.
fn fmt_point(p: Point) -> String {
    format!("{}:{}", p.row, p.column)
}

/// CSV writer that outputs similarity results to a CSV file.
pub struct CsvWriter {
    pairs_writer: csv::Writer<File>,
    /// Directory where `matches.csv` is created on demand.
    report_dir: PathBuf,
    /// Writer for `matches.csv`, created lazily on the first pair that carries
    /// fragments. Stays `None` (and the file is never created) when no pair has
    /// matches.
    matches_writer: Option<csv::Writer<File>>,
}

impl CsvWriter {
    /// Create a new CSV writer.
    ///
    /// Creates `{output_destination}/{name}/pairs.csv`, along with all required
    /// parent directories. `matches.csv` is only created later if at least one
    /// pair carries fragments.
    pub(super) fn new(output_destination: PathBuf, name: &str) -> Result<Self> {
        let report_dir = output_destination.join(name);
        std::fs::create_dir_all(&report_dir)?;
        let csv_path = report_dir.join("pairs.csv");
        let mut pairs_writer = csv::Writer::from_path(&csv_path).map_err(Error::other)?;
        pairs_writer
            .write_record(PAIRS_HEADER)
            .map_err(Error::other)?;
        Ok(Self {
            pairs_writer,
            report_dir,
            matches_writer: None,
        })
    }
}

impl OutputWriter for CsvWriter {
    fn write_pair(&mut self, pair: &Pair) -> Result<()> {
        let m = &pair.metrics;
        let file1 = pair.left_file.relative_path.display().to_string();
        let file2 = pair.right_file.relative_path.display().to_string();

        self.pairs_writer
            .serialize((
                &file1,
                &file2,
                m.similarity,
                m.longest_fragment,
                m.total_left,
                m.total_right,
                m.overlap_left,
                m.overlap_right,
            ))
            .map_err(Error::other)?;

        if let Some(fragments) = &pair.fragments {
            // Lazily create matches.csv on the first pair that has fragments,
            // so the file is never written when no matches are stored.
            let writer = match &mut self.matches_writer {
                Some(w) => w,
                None => {
                    let path = self.report_dir.join("matches.csv");
                    let mut w = csv::Writer::from_path(&path).map_err(Error::other)?;
                    w.write_record(MATCHES_HEADER).map_err(Error::other)?;
                    self.matches_writer.insert(w)
                }
            };

            for frag in fragments {
                writer
                    .serialize((
                        &file1,
                        &file2,
                        fmt_point(frag.left_region.start_point),
                        fmt_point(frag.left_region.end_point),
                        fmt_point(frag.right_region.start_point),
                        fmt_point(frag.right_region.end_point),
                        frag.fingerprint_count,
                    ))
                    .map_err(Error::other)?;
            }
        }

        Ok(())
    }

    fn finish(mut self) -> Result<()> {
        self.pairs_writer.flush().map_err(Error::other)?;
        if let Some(w) = &mut self.matches_writer {
            w.flush().map_err(Error::other)?;
        }
        Ok(())
    }
}
