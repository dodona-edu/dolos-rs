use dolos::Point;
use std::iter;

/// The line starts of one file, to convert byte columns into UTF-16 columns.
pub struct LineIndex<'a> {
    content: &'a str,
    /// The byte offset of the start of each line. `None` when `content` is ASCII.
    line_starts: Option<Vec<usize>>,
}

impl<'a> LineIndex<'a> {
    /// Index the line starts of `content`. Skips the index when `content` is ASCII.
    pub fn new(content: &'a str) -> Self {
        let line_starts = (!content.is_ascii()).then(|| {
            iter::once(0)
                .chain(content.match_indices('\n').map(|(newline, _)| newline + 1))
                .collect()
        });
        Self { content, line_starts }
    }

    /// The column of `point` in UTF-16 code units. `point.column` is a byte offset in its line.
    ///
    /// Returns `point.column` unchanged for a point outside the content.
    pub fn utf16_column(&self, point: Point) -> usize {
        // In ASCII content, every byte is one UTF-16 code unit.
        let Some(line_starts) = &self.line_starts else {
            return point.column;
        };
        let prefix = line_starts
            .get(point.row)
            .and_then(|&start| self.content.get(start..start + point.column));
        match prefix {
            Some(prefix) => prefix.encode_utf16().count(),
            None => point.column,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rstest::rstest;

    const LINE: &str = r#"let s = "é😀"; let t = 1;"#;

    #[rstest]
    #[case::ascii("let t = 1;", Point::new(0, 4), 4)]
    #[case::after_non_ascii_text(LINE, Point::new(0, 18), 15)]
    #[case::at_the_line_end("é😀\nx", Point::new(0, 6), 3)]
    #[case::at_the_start_of_the_next_row("é😀\nx", Point::new(1, 0), 0)]
    #[case::on_a_row_after_non_ascii_rows("é\nlet x", Point::new(1, 4), 4)]
    #[case::at_the_end_of_the_file(LINE, Point::new(0, 28), 25)]
    #[case::past_the_last_line("é😀\n", Point::new(1, 0), 0)]
    fn utf16_column_counts_utf16_code_units(
        #[case] content: &str,
        #[case] point: Point,
        #[case] expected: usize,
    ) {
        assert_eq!(LineIndex::new(content).utf16_column(point), expected);
    }
}
