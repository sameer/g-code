use super::token::*;
use std::fmt::Debug;

#[derive(Clone, Copy, Debug, PartialEq, Hash, Eq)]
/// A range of [u8] in the raw text of the program.
/// Useful for providing diagnostic information in
/// higher-level tooling.
///
/// The end of the span is exclusive.
pub struct Span(pub usize, pub usize);

pub trait Spanned {
    fn span(&self) -> Span;
}

impl std::ops::Add for Span {
    type Output = Self;
    fn add(self, rhs: Self) -> Self {
        Self(self.0.min(rhs.0), self.1.max(rhs.1))
    }
}
impl std::ops::AddAssign for Span {
    fn add_assign(&mut self, rhs: Self) {
        *self = Self {
            0: self.0.min(rhs.0),
            1: self.1.max(rhs.1),
        }
    }
}

impl From<Span> for std::ops::Range<usize> {
    fn from(span: Span) -> Self {
        span.0..span.1
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
/// Representation of a sequence of g-code logically organized as a file.
/// This may also be referred to as a program.
pub struct File<'input> {
    pub(crate) start_percent: bool,
    pub(crate) lines: Vec<(Line<'input>, Newline)>,
    pub(crate) last_line: Option<Line<'input>>,
    pub(crate) end_percent: bool,
    pub(crate) span: Span,
}

impl<'input> File<'input> {
    /// Iterate by [Line].
    /// The last [Line] may or may not be followed by a [Newline].
    pub fn iter(&self) -> impl Iterator<Item = &Line<'input>> {
        self.lines
            .iter()
            .map(|(line, _)| line)
            .chain(self.last_line.iter())
    }

    /// Iterating by [Line] in a file.
    pub fn iter_fields(&self) -> impl Iterator<Item = &Field<'input>> {
        self.iter().flat_map(Line::iter_fields)
    }

    /// Iterate by [InlineComment] in a file.
    pub fn iter_inline_comments(&self) -> impl Iterator<Item = &InlineComment<'input>> {
        self.iter().flat_map(Line::iter_inline_comments)
    }

    /// Iterate by [Whitespace] in a file.
    pub fn iter_whitespace(&self) -> impl Iterator<Item = &Whitespace<'input>> {
        self.iter().flat_map(Line::iter_whitespace)
    }

    /// Iterate by [u8] in the file.
    pub fn iter_bytes(&self) -> impl Iterator<Item = &u8> {
        self.iter().flat_map(|line| line.iter_bytes())
    }
}

impl<'input> Spanned for File<'input> {
    fn span(&self) -> Span {
        self.span
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
/// A sequence of g-code that may be inserted into a file.
///
/// This might be used when verifying user-supplied tool
/// start/stop sequences.
pub struct Snippet<'input> {
    pub(crate) lines: Vec<(Line<'input>, Newline)>,
    pub(crate) last_line: Option<Line<'input>>,
    pub(crate) span: Span,
}

impl<'input> Snippet<'input> {
    /// Iterate by [Line].
    /// The last [Line] may or may not be followed by a [Newline].
    pub fn iter(&self) -> impl Iterator<Item = &Line<'input>> {
        self.lines
            .iter()
            .map(|(line, _)| line)
            .chain(self.last_line.iter())
    }

    /// Iterating by [Line] in a snippet.
    pub fn iter_fields(&self) -> impl Iterator<Item = &Field<'input>> {
        self.iter().flat_map(Line::iter_fields)
    }

    /// Iterate by [InlineComment] in a snippet.
    pub fn iter_inline_comments(&self) -> impl Iterator<Item = &InlineComment<'input>> {
        self.iter().flat_map(Line::iter_inline_comments)
    }

    /// Iterate by [Whitespace] in a snippet.
    pub fn iter_whitespace(&self) -> impl Iterator<Item = &Whitespace<'input>> {
        self.iter().flat_map(Line::iter_whitespace)
    }

    /// Iterate by [u8] in the snippet.
    pub fn iter_bytes(&self) -> impl Iterator<Item = &u8> {
        self.iter().map(|line| line.iter_bytes()).flatten()
    }
}

impl<'input> Spanned for Snippet<'input> {
    fn span(&self) -> Span {
        self.span
    }
}
#[derive(Debug, Clone, PartialEq, Eq)]
/// A sequence of g-code that is either followed by a [Newline] or at the end of a file.
pub struct Line<'input> {
    pub(crate) line_components: Vec<LineComponent<'input>>,
    pub(crate) checksum: Option<Checksum>,
    pub(crate) comment: Option<Comment<'input>>,
    pub(crate) span: Span,
}

impl<'input> Spanned for Line<'input> {
    fn span(&self) -> Span {
        self.span
    }
}

impl<'input> Line<'input> {
    /// Iterate by [Field] in a line of g-code.
    pub fn iter_fields(&self) -> impl Iterator<Item = &Field<'input>> {
        self.line_components.iter().filter_map(|c| c.field.as_ref())
    }

    /// Iterate by [InlineComment] in a line of g-code.
    pub fn iter_inline_comments(&self) -> impl Iterator<Item = &InlineComment<'input>> {
        self.line_components
            .iter()
            .filter_map(|c| c.inline_comment.as_ref())
    }

    /// Iterate by [Whitespace] in a line of g-code.
    pub fn iter_whitespace(&self) -> impl Iterator<Item = &Whitespace<'input>> {
        self.line_components
            .iter()
            .filter_map(|c| c.whitespace.as_ref())
    }

    /// Validate the line's checksum, if any, against its fields.
    ///
    /// Returns [None] if there is no checksum.
    ///
    /// If the line does have a checksum, this will return an empty [Ok]
    /// or an [Err] containing the computed checksum that differs from the actual.
    pub fn validate_checksum(&self) -> Option<Result<(), u8>> {
        if let Some(Checksum {
            inner: checksum, ..
        }) = self.checksum.as_ref()
        {
            let computed_checksum = self.compute_checksum();
            if computed_checksum != *checksum {
                return Some(Err(computed_checksum));
            } else {
                return Some(Ok(()));
            }
        }
        None
    }

    /// Iterate over [u8] in a [Line].
    pub fn iter_bytes(&self) -> impl Iterator<Item = &u8> {
        self.line_components
            .iter()
            .map(|c| c.iter_bytes())
            .flatten()
    }

    /// XORs bytes in a [Line] leading up to the asterisk of a [`Checksum`].
    pub fn compute_checksum(&self) -> u8 {
        let take = if let Some(checksum) = &self.checksum {
            checksum.span.0
        } else if let Some(comment) = &self.comment {
            comment.pos
        } else {
            self.span.1
        } - self.span.0;
        self.iter_bytes().take(take).fold(0u8, |acc, b| acc ^ b)
    }
}
