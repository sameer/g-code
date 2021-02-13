use super::token::*;
use std::fmt::Debug;

#[derive(Clone, Copy, Debug, PartialEq, Hash, Eq)]
/// A range of bytes in the raw text of the program.
/// Useful for providing diagnostic information in
/// higher-level tooling.
///
/// The end of the range is exclusive.
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
impl Into<std::ops::Range<usize>> for Span {
    fn into(self) -> std::ops::Range<usize> {
        self.0..self.1
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
/// Representation of a sequence of GCode logically organized as a file.
/// This may also be referred to as a program.
pub struct File<'input> {
    pub(crate) start_percent: bool,
    pub(crate) lines: Vec<(Line<'input>, Newline)>,
    pub(crate) last_line: Option<Line<'input>>,
    pub(crate) end_percent: bool,
    pub(crate) span: Span,
}

impl<'input> File<'input> {
    /// Iterate by [`Line`].
    /// The last [`Line`] may or may not be followed by a [`Newline`].
    pub fn iter(&'input self) -> impl Iterator<Item = &'input Line<'input>> {
        self.lines
            .iter()
            .map(|(line, _)| line)
            .chain(self.last_line.iter())
    }

    /// Iterating by [`Line`] may be too verbose, so this method is offered as
    /// an alternative for directly examining each [`Field`].
    pub fn iter_fields(&'input self) -> impl Iterator<Item = &'input Field<'input>> {
        self.iter().map(|line| line.iter_fields()).flatten()
    }

    /// Iterate by [`u8`]. This will return bytes identical to [`str::as_bytes`].[`slice.iter`].
    pub fn iter_bytes(&'input self) -> impl Iterator<Item = &'input u8> {
        self.iter().map(|line| line.iter_bytes()).flatten()
    }
}

impl<'input> Spanned for File<'input> {
    fn span(&self) -> Span {
        self.span
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
/// A sequence of GCode that may be inserted into a file.
///
/// This might be used when verifying user-supplied tool
/// start/stop sequences.
pub struct Snippet<'input> {
    pub(crate) lines: Vec<(Line<'input>, Newline)>,
    pub(crate) last_line: Option<Line<'input>>,
    pub(crate) span: Span,
}

impl<'input> Snippet<'input> {
    /// Iterate by [`Line`].
    /// The last [`Line`] may or may not be followed by a [`Newline`].
    pub fn iter(&'input self) -> impl Iterator<Item = &'input Line<'input>> {
        self.lines
            .iter()
            .map(|(line, _)| line)
            .chain(self.last_line.iter())
    }

    /// Iterating by [`Line`] may be too verbose, so this method is offered as
    /// an alternative for directly examining each [`Field`].
    pub fn iter_fields<'a>(&'a self) -> impl Iterator<Item = &'a Field> {
        self.iter().map(|line| line.iter_fields()).flatten()
    }

    /// Iterate by [`u8`]. This will return bytes identical to [`str::as_bytes`].[`slice.iter`].
    pub fn iter_bytes(&'input self) -> impl Iterator<Item = &'input u8> {
        self.iter().map(|line| line.iter_bytes()).flatten()
    }
}

impl<'input> Spanned for Snippet<'input> {
    fn span(&self) -> Span {
        self.span
    }
}

type PrecedingWhitespaceAndComments<'input> = Vec<(Whitespace<'input>, Vec<InlineComment<'input>>)>;
type WrappedVec<'input, T> = Vec<(
    Vec<InlineComment<'input>>,
    PrecedingWhitespaceAndComments<'input>,
    T,
)>;
type WrappedOpt<'input, T> = Option<(
    Vec<InlineComment<'input>>,
    PrecedingWhitespaceAndComments<'input>,
    T,
)>;

#[derive(Debug, Clone, PartialEq, Eq)]
/// A sequence of GCode that is either followed by a [`Newline`] or at the end of a file.
pub struct Line<'input> {
    pub(crate) fields: WrappedVec<'input, Field<'input>>,
    pub(crate) checksum: WrappedOpt<'input, Checksum>,
    pub(crate) comment: WrappedOpt<'input, Comment<'input>>,
    pub(crate) whitespace: Option<(PrecedingWhitespaceAndComments<'input>, Whitespace<'input>)>,
    pub(crate) inline_comment: Vec<InlineComment<'input>>,
    pub(crate) span: Span,
}

impl<'input> Spanned for Line<'input> {
    fn span(&self) -> Span {
        self.span
    }
}

impl<'input> Line<'input> {
    /// Iterate by [`Field`] in a line of GCode.
    pub fn iter_fields<'a>(&'a self) -> impl Iterator<Item = &'a Field> {
        self.fields.iter().map(|(_, _, field)| field)
    }

    /// Validates [`Line::checksum`] against the fields that the line contains.
    /// If the line has no checksum, this will return [`Option::None`].
    ///
    /// If the line does have a checksum, this will return an empty [`Result::Ok`]
    /// or an [`Result::Err`] containing the computed checksum that differs from the actual.
    pub fn validate_checksum(&'input self) -> Option<Result<(), u8>> {
        if let Some((
            _,
            _,
            Checksum {
                inner: checksum, ..
            },
        )) = self.checksum.as_ref()
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

    /// Iterate over ALL [`u8`] in a line.
    pub fn iter_bytes(&'input self) -> impl Iterator<Item = &'input u8> {
        self.fields
            .iter()
            .map(|(comments, whitespace, field)| {
                comments.iter().map(|c| c.iter_bytes()).flatten().chain(
                    whitespace
                        .iter()
                        .map(|w| {
                            w.0.iter_bytes()
                                .chain(w.1.iter().map(|c| c.iter_bytes()).flatten())
                        })
                        .flatten()
                        .chain(field.iter_bytes()),
                )
            })
            .flatten()
            .chain(
                self.checksum
                    .iter()
                    .map(|(comments, w, _checksum)| {
                        comments.iter().map(|c| c.iter_bytes()).flatten().chain(
                            w.iter()
                                .map(|w| {
                                    w.0.iter_bytes().chain(
                                        w.1.iter().map(|comment| comment.iter_bytes()).flatten(),
                                    )
                                })
                                .flatten(),
                        )
                    })
                    .flatten(),
            )
            .chain(
                self.comment
                    .iter()
                    .map(|(comments, whitespace_plus_comments, comment)| {
                        comments
                            .iter()
                            .map(|c| c.iter_bytes())
                            .flatten()
                            .chain(
                                whitespace_plus_comments
                                    .iter()
                                    .map(|(whitespace, comments)| {
                                        whitespace.iter_bytes().chain(
                                            comments.iter().map(|c| c.iter_bytes()).flatten(),
                                        )
                                    })
                                    .flatten(),
                            )
                            .chain(comment.iter_bytes())
                    })
                    .flatten(),
            )
            .chain(
                self.whitespace
                    .iter()
                    .map(|(whitespace_plus_comments, whitespace)| {
                        whitespace_plus_comments
                            .iter()
                            .map(|(w, comments)| {
                                w.iter_bytes().chain(
                                    comments
                                        .iter()
                                        .map(|comment| comment.iter_bytes())
                                        .flatten(),
                                )
                            })
                            .flatten()
                            .chain(whitespace.iter_bytes())
                    })
                    .flatten(),
            )
            .chain(
                self.inline_comment
                    .iter()
                    .map(|comment| comment.iter_bytes())
                    .flatten(),
            )
    }

    /// XORs bytes in a [`Line`] leading up to the asterisk of a [`Checksum`].
    pub fn compute_checksum(&'input self) -> u8 {
        let take = if let Some((_, _, checksum)) = &self.checksum {
            checksum.span.0
        } else if let Some((_, _, comment)) = &self.comment {
            comment.pos
        } else {
            self.span.1
        } - self.span.0;
        self.iter_bytes().take(take).fold(0u8, |acc, b| acc ^ b)
    }
}

pub mod token {}
