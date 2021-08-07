/// g-code emitter with a few basic commands and argument-checking
pub mod emit;
/// g-code parser written with [peg]
pub mod parse;

#[cfg(test)]
mod test {
    use crate::parse::into_diagnostic;
    use pretty_assertions::assert_eq;

    #[test]
    #[cfg(feature = "codespan_helpers")]
    fn parsing_gcode_then_emitting_then_parsing_again_returns_identical_gcode() {
        use codespan_reporting::diagnostic::{Diagnostic, Label};
        use codespan_reporting::term::{
            emit,
            termcolor::{ColorChoice, StandardStream},
        };
        use std::fmt::Write;

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
            let emission_tokens = parsed_file.iter_emit_tokens().collect::<Vec<_>>();

            let mut emitted_gcode = String::new();
            for token in emission_tokens {
                write!(&mut emitted_gcode, "{}", token).unwrap();
            }
            assert!(str == emitted_gcode);

            let reparsed_file = match super::parse::file_parser(&emitted_gcode) {
                Ok(reparsed) => reparsed,
                Err(err) => {
                    emit(
                        &mut writer,
                        &config,
                        &codespan_reporting::files::SimpleFile::new(
                            "test_input.gcode",
                            emitted_gcode,
                        ),
                        &into_diagnostic(&err),
                    )
                    .unwrap();
                    panic!("{}", err);
                }
            };
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
