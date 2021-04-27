/// GCode emitter with a few basic commands and argument-checking
pub mod emit;
/// GCode parser written with [peg]
pub mod parse;

#[cfg(test)]
mod test {
    #[test]
    fn parsed_gcode_is_functionally_equivalent_to_emitted_gcode_reparsed() {
        let parsed_file = super::parse::file_parser(include_str!("../tests/vandy_commodores_logo.gcode"))
            .unwrap();
        let emission_tokens = parsed_file
            .iter_fields()
            .map(|f| super::emit::Token::from(f))
            .collect::<Vec<_>>();
        let emitted_gcode = emission_tokens
            .iter()
            .map(|t| t.to_string())
            .collect::<Vec<_>>()
            .join(" ");
        let reparsed_file = super::parse::file_parser(&emitted_gcode).unwrap();
        parsed_file.iter_fields()
            .zip(reparsed_file.iter_fields())
            .for_each(|(expected, actual)| {
                assert_eq!(expected.raw_value, actual.raw_value);
            })
    }
}
