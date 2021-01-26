use std::num::ParseIntError;
use std::str::CharIndices;

pub type Spanned<Tok, Pos, Error> = Result<(Pos, Tok, Pos), Error>;

/// A lexer for producing the primitive GCode tokens in [`LexTok`].
/// Operates as a [Mealy Machine](https://en.wikipedia.org/wiki/Mealy_machine).
pub struct Lexer<'input> {
    input: &'input str,
    chars: CharIndices<'input>,
    state: LexerState,
}

#[derive(Debug, Clone, PartialEq)]
pub enum LexicalError {
    /// This character is not part of the [ASCII character set](https://en.wikipedia.org/wiki/ASCII),
    /// or its presence does not make sense in the context of GCode (i.e. a stray dollar sign)
    UnexpectedCharacter(usize, char),
    /// A [`LexTok::InlineComment`] started but a [`LexTok::Newline`] was encountered before it was finished.
    UnexpectedNewline,
    /// Input ended prematurely while building a [`LexTok::String`] or [`LexTok::InlineComment`]
    /// both of which require a closing delimiter
    UnexpectedEOF,
    /// A [`LexTok::Integer`] was out of the bounds of a `usize`.
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
    String {
        start: usize,
        prev_could_be_escaped_quote: bool,
    },
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
                        Some((pos, '"')) => {
                            self.state = String {
                                start: pos,
                                prev_could_be_escaped_quote: false,
                            }
                        }
                        Some((pos, '(')) => self.state = InlineComment(pos),
                        Some((pos, ';')) => self.state = Comment(pos),
                        Some((pos, '.')) => self.state = Dot(pos),
                        Some((pos, '*')) => self.state = Star(pos),
                        Some((pos, digit)) if digit.is_ascii_digit() => self.state = Integer(pos),
                        Some((pos, letter)) if letter.is_ascii_alphabetic() => {
                            self.state = Letters(pos)
                        }
                        Some((pos, whitespace))
                            if whitespace.is_ascii_whitespace() && whitespace != '\n' =>
                        {
                            self.state = Whitespace(pos)
                        }
                        Some((pos, c)) => return Some(Err(UnexpectedCharacter(pos, c))),
                        None => return None,
                    };
                }
                Newline(pos) | Dot(pos) | Star(pos) => {
                    let prev_state = self.state.clone();
                    self.state = Init;
                    return Some(Ok((
                        pos,
                        match prev_state {
                            Newline(_) => LexTok::Newline,
                            Dot(_) => LexTok::Dot,
                            Star(_) => LexTok::Star,
                            _ => unreachable!(),
                        },
                        pos + 1,
                    )));
                }
                String {
                    start,
                    prev_could_be_escaped_quote,
                } => match self.chars.next() {
                    Some((_, '"')) => {
                        if !prev_could_be_escaped_quote {
                            self.state = String {
                                start,
                                prev_could_be_escaped_quote: true,
                            }
                        } else {
                            // escaped quote
                            self.state = String {
                                start,
                                prev_could_be_escaped_quote: false,
                            }
                        }
                    }

                    Some((end, c)) if c.is_ascii() => {
                        if prev_could_be_escaped_quote {
                            // string just ended
                            self.state = Init;
                            return Some(Ok((
                                start,
                                LexTok::String(self.input.get(start..end).unwrap()),
                                end,
                            )));
                        }
                    }
                    Some((pos, other)) => return Some(Err(UnexpectedCharacter(pos, other))),
                    None => {
                        if prev_could_be_escaped_quote {
                            self.state = Init;
                            return Some(Ok((
                                start,
                                LexTok::String(self.input.get(start..).unwrap()),
                                self.input.len(),
                            )));
                        } else {
                            return Some(Err(UnexpectedEOF));
                        }
                    }
                },
                InlineComment(start) => match self.chars.next() {
                    Some((_, '\n')) => return Some(Err(UnexpectedNewline)),
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
                    Some((pos, other)) => return Some(Err(UnexpectedCharacter(pos, other))),
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
                    Some((pos, other)) => return Some(Err(UnexpectedCharacter(pos, other))),
                },
                Integer(start) | Letters(start) | Whitespace(start) => {
                    let original_state = self.state.clone();
                    let output = match self.chars.next() {
                        None => {
                            self.state = Init;
                            Some((start, self.input.len()))
                        }
                        Some((end, '\n')) => {
                            self.state = Newline(end);
                            Some((start, end))
                        }
                        Some((end, '.')) => {
                            self.state = Dot(end);
                            Some((start, end))
                        }
                        Some((end, '*')) => {
                            self.state = Star(end);
                            Some((start, end))
                        }
                        Some((end, '"')) => {
                            self.state = String {
                                start: end,
                                prev_could_be_escaped_quote: false,
                            };
                            Some((start, end))
                        }
                        Some((end, '(')) => {
                            self.state = InlineComment(end);
                            Some((start, end))
                        }
                        Some((end, ';')) => {
                            self.state = Comment(end);
                            Some((start, end))
                        }
                        Some((pos, non_ascii)) if !non_ascii.is_ascii() => {
                            return Some(Err(UnexpectedCharacter(pos, non_ascii)));
                        }
                        Some((end, other)) => {
                            if !other.is_ascii_digit()
                                && !other.is_ascii_alphabetic()
                                && !other.is_ascii_whitespace()
                            {
                                return Some(Err(UnexpectedCharacter(end, other)));
                            }
                            let output = if let Letters(_) | Integer(_) = &original_state {
                                if other.is_ascii_whitespace() && other != '\n' {
                                    self.state = Whitespace(end);
                                    Some((start, end))
                                } else {
                                    None
                                }
                            } else {
                                None
                            }
                            .or(if let Whitespace(_) | Integer(_) = &original_state {
                                if other.is_ascii_alphabetic() {
                                    self.state = Letters(end);
                                    Some((start, end))
                                } else {
                                    None
                                }
                            } else {
                                None
                            })
                            .or(
                                if let Whitespace(_) | Letters(_) = &original_state {
                                    if other.is_ascii_digit() {
                                        self.state = Integer(end);
                                        Some((start, end))
                                    } else {
                                        None
                                    }
                                } else {
                                    None
                                },
                            );
                            output
                        }
                    };

                    match output {
                        Some((start, end)) => match original_state {
                            Whitespace(_) => {
                                return Some(Ok((
                                    start,
                                    LexTok::Whitespace(self.input.get(start..end).unwrap()),
                                    end,
                                )))
                            }
                            Letters(_) => {
                                return Some(Ok((
                                    start,
                                    LexTok::Letters(self.input.get(start..end).unwrap()),
                                    end,
                                )))
                            }
                            Integer(_) => {
                                return Some(Ok((
                                    start,
                                    LexTok::Integer(self.input.get(start..end).unwrap()),
                                    end,
                                )))
                            }
                            _ => unreachable!(),
                        },
                        None => {}
                    }
                }
            }
        }
    }
}
