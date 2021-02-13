use super::ast::{Span, Spanned};
use num_rational::Ratio;
use std::cmp::PartialEq;

#[derive(Debug, Clone, PartialEq, Eq)]
/// ASCII letter(s) followed by a [`Value`]
pub struct Field<'input> {
    pub(crate) letters: &'input str,
    pub(crate) value: Value<'input>,
    pub(crate) raw_value: Vec<&'input str>,
    pub(crate) span: Span,
}

impl<'input> Field<'input> {
    /// Iterate over the bytes of the raw text.
    /// Used in [`Line.compute_checksum`].
    pub fn iter_bytes(&'input self) -> impl Iterator<Item = &'input u8> {
        self.letters
            .as_bytes()
            .iter()
            .chain(self.raw_value.iter().map(|s| s.as_bytes().iter()).flatten())
    }
}

impl<'input> Spanned for Field<'input> {
    fn span(&self) -> Span {
        self.span
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Value<'input> {
    /// A real number GCode value.
    ///
    /// While this is often a floating point number,
    /// that was converted to a string,
    /// it is parsed as a rational number to
    /// ensure numerical stability.
    Rational(Ratio<i64>),
    /// An unsigned integer GCode value fitting in a [`usize`].
    /// For instance, this would be the 0 in G0.
    Integer(usize),
    /// A string GCode value.
    ///
    /// The delimiting quotes are included in the value
    /// and the escaped quotes are NOT unescaped.
    String(&'input str),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Checksum {
    pub(crate) inner: u8,
    pub(crate) span: Span,
}

impl Spanned for Checksum {
    fn span(&self) -> Span {
        self.span
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
/// A `'\n'` token that delimits [`super::Line`]s in a [`super::File`].
pub struct Newline {
    pub(crate) pos: usize,
}

impl Spanned for Newline {
    fn span(&self) -> Span {
        Span(self.pos, self.pos + 1)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
/// Any sequence of ASCII whitespace except for [`Newline`].
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
/// A semicolon `;` followed by ASCII characters and terminated by a [`Newline`]
/// or the end of the file.
///
/// The semicolon is part of the inner representation.
///
/// Some machines/programs will display these comments
/// as the GCode is executed.
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
/// A [`Newline`] is not allowed in an inline comment.
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
