//! There are many AstNodes, but only a few tokens, so we hand-write them here.

use std::convert::TryFrom;

use crate::{
    ast::AstToken,
    SyntaxKind::{COMMENT, RAW_STRING, STRING, WHITESPACE},
    SyntaxToken, TextRange, TextSize,
};

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Comment(SyntaxToken);

impl AstToken for Comment {
    fn cast(token: SyntaxToken) -> Option<Self> {
        match token.kind() {
            COMMENT => Some(Comment(token)),
            _ => None,
        }
    }
    fn syntax(&self) -> &SyntaxToken {
        &self.0
    }
}

impl Comment {
    pub fn kind(&self) -> CommentKind {
        kind_by_prefix(self.text())
    }

    pub fn prefix(&self) -> &'static str {
        prefix_by_kind(self.kind())
    }
}

#[derive(Debug, PartialEq, Eq, Clone, Copy)]
pub struct CommentKind {
    pub shape: CommentShape,
    pub doc: Option<CommentPlacement>,
}

#[derive(Debug, PartialEq, Eq, Clone, Copy)]
pub enum CommentShape {
    Line,
    Block,
}

impl CommentShape {
    pub fn is_line(self) -> bool {
        self == CommentShape::Line
    }

    pub fn is_block(self) -> bool {
        self == CommentShape::Block
    }
}

#[derive(Debug, PartialEq, Eq, Clone, Copy)]
pub enum CommentPlacement {
    Inner,
    Outer,
}

const COMMENT_PREFIX_TO_KIND: &[(&str, CommentKind)] = {
    use {CommentPlacement::*, CommentShape::*};
    &[
        ("///", CommentKind { shape: Line, doc: Some(Outer) }),
        ("//!", CommentKind { shape: Line, doc: Some(Inner) }),
        ("/**", CommentKind { shape: Block, doc: Some(Outer) }),
        ("/*!", CommentKind { shape: Block, doc: Some(Inner) }),
        ("//", CommentKind { shape: Line, doc: None }),
        ("/*", CommentKind { shape: Block, doc: None }),
    ]
};

fn kind_by_prefix(text: &str) -> CommentKind {
    for (prefix, kind) in COMMENT_PREFIX_TO_KIND.iter() {
        if text.starts_with(prefix) {
            return *kind;
        }
    }
    panic!("bad comment text: {:?}", text)
}

fn prefix_by_kind(kind: CommentKind) -> &'static str {
    for (prefix, k) in COMMENT_PREFIX_TO_KIND.iter() {
        if *k == kind {
            return prefix;
        }
    }
    unreachable!()
}

pub struct Whitespace(SyntaxToken);

impl AstToken for Whitespace {
    fn cast(token: SyntaxToken) -> Option<Self> {
        match token.kind() {
            WHITESPACE => Some(Whitespace(token)),
            _ => None,
        }
    }
    fn syntax(&self) -> &SyntaxToken {
        &self.0
    }
}

impl Whitespace {
    pub fn spans_multiple_lines(&self) -> bool {
        let text = self.text();
        text.find('\n').map_or(false, |idx| text[idx + 1..].contains('\n'))
    }
}

pub struct QuoteOffsets {
    pub quotes: [TextRange; 2],
    pub contents: TextRange,
}

impl QuoteOffsets {
    fn new(literal: &str) -> Option<QuoteOffsets> {
        let left_quote = literal.find('"')?;
        let right_quote = literal.rfind('"')?;
        if left_quote == right_quote {
            // `literal` only contains one quote
            return None;
        }

        let start = TextSize::from(0);
        let left_quote = TextSize::try_from(left_quote).unwrap() + TextSize::of('"');
        let right_quote = TextSize::try_from(right_quote).unwrap();
        let end = TextSize::of(literal);

        let res = QuoteOffsets {
            quotes: [TextRange::new(start, left_quote), TextRange::new(right_quote, end)],
            contents: TextRange::new(left_quote, right_quote),
        };
        Some(res)
    }
}

pub trait HasQuotes: AstToken {
    fn quote_offsets(&self) -> Option<QuoteOffsets> {
        let text = self.text().as_str();
        let offsets = QuoteOffsets::new(text)?;
        let o = self.syntax().text_range().start();
        let offsets = QuoteOffsets {
            quotes: [offsets.quotes[0] + o, offsets.quotes[1] + o],
            contents: offsets.contents + o,
        };
        Some(offsets)
    }
    fn open_quote_text_range(&self) -> Option<TextRange> {
        self.quote_offsets().map(|it| it.quotes[0])
    }

    fn close_quote_text_range(&self) -> Option<TextRange> {
        self.quote_offsets().map(|it| it.quotes[1])
    }

    fn text_range_between_quotes(&self) -> Option<TextRange> {
        self.quote_offsets().map(|it| it.contents)
    }
}

impl HasQuotes for String {}
impl HasQuotes for RawString {}

pub trait HasStringValue: HasQuotes {
    fn value(&self) -> Option<std::string::String>;
}

pub struct String(SyntaxToken);

impl AstToken for String {
    fn cast(token: SyntaxToken) -> Option<Self> {
        match token.kind() {
            STRING => Some(String(token)),
            _ => None,
        }
    }
    fn syntax(&self) -> &SyntaxToken {
        &self.0
    }
}

impl HasStringValue for String {
    fn value(&self) -> Option<std::string::String> {
        let text = self.text().as_str();
        let text = &text[self.text_range_between_quotes()? - self.syntax().text_range().start()];

        let mut buf = std::string::String::with_capacity(text.len());
        let mut has_error = false;
        rustc_lexer::unescape::unescape_str(text, &mut |_, unescaped_char| match unescaped_char {
            Ok(c) => buf.push(c),
            Err(_) => has_error = true,
        });

        if has_error {
            return None;
        }
        Some(buf)
    }
}

pub struct RawString(SyntaxToken);

impl AstToken for RawString {
    fn cast(token: SyntaxToken) -> Option<Self> {
        match token.kind() {
            RAW_STRING => Some(RawString(token)),
            _ => None,
        }
    }
    fn syntax(&self) -> &SyntaxToken {
        &self.0
    }
}

impl HasStringValue for RawString {
    fn value(&self) -> Option<std::string::String> {
        let text = self.text().as_str();
        let text = &text[self.text_range_between_quotes()? - self.syntax().text_range().start()];
        Some(text.to_string())
    }
}

impl RawString {
    pub fn map_range_up(&self, range: TextRange) -> Option<TextRange> {
        let contents_range = self.text_range_between_quotes()?;
        assert!(contents_range.contains_range(range));
        Some(range + contents_range.start())
    }
}
