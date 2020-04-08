//! `LineIndex` maps flat `TextSize` offsets into `(Line, Column)`
//! representation.
use std::iter;

use ra_syntax::{TextRange, TextSize};
use rustc_hash::FxHashMap;
use superslice::Ext;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LineIndex {
    pub(crate) newlines: Vec<TextSize>,
    pub(crate) utf16_lines: FxHashMap<u32, Vec<Utf16Char>>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct LineCol {
    /// Zero-based
    pub line: u32,
    /// Zero-based
    pub col_utf16: u32,
}

#[derive(Clone, Debug, Hash, PartialEq, Eq)]
pub(crate) struct Utf16Char {
    pub(crate) start: TextSize,
    pub(crate) end: TextSize,
}

impl Utf16Char {
    fn len(&self) -> TextSize {
        self.end - self.start
    }
}

impl LineIndex {
    pub fn new(text: &str) -> LineIndex {
        let mut utf16_lines = FxHashMap::default();
        let mut utf16_chars = Vec::new();

        let mut newlines = vec![0.into()];
        let mut curr_row = 0.into();
        let mut curr_col = 0.into();
        let mut line = 0;
        for c in text.chars() {
            curr_row += TextSize::of(c);
            if c == '\n' {
                newlines.push(curr_row);

                // Save any utf-16 characters seen in the previous line
                if !utf16_chars.is_empty() {
                    utf16_lines.insert(line, utf16_chars);
                    utf16_chars = Vec::new();
                }

                // Prepare for processing the next line
                curr_col = 0.into();
                line += 1;
                continue;
            }

            let char_len = TextSize::of(c);
            if char_len > TextSize::from(1) {
                utf16_chars.push(Utf16Char { start: curr_col, end: curr_col + char_len });
            }

            curr_col += char_len;
        }

        // Save any utf-16 characters seen in the last line
        if !utf16_chars.is_empty() {
            utf16_lines.insert(line, utf16_chars);
        }

        LineIndex { newlines, utf16_lines }
    }

    pub fn line_col(&self, offset: TextSize) -> LineCol {
        let line = self.newlines.upper_bound(&offset) - 1;
        let line_start_offset = self.newlines[line];
        let col = offset - line_start_offset;

        LineCol { line: line as u32, col_utf16: self.utf8_to_utf16_col(line as u32, col) as u32 }
    }

    pub fn offset(&self, line_col: LineCol) -> TextSize {
        //FIXME: return Result
        let col = self.utf16_to_utf8_col(line_col.line, line_col.col_utf16);
        self.newlines[line_col.line as usize] + col
    }

    pub fn lines(&self, range: TextRange) -> impl Iterator<Item = TextRange> + '_ {
        let lo = self.newlines.lower_bound(&range.start());
        let hi = self.newlines.upper_bound(&range.end());
        let all = iter::once(range.start())
            .chain(self.newlines[lo..hi].iter().copied())
            .chain(iter::once(range.end()));

        all.clone()
            .zip(all.skip(1))
            .map(|(lo, hi)| TextRange::new(lo, hi))
            .filter(|it| !it.is_empty())
    }

    fn utf8_to_utf16_col(&self, line: u32, col: TextSize) -> usize {
        let correction = if let Some(utf16_chars) = self.utf16_lines.get(&line) {
            let mut correction = 0;
            for c in utf16_chars {
                if col >= c.end {
                    correction += usize::from(c.len()) - 1;
                } else {
                    // From here on, all utf16 characters come *after* the character we are mapping,
                    // so we don't need to take them into account
                    break;
                }
            }
            correction
        } else {
            0
        };
        usize::from(col) - correction
    }

    fn utf16_to_utf8_col(&self, line: u32, col: u32) -> TextSize {
        let mut col: TextSize = col.into();
        if let Some(utf16_chars) = self.utf16_lines.get(&line) {
            for c in utf16_chars {
                if col >= c.start {
                    col += c.len() - TextSize::from(1);
                } else {
                    // From here on, all utf16 characters come *after* the character we are mapping,
                    // so we don't need to take them into account
                    break;
                }
            }
        }

        col
    }
}

#[cfg(test)]
mod test_line_index {
    use super::*;

    #[test]
    fn test_line_index() {
        let text = "hello\nworld";
        let index = LineIndex::new(text);
        assert_eq!(index.line_col(0.into()), LineCol { line: 0, col_utf16: 0 });
        assert_eq!(index.line_col(1.into()), LineCol { line: 0, col_utf16: 1 });
        assert_eq!(index.line_col(5.into()), LineCol { line: 0, col_utf16: 5 });
        assert_eq!(index.line_col(6.into()), LineCol { line: 1, col_utf16: 0 });
        assert_eq!(index.line_col(7.into()), LineCol { line: 1, col_utf16: 1 });
        assert_eq!(index.line_col(8.into()), LineCol { line: 1, col_utf16: 2 });
        assert_eq!(index.line_col(10.into()), LineCol { line: 1, col_utf16: 4 });
        assert_eq!(index.line_col(11.into()), LineCol { line: 1, col_utf16: 5 });
        assert_eq!(index.line_col(12.into()), LineCol { line: 1, col_utf16: 6 });

        let text = "\nhello\nworld";
        let index = LineIndex::new(text);
        assert_eq!(index.line_col(0.into()), LineCol { line: 0, col_utf16: 0 });
        assert_eq!(index.line_col(1.into()), LineCol { line: 1, col_utf16: 0 });
        assert_eq!(index.line_col(2.into()), LineCol { line: 1, col_utf16: 1 });
        assert_eq!(index.line_col(6.into()), LineCol { line: 1, col_utf16: 5 });
        assert_eq!(index.line_col(7.into()), LineCol { line: 2, col_utf16: 0 });
    }

    #[test]
    fn test_char_len() {
        assert_eq!('メ'.len_utf8(), 3);
        assert_eq!('メ'.len_utf16(), 1);
    }

    #[test]
    fn test_empty_index() {
        let col_index = LineIndex::new(
            "
const C: char = 'x';
",
        );
        assert_eq!(col_index.utf16_lines.len(), 0);
    }

    #[test]
    fn test_single_char() {
        let col_index = LineIndex::new(
            "
const C: char = 'メ';
",
        );

        assert_eq!(col_index.utf16_lines.len(), 1);
        assert_eq!(col_index.utf16_lines[&1].len(), 1);
        assert_eq!(col_index.utf16_lines[&1][0], Utf16Char { start: 17.into(), end: 20.into() });

        // UTF-8 to UTF-16, no changes
        assert_eq!(col_index.utf8_to_utf16_col(1, 15.into()), 15);

        // UTF-8 to UTF-16
        assert_eq!(col_index.utf8_to_utf16_col(1, 22.into()), 20);

        // UTF-16 to UTF-8, no changes
        assert_eq!(col_index.utf16_to_utf8_col(1, 15), TextSize::from(15));

        // UTF-16 to UTF-8
        assert_eq!(col_index.utf16_to_utf8_col(1, 19), TextSize::from(21));
    }

    #[test]
    fn test_string() {
        let col_index = LineIndex::new(
            "
const C: char = \"メ メ\";
",
        );

        assert_eq!(col_index.utf16_lines.len(), 1);
        assert_eq!(col_index.utf16_lines[&1].len(), 2);
        assert_eq!(col_index.utf16_lines[&1][0], Utf16Char { start: 17.into(), end: 20.into() });
        assert_eq!(col_index.utf16_lines[&1][1], Utf16Char { start: 21.into(), end: 24.into() });

        // UTF-8 to UTF-16
        assert_eq!(col_index.utf8_to_utf16_col(1, 15.into()), 15);

        assert_eq!(col_index.utf8_to_utf16_col(1, 21.into()), 19);
        assert_eq!(col_index.utf8_to_utf16_col(1, 25.into()), 21);

        assert!(col_index.utf8_to_utf16_col(2, 15.into()) == 15);

        // UTF-16 to UTF-8
        assert_eq!(col_index.utf16_to_utf8_col(1, 15), TextSize::from(15));

        assert_eq!(col_index.utf16_to_utf8_col(1, 18), TextSize::from(20));
        assert_eq!(col_index.utf16_to_utf8_col(1, 19), TextSize::from(23));

        assert_eq!(col_index.utf16_to_utf8_col(2, 15), TextSize::from(15));
    }

    #[test]
    fn test_splitlines() {
        fn r(lo: u32, hi: u32) -> TextRange {
            TextRange::new(lo.into(), hi.into())
        }

        let text = "a\nbb\nccc\n";
        let line_index = LineIndex::new(text);

        let actual = line_index.lines(r(0, 9)).collect::<Vec<_>>();
        let expected = vec![r(0, 2), r(2, 5), r(5, 9)];
        assert_eq!(actual, expected);

        let text = "";
        let line_index = LineIndex::new(text);

        let actual = line_index.lines(r(0, 0)).collect::<Vec<_>>();
        let expected = vec![];
        assert_eq!(actual, expected);

        let text = "\n";
        let line_index = LineIndex::new(text);

        let actual = line_index.lines(r(0, 1)).collect::<Vec<_>>();
        let expected = vec![r(0, 1)];
        assert_eq!(actual, expected)
    }
}
