use std::fmt::Debug;
use token::*;

#[derive(Debug, Clone, PartialEq, Eq)]
/// Representation of a sequence of GCode logically organized as a file.
pub struct File<'input> {
    pub(crate) lines: Vec<(Line<'input>, Newline)>,
    pub(crate) last_line: Option<Line<'input>>,
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
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
/// A sequence of GCode that is either followed by a [`Newline`] or at the end of a file.
pub struct Line<'input> {
    pub fields: Vec<(
        Option<InlineComment<'input>>,
        Option<(Whitespace<'input>, Option<InlineComment<'input>>)>,
        Field<'input>,
    )>,
    pub checksum: Option<(
        Option<InlineComment<'input>>,
        Option<(Whitespace<'input>, Option<InlineComment<'input>>)>,
        Checksum,
    )>,
    pub comment: Option<(
        Option<InlineComment<'input>>,
        Option<(Whitespace<'input>, Option<InlineComment<'input>>)>,
        Comment<'input>,
    )>,
    pub whitespace: Option<(Option<InlineComment<'input>>, Whitespace<'input>)>,
    pub inline_comment: Option<InlineComment<'input>>,
}

impl<'input> Line<'input> {
    /// Iterate by [`Field`] in a line of GCode.
    pub fn iter_fields(&'input self) -> impl Iterator<Item = &'input Field> {
        self.fields.iter().map(|(_, _, field)| field)
    }

    /// Validates [`Line::checksum`] against the fields that the line contains.
    /// If the line has no checksum, this will return [`Optional::None`].
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
        return None;
    }

    /// XORs bytes in a [`Line`] leading up to the asterisk of a [`Checksum`].
    pub fn compute_checksum(&'input self) -> u8 {
        self.fields
            .iter()
            .map(|(comment, whitespace, field)| {
                comment
                    .iter()
                    .map(|comment| comment.iter_bytes())
                    .flatten()
                    .chain(
                        whitespace
                            .iter()
                            .map(|w| {
                                w.0.iter_bytes()
                                    .chain(w.1.iter().map(|comment| comment.iter_bytes()).flatten())
                            })
                            .flatten()
                            .chain(field.iter_bytes()),
                    )
            })
            .flatten()
            .chain(
                self.checksum
                    .iter()
                    .map(|(comment, w, _checksum)| {
                        comment
                            .iter()
                            .map(|comment| comment.iter_bytes())
                            .flatten()
                            .chain(
                                w.iter()
                                    .map(|w| {
                                        w.0.iter_bytes().chain(
                                            w.1.iter()
                                                .map(|comment| comment.iter_bytes())
                                                .flatten(),
                                        )
                                    })
                                    .flatten(),
                            )
                    })
                    .flatten(),
            )
            .fold(0u8, |acc, b| acc ^ b)
    }
}

pub mod token {
    use std::cmp::PartialEq;

    pub trait Spanned {
        fn span(&self) -> Span;
    }

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
        /// Used in [`Line::compute_checksum`].
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
        /// While this kind of value is often a floating point
        /// number, it is represented as a mixed fraction to
        /// ensure numerical stability.
        Rational(usize, usize),
        /// An integer GCode value fitting in a [`usize`].
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
    /// A `'\n'` token that delimits [`Line`]s in a [`File`].
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

    #[derive(Clone, Copy, Debug, PartialEq, Hash, Eq)]
    /// A range of bytes in the raw text of the program.
    /// Useful for providing diagnostic information in
    /// higher-level tooling.
    ///
    /// The end of the range is exclusive.
    pub struct Span(pub usize, pub usize);

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
}
