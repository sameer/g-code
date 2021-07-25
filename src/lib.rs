/// GCode emitter with a few basic commands and argument-checking
pub mod emit;
/// GCode parser written with [peg]
pub mod parse;

#[cfg(test)]
mod test {
    #[test]
    #[cfg(feature = "codespan_helpers")]
    fn parsing_gcode_then_emitting_then_parsing_again_returns_functionally_equivalent_gcode() {
        use codespan_reporting::diagnostic::{Diagnostic, Label};
        use codespan_reporting::term::{
            emit,
            termcolor::{ColorChoice, StandardStream},
        };
        let mut writer = StandardStream::stderr(ColorChoice::Auto);
        let config = codespan_reporting::term::Config::default();

        for str in [
            include_str!("../tests/square.gcode"),
            include_str!("../tests/string_field.gcode"),
            include_str!("../tests/zero_dot.gcode"),
            include_str!("../tests/vandy_commodores_logo.gcode"),
            include_str!("../tests/ncviewer_sample.gcode"),
        ] {
            let parsed_file = crate::parse::file_parser(str).unwrap();
            let emission_tokens = parsed_file
                .iter_fields()
                .map(crate::emit::Token::from)
                .collect::<Vec<_>>();
            let emitted_gcode = emission_tokens
                .iter()
                .map(ToString::to_string)
                .collect::<Vec<_>>()
                .join(" ");
            let reparsed_file = super::parse::file_parser(&emitted_gcode).unwrap();
            parsed_file
                .iter_fields()
                .zip(reparsed_file.iter_fields())
                .for_each(|(expected, actual)| {
                    if expected.raw_value != actual.raw_value {
                        emit(
                            &mut writer,
                            &config,
                            &codespan_reporting::files::SimpleFile::new("test_input.gcode", str),
                            &Diagnostic::error()
                                .with_message("fields do not match")
                                .with_labels(vec![Label::primary((), expected.span)
                                    .with_message("this one here officer")]),
                        )
                        .unwrap();
                    }
                    assert_eq!(
                        expected.raw_value, actual.raw_value,
                        "{:?} vs {:?}",
                        expected, actual
                    );
                })
        }
    }
}
