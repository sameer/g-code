use lalrpop_util::lalrpop_mod;

lalrpop_mod!(pub parser);

pub mod ast;
pub mod lexer;

#[cfg(test)]
mod tests {
    use super::lexer;
    use super::parser::FileParser;

    #[test]
    fn parses_svg2gcode_output() {
        let gcode = include_str!("../tests/vandy_commodores_logo.gcode");
        FileParser::new()
            .parse(gcode, lexer::Lexer::new(gcode))
            .unwrap();
    }

    #[test]
    fn parses_field_with_string_value() {
        let gcode = include_str!("../tests/string.gcode");
        FileParser::new()
            .parse(gcode, lexer::Lexer::new(gcode))
            .unwrap();
    }
}
