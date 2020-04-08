//! FIXME: write short doc here

mod text_edit;

use text_size::{TextRange, TextSize};

pub use crate::text_edit::{TextEdit, TextEditBuilder};

/// Must not overlap with other `AtomTextEdit`s
#[derive(Debug, Clone)]
pub struct AtomTextEdit {
    /// Refers to offsets in the original text
    pub delete: TextRange,
    pub insert: String,
}

impl AtomTextEdit {
    pub fn replace(range: TextRange, replace_with: String) -> AtomTextEdit {
        AtomTextEdit { delete: range, insert: replace_with }
    }

    pub fn delete(range: TextRange) -> AtomTextEdit {
        AtomTextEdit::replace(range, String::new())
    }

    pub fn insert(offset: TextSize, text: String) -> AtomTextEdit {
        AtomTextEdit::replace(TextRange::at(offset, 0.into()), text)
    }

    pub fn apply(&self, mut text: String) -> String {
        let start = usize::from(self.delete.start());
        let end = usize::from(self.delete.end());
        text.replace_range(start..end, &self.insert);
        text
    }
}
