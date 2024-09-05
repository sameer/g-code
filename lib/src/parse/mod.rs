#[cfg(feature = "codespan_helpers")]
use codespan_reporting::diagnostic::{Diagnostic as CodespanDiagnostic, Label};

mod parser;
pub use parser::g_code::{file_parser, snippet_parser};
pub mod ast;
pub mod compact;
pub mod token;

pub type ParseError = peg::error::ParseError<peg::str::LineCol>;
#[cfg(feature = "codespan_helpers")]
pub type Diagnostic = CodespanDiagnostic<()>;

/// Convenience function for converting a parsing error
/// into a [codespan_reporting::diagnostic::Diagnostic] for displaying to a user.
#[cfg(feature = "codespan_helpers")]
pub fn into_diagnostic(err: &ParseError) -> Diagnostic {
    let expected_count = err.expected.tokens().count();
    let label_msg = if expected_count == 0 {
        "unclear cause".to_string()
    } else if expected_count == 1 {
        format!("expected {}", err.expected.tokens().next().unwrap())
    } else {
        let tokens = {
            let mut tokens = err.expected.tokens().collect::<Vec<_>>();
            tokens.sort_unstable();
            tokens
        };
        let mut acc = "expected one of ".to_string();
        for token in tokens.iter().take(expected_count - 1) {
            acc += token;
            acc += ", ";
        }
        acc += "or ";
        acc += tokens.last().unwrap();
        acc
    };
    Diagnostic::error()
        .with_message("could not parse gcode")
        .with_labels(vec![Label::primary(
            (),
            err.location.offset..err.location.offset,
        )
        .with_message(label_msg)])
}

#[cfg(test)]
mod tests {
    use super::file_parser;
    use crate::parse::ast::{Line, Span};
    use crate::parse::token::*;
    use pretty_assertions::assert_eq;

    mod parser {
        use super::super::parser::g_code::*;
        use super::{assert_eq, *};

        #[test]
        fn parses_svg2gcode_output() {
            let gcode = include_str!("../../tests/vandy_commodores_logo.gcode");
            file_parser(gcode).unwrap();
        }

        #[test]
        fn parses_ncviewer_sample() {
            let gcode = include_str!("../../tests/ncviewer_sample.gcode");
            file_parser(gcode).unwrap();
        }

        #[test]
        fn parses_field_with_string_value() {
            let gcode = r#"S"MYROUTER""#;
            file_parser(gcode).unwrap();
        }

        #[test]
        fn parses_field_with_escaped_string_value() {
            let gcode = r#"P"ABCxyz;"" 123""#;
            file_parser(gcode).unwrap();
        }

        #[test]
        fn parses_field_with_complex_string_value() {
            let gcode = r#"
                M587 S"MYROUTER" P"ABCxyz;"" 123" 
                M587 S"MYROUTER" P"ABC'X'Y'Z;"" 123"
            "#;
            file_parser(gcode).unwrap();
        }

        #[test]
        fn parses_fields_without_whitespace() {
            let gcode = "G0X1Y0";
            file_parser(gcode).unwrap();
        }

        #[test]
        fn parses_fields_with_trailing_whitespace() {
            let gcode = "G0 X1 ";
            file_parser(gcode).unwrap();
        }

        #[test]
        fn parses_fields_with_leading_whitespace() {
            let gcode = " G0 X1";
            line(gcode).unwrap();
        }

        #[test]
        fn parses_field_followed_by_inline_comment() {
            let gcode = "M1 ()";
            line(gcode).unwrap();
        }

        #[test]
        fn validates_checksums() {
            let gcode = r#"N0 M106*36
N1 G28*18
N2 M107*39"#;
            let parsed = file_parser(gcode).unwrap();
            for (line, checksum) in parsed.iter().zip(&[36u8, 18u8, 39u8]) {
                assert_eq!(line.compute_checksum(), *checksum);
                assert_eq!(line.validate_checksum(), Some(Ok(())));
            }
        }

        #[test]
        fn checksum_of_empty_line_is_zero() {
            let gcode = "*0";
            let parsed = file_parser(gcode).unwrap();
            assert_eq!(parsed.iter().next().unwrap().compute_checksum(), 0u8);
        }

        #[test]
        fn checksum_of_line_with_inline_comments_is_correct() {
            let gcode = "(inline)G0 X0 (inline) (inline) Y0(inline)";
            let parsed = file_parser(gcode).unwrap();
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
            let parsed = file_parser(gcode).unwrap();
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
            let parsed = file_parser(gcode).unwrap();
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
            let parsed = file_parser(gcode).unwrap();
            assert_eq!(
                *parsed.iter().next().unwrap(),
                Line {
                    line_components: vec![LineComponent {
                        inline_comment: Some(InlineComment {
                            inner: "(comment)",
                            pos: 0
                        }),
                        ..Default::default()
                    }],
                    checksum: None,
                    comment: None,
                    span: Span(0, gcode.len())
                }
            );
        }
    }

    mod lexer {
        use super::super::parser::g_code::*;
        use super::{assert_eq, *};

        #[test]
        fn escaped_quotes_are_lexed() {
            let gcode = r#""""Testing""""#;
            assert_eq!(string(gcode), Ok(gcode));
        }

        #[test]
        fn comment_is_lexed() {
            let gcode = ";Comment";
            assert_eq!(
                comment(gcode),
                Ok(Comment {
                    inner: gcode,
                    pos: 0
                })
            );
        }

        #[test]
        fn letter_is_lexed() {
            let gcode = "A";
            assert_eq!(letter(gcode), Ok(gcode),)
        }

        #[test]
        fn letters_are_lexed() {
            let gcode = "ABCD";
            assert_eq!(letters(gcode), Ok(gcode),)
        }

        #[test]
        fn integer_is_lexed() {
            let gcode = "1234567890";
            assert_eq!(integer(gcode), Ok(gcode),)
        }

        #[test]
        fn dot_is_lexed() {
            let gcode = ".";
            assert_eq!(dot(gcode), Ok(gcode),)
        }

        #[test]
        fn star_is_lexed() {
            let gcode = "*";
            assert_eq!(star(gcode), Ok(gcode),)
        }

        #[test]
        fn minus_is_lexed() {
            let gcode = "-";
            assert_eq!(minus(gcode), Ok(gcode),)
        }

        #[test]
        fn percent_is_lexed() {
            let gcode = "%";
            assert_eq!(percent(gcode), Ok(Percent { pos: 0 }),)
        }

        #[test]
        fn newline_is_lexed() {
            let gcode = "\n";
            assert_eq!(newline(gcode), Ok(Newline { pos: 0 }),)
        }

        #[test]
        fn inline_comment_is_lexed() {
            let gcode = "(Comment)";
            assert_eq!(
                inline_comment(gcode),
                Ok(InlineComment {
                    pos: 0,
                    inner: gcode
                }),
            )
        }

        #[test]
        fn non_ascii_in_string_returns_unexpected_character_error() {
            assert!(string(r#""§""#).is_err());
        }

        #[test]
        fn non_ascii_in_comment_returns_unexpected_character_error() {
            assert!(comment(";§").is_err());
        }

        #[test]
        fn non_ascii_in_inline_comment_returns_unexpected_character_error() {
            assert!(inline_comment("(§)").is_err());
        }

        #[test]
        fn unterminated_quote_returns_unexpected_eof_error() {
            assert!(string("\"x").is_err());
        }

        #[test]
        fn unterminated_inline_comment_returns_unexpected_eof_error() {
            assert!(inline_comment("(x").is_err());
        }

        #[test]
        fn unterminated_inline_comment_followed_by_newline_returns_unexpected_newline_error() {
            assert!(inline_comment("(x\n)").is_err());
        }
    }
}
