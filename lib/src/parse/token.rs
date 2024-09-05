use super::ast::{Span, Spanned};
use rust_decimal::Decimal;
use std::cmp::PartialEq;

#[derive(Debug, Clone, PartialEq, Eq)]
/// ASCII letter(s) followed by a [Value]
pub struct Field<'input> {
    pub(crate) letters: &'input str,
    pub(crate) value: Value<'input>,
    pub(crate) raw_value: Vec<&'input str>,
    pub(crate) span: Span,
}

impl<'input> Field<'input> {
    /// Iterate over [u8] in a [Field].
    pub fn iter_bytes(&'input self) -> impl Iterator<Item = &'input u8> {
        self.letters
            .as_bytes()
            .iter()
            .chain(self.raw_value.iter().flat_map(|s| s.as_bytes().iter()))
    }
}

impl<'input> Spanned for Field<'input> {
    fn span(&self) -> Span {
        self.span
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Flag<'input> {
    pub(crate) letter: &'input str,
    pub(crate) span: Span,
}

impl<'input> Flag<'input> {
    /// Iterate over [u8] in a [Field].
    pub fn iter_bytes(&'input self) -> impl Iterator<Item = &'input u8> {
        self.letter.as_bytes().iter()
    }
}

impl<'input> Spanned for Flag<'input> {
    fn span(&self) -> Span {
        self.span
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Value<'input> {
    /// A real number g-code value.
    ///
    /// While this is often an [f64] or [f32],
    /// that was converted to a string,
    /// it is parsed as a [Decimal] to
    /// ensure numerical stability.
    Rational(Decimal),
    /// An unsigned integer g-code value fitting in a [usize].
    /// For instance, this would be the 0 in G0.
    Integer(usize),
    /// A [string](str) g-code value.
    ///
    /// Delimiting quotes are included in the value
    /// and escaped quotes are NOT unescaped.
    String(&'input str),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Checksum {
    // Note for readers:
    // this is not stored as a str because any
    // leading zeros do not affect the checksum
    pub(crate) inner: u8,
    pub(crate) span: Span,
}

impl Spanned for Checksum {
    fn span(&self) -> Span {
        self.span
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
/// A `\n` or `\r\n` token that delimits instances of [super::ast::Line] in a [super::ast::File].
pub struct Newline {
    pub(crate) pos: usize,
}

impl Spanned for Newline {
    fn span(&self) -> Span {
        Span(self.pos, self.pos + 1)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
/// A `%` token that wraps a g-code [super::ast::File].
pub struct Percent {
    pub(crate) pos: usize,
}

impl Spanned for Percent {
    fn span(&self) -> Span {
        Span(self.pos, self.pos + 1)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
/// Any sequence of ASCII whitespace except for [Newline].
pub struct Whitespace<'input> {
    pub(crate) inner: &'input str,
    pub(crate) pos: usize,
}

impl<'input> Whitespace<'input> {
    pub fn iter_bytes(&'input self) -> impl Iterator<Item = &'input u8> {
        self.inner.as_bytes().iter()
    }
}

impl<'input> Spanned for Whitespace<'input> {
    fn span(&self) -> Span {
        Span(self.pos, self.pos + self.inner.len())
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
/// A semicolon `;` followed by ASCII characters and terminated by a [Newline]
/// or the end of the file.
///
/// The semicolon is part of the inner representation.
///
/// Some machines/programs will display these comments
/// as the g-code is executed.
pub struct Comment<'input> {
    pub(crate) inner: &'input str,
    pub(crate) pos: usize,
}

impl<'input> Comment<'input> {
    pub fn iter_bytes(&'input self) -> impl Iterator<Item = &'input u8> {
        self.inner.as_bytes().iter()
    }
}

impl<'input> Spanned for Comment<'input> {
    fn span(&self) -> Span {
        Span(self.pos, self.pos + self.inner.len())
    }
}

/// An opening parenthesis `(` followed by ASCII characters and terminated
/// by a closing parenthesis `)`.
/// A [Newline] is not allowed in an inline comment.
///
/// The parentheses are part of the inner representation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InlineComment<'input> {
    pub(crate) inner: &'input str,
    pub(crate) pos: usize,
}

impl<'input> InlineComment<'input> {
    pub fn iter_bytes(&'input self) -> impl Iterator<Item = &'input u8> {
        self.inner.as_bytes().iter()
    }
}

impl<'input> Spanned for InlineComment<'input> {
    fn span(&self) -> Span {
        Span(self.pos, self.pos + self.inner.len())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
/// An internal structure used to make writing the [peg] parser easier.
pub struct LineComponent<'input> {
    pub(crate) field: Option<Field<'input>>,
    pub(crate) flag: Option<Flag<'input>>,
    pub(crate) whitespace: Option<Whitespace<'input>>,
    pub(crate) inline_comment: Option<InlineComment<'input>>,
}

impl<'input> LineComponent<'input> {
    pub fn iter_bytes(&'input self) -> impl Iterator<Item = &'input u8> + 'input {
        self.field
            .iter()
            .flat_map(|f| f.iter_bytes())
            .chain(self.flag.iter().flat_map(|f| f.iter_bytes()))
            .chain(self.whitespace.iter().flat_map(|w| w.iter_bytes()))
            .chain(self.inline_comment.iter().flat_map(|i| i.iter_bytes()))
    }
}
