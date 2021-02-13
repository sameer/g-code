use lalrpop_util::{lalrpop_mod, ParseError as LalrpopParseError};

lalrpop_mod!(parser);

use codespan_reporting::diagnostic::{Diagnostic as CodespanDiagnostic, Label};

pub use parser::{FileParser, SnippetParser};

pub mod ast;
pub mod lexer;
pub mod token;

pub type ParseError<'input> = LalrpopParseError<usize, lexer::LexTok<'input>, lexer::LexicalError>;
pub type Diagnostic = CodespanDiagnostic<()>;

/// Convenience function for converting a parsing error
/// into a codespan diagnostic for displaying to a user.
pub fn into_diagnostic<'a: 'input, 'input>(err: &'a ParseError<'input>) -> Diagnostic {
    use lexer::LexicalError::*;
    use LalrpopParseError::*;
    Diagnostic::error()
        .with_message(format!(
            "could not parse GCode: {}",
            match err {
                UnrecognizedToken { .. } => "unexpected token",
                UnrecognizedEOF { .. } => "unexpected end of file",
                InvalidToken { .. } => "invalid token",
                ExtraToken { .. } => "extra token",
                User { error } => match error {
                    UnexpectedCharacter(..) => "unexpected character",
                    UnexpectedNewline(_, state) => match state {
                        lexer::LexerState::InlineComment(_) =>
                            "unexpected newline while building inline comment",
                        _ => unreachable!(),
                    },
                    UnexpectedEOF(_, state) => match state {
                        lexer::LexerState::InlineComment(_) =>
                            "unexpected end of file while building inline comment",
                        lexer::LexerState::String { .. } =>
                            "unexpected end of file while building string",
                        _ => unreachable!(),
                    },
                    ParseIntError(..) => "failed to parse integer",
                    ParseRatioError(..) => "failed to parse ratio",
                },
            }
        ))
        .with_labels({
            let mut labels = vec![];
            match err {
                UnrecognizedToken {
                    token: (left, _token, right),
                    expected,
                } => labels.push(Label::primary((), *left..*right).with_message(format!(
                        "expected {}",
                        expected
                            .iter()
                            .map(|e| lexer::LexTok::lalrpop_name_for_display(e).unwrap_or(e))
                            .collect::<Vec<_>>()
                            .join("|")
                    ))),
                UnrecognizedEOF { location, expected } => labels.push(
                    Label::primary((), *location..*location).with_message(format!(
                        "expected {}",
                        expected
                            .iter()
                            .map(|e| lexer::LexTok::lalrpop_name_for_display(e).unwrap_or(e))
                            .collect::<Vec<_>>()
                            .join("|")
                    )),
                ),
                InvalidToken { location } => {
                    labels.push(Label::primary((), *location..*location + 1))
                }
                ExtraToken {
                    token: (left, _token, right),
                } => labels.push(Label::primary((), *left..*right)),
                User { error } => match error {
                    UnexpectedCharacter(location, _) => {
                        labels.push(Label::primary((), *location..*location + 1))
                    }
                    UnexpectedNewline(pos, state) => match state {
                        lexer::LexerState::InlineComment(start) => {
                            labels.push(
                                Label::primary((), *pos..*pos + 1)
                                    .with_message("expected a closing `)` before this"),
                            );
                            labels.push(
                                Label::secondary((), *start..*start + 1)
                                    .with_message("token starts here".to_owned()),
                            );
                        }
                        _ => unreachable!(),
                    },
                    UnexpectedEOF(pos, state) => match state {
                        lexer::LexerState::String { start, .. }
                        | lexer::LexerState::InlineComment(start) => {
                            labels.push(Label::primary((), *pos..*pos + 1).with_message(format!(
                                "expected a closing `{}` after this",
                                match state {
                                    lexer::LexerState::String { .. } => '\"',
                                    lexer::LexerState::InlineComment(_) => ')',
                                    _ => unreachable!(),
                                },
                            )));
                            labels.push(
                                Label::secondary((), *start..*start + 1)
                                    .with_message("token starts here".to_owned()),
                            );
                        }
                        _ => unreachable!(),
                    },
                    ParseIntError(err, start, end) => labels
                        .push(Label::primary((), *start..*end).with_message(format!("{}", err))),
                    ParseRatioError(err, start, end) => labels
                        .push(Label::primary((), *start..*end).with_message(format!("{}", err))),
                },
            }
            labels
        })
}

#[cfg(test)]
mod tests {
    use super::parser::FileParser;
    use crate::ast::{token::*, Line, Span};
    use crate::lexer::{LexTok, Lexer, LexerState, LexicalError};
    use pretty_assertions::assert_eq;

    mod parser {
        use super::{assert_eq, *};

        #[test]
        fn parses_svg2gcode_output() {
            let gcode = include_str!("../tests/vandy_commodores_logo.gcode");
            FileParser::new().parse(gcode, Lexer::new(gcode)).unwrap();
        }

        #[test]
        fn parses_ncviewer_sample() {
            let gcode = include_str!("../tests/ncviewer_sample.gcode");
            FileParser::new().parse(gcode, Lexer::new(gcode)).unwrap();
        }

        #[test]
        fn parses_field_with_string_value() {
            let gcode = r#"M587 S"MYROUTER" P"ABCxyz;"" 123" 
        M587 S"MYROUTER" P"ABC'X'Y'Z;"" 123""#;
            FileParser::new().parse(gcode, Lexer::new(gcode)).unwrap();
        }

        #[test]
        fn parses_fields_without_whitespace() {
            let gcode = "G0X1Y0";
            FileParser::new().parse(gcode, Lexer::new(gcode)).unwrap();
        }

        #[test]
        fn parses_fields_with_trailing_whitespace() {
            let gcode = "G0 X1 ";
            FileParser::new().parse(gcode, Lexer::new(gcode)).unwrap();
        }

        #[test]
        fn validates_checksums() {
            let gcode = r#"N0 M106*36
N1 G28*18
N2 M107*39"#;
            let parsed = FileParser::new().parse(gcode, Lexer::new(gcode)).unwrap();
            for (line, checksum) in parsed.iter().zip(&[36u8, 18u8, 39u8]) {
                assert_eq!(line.compute_checksum(), *checksum);
                assert_eq!(line.validate_checksum(), Some(Ok(())));
            }
        }

        #[test]
        fn checksum_of_empty_line_is_zero() {
            let gcode = "*0";
            let parsed = FileParser::new().parse(gcode, Lexer::new(gcode)).unwrap();
            assert_eq!(parsed.iter().next().unwrap().compute_checksum(), 0u8);
        }

        #[test]
        fn checksum_of_line_with_inline_comments_is_correct() {
            let gcode = "(inline)G0 X0 (inline) (inline) Y0(inline)";
            let parsed = FileParser::new().parse(gcode, Lexer::new(gcode)).unwrap();
            assert_eq!(
                parsed
                    .iter()
                    .next()
                    .unwrap()
                    .iter_bytes()
                    .copied()
                    .collect::<Vec<u8>>(),
                gcode.as_bytes()
            );
            assert_eq!(
                parsed.iter().next().unwrap().compute_checksum(),
                gcode.as_bytes().iter().fold(0u8, |acc, x| acc ^ x)
            );
        }

        #[test]
        fn checksum_of_line_with_comment_is_correct() {
            let gcode = "(inline)G0 X0 (inline) (inline) Y0(inline);eolcomment";
            let parsed = FileParser::new().parse(gcode, Lexer::new(gcode)).unwrap();
            assert_eq!(
                parsed.iter().next().unwrap().compute_checksum(),
                gcode
                    .split(';')
                    .next()
                    .unwrap()
                    .as_bytes()
                    .iter()
                    .fold(0u8, |acc, x| acc ^ x)
            );
        }

        #[test]
        fn checksum_of_line_with_checkum_and_comment_is_correct() {
            let gcode = "(inline)G0 X0 (inline) (inline) Y0(inline)*118;eolcomment";
            let parsed = FileParser::new().parse(gcode, Lexer::new(gcode)).unwrap();
            assert_eq!(
                parsed.iter().next().unwrap().validate_checksum(),
                Some(Ok(()))
            );
            assert_eq!(
                parsed.iter().next().unwrap().compute_checksum(),
                gcode
                    .split('*')
                    .next()
                    .unwrap()
                    .as_bytes()
                    .iter()
                    .fold(0u8, |acc, x| acc ^ x)
            );
        }

        #[test]
        fn inline_comment_is_parsed() {
            let gcode = "(comment)";
            let parsed = FileParser::new().parse(gcode, Lexer::new(gcode)).unwrap();
            assert_eq!(
                *parsed.iter().next().unwrap(),
                Line {
                    fields: vec![],
                    checksum: None,
                    comment: None,
                    whitespace: None,
                    inline_comment: vec![InlineComment {
                        inner: "(comment)",
                        pos: 0
                    }],
                    span: Span(0, gcode.len())
                }
            );
        }
    }

    mod lexer {
        use super::{assert_eq, *};

        #[test]
        fn escaped_quotes_are_lexed() {
            let gcode = r#""""Testing""""#;
            assert_eq!(
                Lexer::new(gcode).next(),
                Some(Ok((0, LexTok::String(gcode), gcode.len())))
            )
        }

        #[test]
        fn comment_is_lexed() {
            let gcode = ";Comment";
            assert_eq!(
                Lexer::new(gcode).next(),
                Some(Ok((0, LexTok::Comment(gcode), gcode.len())))
            )
        }

        #[test]
        fn letters_are_lexed() {
            let gcode = "ABCD";
            assert_eq!(
                Lexer::new(gcode).next(),
                Some(Ok((0, LexTok::Letters(gcode), gcode.len())))
            )
        }

        #[test]
        fn integer_is_lexed() {
            let gcode = "1234567890";
            assert_eq!(
                Lexer::new(gcode).next(),
                Some(Ok((0, LexTok::Integer(gcode), gcode.len())))
            )
        }

        #[test]
        fn dot_is_lexed() {
            let gcode = ".";
            assert_eq!(
                Lexer::new(gcode).next(),
                Some(Ok((0, LexTok::Dot, gcode.len())))
            )
        }

        #[test]
        fn star_is_lexed() {
            let gcode = "*";
            assert_eq!(
                Lexer::new(gcode).next(),
                Some(Ok((0, LexTok::Star, gcode.len())))
            )
        }

        #[test]
        fn minus_is_lexed() {
            let gcode = "-";
            assert_eq!(
                Lexer::new(gcode).next(),
                Some(Ok((0, LexTok::Minus, gcode.len())))
            )
        }

        #[test]
        fn percent_is_lexed() {
            let gcode = "%";
            assert_eq!(
                Lexer::new(gcode).next(),
                Some(Ok((0, LexTok::Percent, gcode.len())))
            )
        }

        #[test]
        fn newline_is_lexed() {
            let gcode = "\n";
            assert_eq!(
                Lexer::new(gcode).next(),
                Some(Ok((0, LexTok::Newline, gcode.len())))
            )
        }

        #[test]
        fn inline_comment_is_lexed() {
            let gcode = "(Comment)";
            assert_eq!(
                Lexer::new(gcode).next(),
                Some(Ok((0, LexTok::InlineComment(gcode), gcode.len())))
            )
        }

        #[test]
        fn non_ascii_returns_unexpected_character_error() {
            assert_eq!(
                Lexer::new("§").next(),
                Some(Err(LexicalError::UnexpectedCharacter(0, '§')))
            )
        }

        #[test]
        fn non_ascii_in_string_returns_unexpected_character_error() {
            assert_eq!(
                Lexer::new(r#""§""#).next(),
                Some(Err(LexicalError::UnexpectedCharacter(1, '§')))
            )
        }

        #[test]
        fn non_ascii_in_comment_returns_unexpected_character_error() {
            assert_eq!(
                Lexer::new(";§").next(),
                Some(Err(LexicalError::UnexpectedCharacter(1, '§')))
            )
        }

        #[test]
        fn non_ascii_in_inline_comment_returns_unexpected_character_error() {
            assert_eq!(
                Lexer::new("(§)").next(),
                Some(Err(LexicalError::UnexpectedCharacter(1, '§')))
            )
        }

        #[test]
        fn unterminated_quote_returns_unexpected_eof_error() {
            assert_eq!(
                Lexer::new(r#""x"#).next(),
                Some(Err(LexicalError::UnexpectedEOF(
                    1,
                    LexerState::String {
                        start: 0,
                        prev_could_be_escaped_quote: false,
                    }
                )))
            )
        }

        #[test]
        fn unterminated_inline_comment_returns_unexpected_eof_error() {
            assert_eq!(
                Lexer::new("(x").next(),
                Some(Err(LexicalError::UnexpectedEOF(
                    1,
                    LexerState::InlineComment(0)
                )))
            )
        }

        #[test]
        fn unterminated_inline_comment_followed_by_newline_returns_unexpected_newline_error() {
            assert_eq!(
                Lexer::new("(x\n)").next(),
                Some(Err(LexicalError::UnexpectedNewline(
                    2,
                    LexerState::InlineComment(0)
                )))
            )
        }
    }
}
