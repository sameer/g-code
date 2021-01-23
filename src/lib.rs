use lalrpop_util::lalrpop_mod;

lalrpop_mod!(pub parser);

pub mod ast;
pub mod lexer;

#[cfg(test)]
mod tests {
    use super::parser::FileParser;
    use crate::lexer::{Lexer, LexicalError};

    mod parser {
        use super::*;

        #[test]
        fn parses_svg2gcode_output() {
            let gcode = include_str!("../tests/vandy_commodores_logo.gcode");
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
    }

    mod lexer {
        use super::*;
        #[test]
        fn non_ascii_returns_unexpected_character_error() {
            assert_eq!(
                Lexer::new("§").next(),
                Some(Err(LexicalError::UnexpectedCharacter(0, '§')))
            )
        }

        #[test]
        fn unterminated_quote_returns_unexpected_eof_error() {
            assert_eq!(
                Lexer::new(r#""this is a string"#).next(),
                Some(Err(LexicalError::UnexpectedEOF))
            )
        }

        #[test]
        fn unterminated_inline_comment_returns_unexpected_eof_error() {
            assert_eq!(
                Lexer::new("(this is a comment").next(),
                Some(Err(LexicalError::UnexpectedEOF))
            )
        }

        #[test]
        fn unterminated_inline_comment_followed_by_newline_returns_unexpected_newline_error() {
            assert_eq!(
                Lexer::new("(this is a comment\n)").next(),
                Some(Err(LexicalError::UnexpectedNewline))
            )
        }
    }
}
