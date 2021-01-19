use lalrpop_util::lalrpop_mod;

lalrpop_mod!(pub parser);

pub mod ast {
    use std::fmt::Debug;
    use token::*;

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
}

mod lexer {
    use crate::ast::token::*;
    use std::num::ParseIntError;
    use std::str::CharIndices;

    pub type Spanned<Tok, Pos, Error> = Result<(Pos, Tok, Pos), Error>;
    pub struct Lexer<'input> {
        input: &'input str,
        chars: CharIndices<'input>,
        state: LexerState,
    }

    #[derive(Debug, Clone, PartialEq)]
    pub enum LexicalError {
        /// This character is not ascii or does not make sense in the context of GCode
        UnexpectedCharacter((usize, char)),
        UnexpectedNewline,
        UnexpectedEOF,
        ParseIntError(ParseIntError),
    }

    #[derive(Debug, Clone, PartialEq)]
    pub enum LexTok<'input> {
        Newline,
        Dot,
        Star,
        String(&'input str),
        InlineComment(&'input str),
        Comment(&'input str),
        Integer(&'input str),
        Letters(&'input str),
        Whitespace(&'input str),
    }

    #[derive(Debug, Clone, PartialEq)]
    enum LexerState {
        Init,
        Newline(usize),
        Dot(usize),
        Star(usize),
        String(usize),
        InlineComment(usize),
        Comment(usize),
        Integer(usize),
        Letters(usize),
        Whitespace(usize),
    }

    impl Default for LexerState {
        fn default() -> Self {
            Self::Init
        }
    }

    impl<'input> Lexer<'input> {
        pub fn new(input: &'input str) -> Self {
            Lexer {
                input,
                chars: input.char_indices(),
                state: Default::default(),
            }
        }
    }

    impl<'input> Iterator for Lexer<'input> {
        type Item = Spanned<LexTok<'input>, usize, LexicalError>;
        fn next(&mut self) -> Option<Self::Item> {
            use LexerState::*;
            use LexicalError::*;
            loop {
                match self.state {
                    Init => {
                        match self.chars.next() {
                            Some((pos, '\n')) => self.state = Newline(pos),
                            Some((pos, '"')) => self.state = String(pos),
                            Some((pos, '(')) => self.state = InlineComment(pos),
                            Some((pos, ';')) => self.state = Comment(pos),
                            Some((pos, '.')) => self.state = Dot(pos),
                            Some((pos, '*')) => self.state = Star(pos),
                            Some((pos, digit)) if digit.is_ascii_digit() => {
                                self.state = Integer(pos)
                            }
                            Some((pos, letter)) if letter.is_ascii_alphabetic() => {
                                self.state = Letters(pos)
                            }
                            Some((pos, whitespace))
                                if whitespace.is_ascii_whitespace() && whitespace != '\n' =>
                            {
                                self.state = Whitespace(pos)
                            }
                            Some((pos, c)) => return Some(Err(UnexpectedCharacter((pos, c)))),
                            None => return None,
                        };
                    }
                    Newline(pos) => {
                        self.state = Init;
                        return Some(Ok((pos, LexTok::Newline, pos)));
                    }
                    Dot(pos) => {
                        self.state = Init;
                        return Some(Ok((pos, LexTok::Dot, pos)));
                    }
                    Star(pos) => {
                        self.state = Init;
                        return Some(Ok((pos, LexTok::Star, pos)));
                    }
                    String(_) => {
                        unimplemented!();
                    }
                    InlineComment(start) => match self.chars.next() {
                        Some((pos, '\n')) => return Some(Err(UnexpectedNewline)),
                        Some((end, ')')) => {
                            self.state = Init;
                            return Some(Ok((
                                start,
                                LexTok::InlineComment(self.input.get(start..=end).unwrap()),
                                // inclusive of last character
                                end + 1,
                            )));
                        }
                        Some((_, c)) if c.is_ascii() => {}
                        Some((pos, other)) => return Some(Err(UnexpectedCharacter((pos, other)))),
                        None => return Some(Err(UnexpectedEOF)),
                    },
                    Comment(start) => match self.chars.next() {
                        None => {
                            self.state = Init;
                            return Some(Ok((
                                start,
                                LexTok::Comment(self.input.get(start..).unwrap()),
                                self.input.len(),
                            )));
                        }
                        Some((end, '\n')) => {
                            self.state = Newline(end);
                            return Some(Ok((
                                start,
                                LexTok::Comment(self.input.get(start..end).unwrap()),
                                end,
                            )));
                        }
                        Some((_, c)) if c.is_ascii() => {}
                        Some((pos, other)) => return Some(Err(UnexpectedCharacter((pos, other)))),
                    },
                    Integer(start) => match self.chars.next() {
                        None => {
                            self.state = Init;
                            return Some(Ok((
                                start,
                                LexTok::Integer(self.input.get(start..).unwrap()),
                                self.input.len(),
                            )));
                        }
                        Some((end, whitespace))
                            if whitespace.is_ascii_whitespace() && whitespace != '\n' =>
                        {
                            self.state = Whitespace(end);
                            return Some(Ok((
                                start,
                                LexTok::Integer(self.input.get(start..end).unwrap()),
                                end,
                            )));
                        }
                        Some((end, letter)) if letter.is_ascii_alphabetic() => {
                            self.state = Letters(end);
                            return Some(Ok((
                                start,
                                LexTok::Integer(self.input.get(start..end).unwrap()),
                                end,
                            )));
                        }
                        Some((end, '\n')) => {
                            self.state = Newline(end);
                            return Some(Ok((
                                start,
                                LexTok::Integer(self.input.get(start..end).unwrap()),
                                end,
                            )));
                        }
                        Some((end, '.')) => {
                            self.state = Dot(end);
                            return Some(Ok((
                                start,
                                LexTok::Integer(self.input.get(start..end).unwrap()),
                                end,
                            )));
                        }
                        Some((end, '*')) => {
                            self.state = Star(end);
                            return Some(Ok((
                                start,
                                LexTok::Integer(self.input.get(start..end).unwrap()),
                                end,
                            )));
                        }
                        Some((_, digit)) if digit.is_ascii_digit() => {}
                        Some((pos, other)) => return Some(Err(UnexpectedCharacter((pos, other)))),
                    },
                    Letters(start) => match self.chars.next() {
                        None => {
                            self.state = Init;
                            return Some(Ok((
                                start,
                                LexTok::Letters(self.input.get(start..).unwrap()),
                                self.input.len(),
                            )));
                        }
                        Some((end, whitespace))
                            if whitespace.is_ascii_whitespace() && whitespace != '\n' =>
                        {
                            self.state = Whitespace(end);
                            return Some(Ok((
                                start,
                                LexTok::Letters(self.input.get(start..end).unwrap()),
                                end,
                            )));
                        }
                        Some((end, digit)) if digit.is_ascii_digit() => {
                            self.state = Integer(end);
                            return Some(Ok((
                                start,
                                LexTok::Letters(self.input.get(start..end).unwrap()),
                                end,
                            )));
                        }
                        Some((end, '\n')) => {
                            self.state = Newline(end);
                            return Some(Ok((
                                start,
                                LexTok::Letters(self.input.get(start..end).unwrap()),
                                end,
                            )));
                        }
                        Some((end, '.')) => {
                            self.state = Dot(end);
                            return Some(Ok((
                                start,
                                LexTok::Letters(self.input.get(start..end).unwrap()),
                                end,
                            )));
                        }
                        Some((end, '*')) => {
                            self.state = Star(end);
                            return Some(Ok((
                                start,
                                LexTok::Letters(self.input.get(start..end).unwrap()),
                                end,
                            )));
                        }
                        Some((_, letter)) if letter.is_ascii_alphabetic() => {}
                        Some((pos, other)) => return Some(Err(UnexpectedCharacter((pos, other)))),
                    },
                    Whitespace(start) => match self.chars.next() {
                        None => {
                            self.state = Init;
                            return Some(Ok((
                                start,
                                LexTok::Whitespace(self.input.get(start..).unwrap()),
                                self.input.len(),
                            )));
                        }
                        Some((end, letter)) if letter.is_ascii_alphabetic() => {
                            self.state = Letters(end);
                            return Some(Ok((
                                start,
                                LexTok::Whitespace(self.input.get(start..end).unwrap()),
                                end,
                            )));
                        }
                        Some((end, digit)) if digit.is_ascii_digit() => {
                            self.state = Integer(end);
                            return Some(Ok((
                                start,
                                LexTok::Whitespace(self.input.get(start..end).unwrap()),
                                end,
                            )));
                        }
                        Some((end, '\n')) => {
                            self.state = Newline(end);
                            return Some(Ok((
                                start,
                                LexTok::Whitespace(self.input.get(start..end).unwrap()),
                                end,
                            )));
                        }
                        Some((end, '.')) => {
                            self.state = Dot(end);
                            return Some(Ok((
                                start,
                                LexTok::Whitespace(self.input.get(start..end).unwrap()),
                                end,
                            )));
                        }
                        Some((end, '*')) => {
                            self.state = Star(end);
                            return Some(Ok((
                                start,
                                LexTok::Whitespace(self.input.get(start..end).unwrap()),
                                end,
                            )));
                        }
                        Some((_, whitespace))
                            if whitespace.is_ascii_whitespace() && whitespace != '\n' => {}
                        Some((pos, other)) => return Some(Err(UnexpectedCharacter((pos, other)))),
                    },
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn parses_svg2gcode_output_correctly() {
        use super::lexer;
        use super::parser::FileParser;
        let gcode = include_str!("../tests/square_transformed.gcode");
        FileParser::new()
            .parse(gcode, lexer::Lexer::new(gcode))
            .unwrap();
    }
}
