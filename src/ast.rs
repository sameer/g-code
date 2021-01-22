use std::fmt::Debug;
use token::*;

#[derive(Debug, Clone, PartialEq)]
pub struct File<'input> {
    pub lines: Vec<(Line<'input>, Newline)>,
    pub last_line: Option<Line<'input>>,
}

impl<'input> File<'input> {
    pub fn iter<'a>(&'a self) -> impl Iterator<Item = &'a Line<'input>> {
        self.lines
            .iter()
            .map(|(line, _)| line)
            .chain(self.last_line.iter())
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct Line<'input> {
    pub fields: Vec<(Option<Whitespace<'input>>, Field<'input>)>,
    pub checksum: Option<(Option<Whitespace<'input>>, Checksum)>,
    pub comment: Option<(Option<Whitespace<'input>>, Comment<'input>)>,
    pub whitespace: Option<Whitespace<'input>>,
}

pub mod token {
    use std::cmp::PartialEq;

    pub trait Spanned {
        fn span(&self) -> Span;
    }

    #[derive(Debug, Clone, PartialEq)]
    pub struct Field<'input> {
        pub(crate) letters: &'input str,
        pub(crate) value: Value<'input>,
        pub(crate) span: Span,
    }

    impl<'input> Spanned for Field<'input> {
        fn span(&self) -> Span {
            self.span
        }
    }

    #[derive(Debug, Clone, PartialEq)]
    pub enum Value<'input> {
        Rational(usize, usize),
        Integer(usize),
        String(&'input str),
    }

    #[derive(Debug, Clone, PartialEq)]
    pub struct Checksum {
        pub(crate) inner: u8,
        pub(crate) span: Span,
    }

    impl Spanned for Checksum {
        fn span(&self) -> Span {
            self.span
        }
    }

    #[derive(Debug, Clone, PartialEq)]
    pub struct Newline {
        pub(crate) pos: usize,
    }

    impl Spanned for Newline {
        fn span(&self) -> Span {
            Span(self.pos, self.pos + 1)
        }
    }

    #[derive(Debug, Clone, PartialEq)]
    pub struct Whitespace<'input> {
        pub(crate) inner: &'input str,
        pub(crate) pos: usize,
    }

    impl<'input> Spanned for Whitespace<'input> {
        fn span(&self) -> Span {
            Span(self.pos, self.pos + self.inner.len())
        }
    }

    #[derive(Debug, Clone, PartialEq)]
    pub struct Comment<'input> {
        pub(crate) inner: &'input str,
        pub(crate) pos: usize,
    }

    #[derive(Debug, Clone, PartialEq)]
    pub struct InlineComment<'input> {
        pub(crate) inner: &'input str,
        pub(crate) pos: usize,
    }

    impl<'input> Spanned for Comment<'input> {
        fn span(&self) -> Span {
            Span(self.pos, self.pos + self.inner.len())
        }
    }

    #[derive(Clone, Copy, Debug, PartialEq, Hash, Eq)]
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
