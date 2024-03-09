/// g-code emitter with a few basic commands and argument-checking
pub mod emit;
/// g-code parser written with [peg]
pub mod parse;

#[cfg(test)]
mod test {
    use crate::{emit::FormatOptions, parse::into_diagnostic};
    use pretty_assertions::assert_eq;

    #[test]
    #[cfg(feature = "codespan_helpers")]
    fn parsing_gcode_then_emitting_then_parsing_again_returns_functionally_identical_gcode() {
        use codespan_reporting::diagnostic::{Diagnostic, Label};
        use codespan_reporting::term::{
            emit,
            termcolor::{ColorChoice, StandardStream},
        };

        let mut writer = StandardStream::stderr(ColorChoice::Auto);
        let config = codespan_reporting::term::Config::default();

        for str in [
            include_str!("../tests/square.gcode"),
            include_str!("../tests/edge_cases.gcode"),
            include_str!("../tests/vandy_commodores_logo.gcode"),
            include_str!("../tests/ncviewer_sample.gcode"),
        ] {
            let parsed_file = crate::parse::file_parser(str).unwrap();
            let emission_tokens = parsed_file.iter_emit_tokens().collect::<Vec<_>>();

            for format_options in [true, false]
                .iter()
                .copied()
                .map(|a| [[a, true], [a, false]])
                .flatten()
                .map(|[a, b]| [[a, b, true], [a, b, false]])
                .flatten()
                .map(|[a, b, c]| [[a, b, c, true], [a, b, c, false]])
                .flatten()
                .map(
                    |[checksums, line_numbers, delimit_with_percent, newline_before_comment]| {
                        FormatOptions {
                            checksums,
                            line_numbers,
                            delimit_with_percent,
                            newline_before_comment,
                        }
                    },
                )
            {
                let mut emitted_gcode = String::new();
                crate::emit::format_gcode_fmt(&emission_tokens, format_options, &mut emitted_gcode)
                    .unwrap();

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

                for line in reparsed_file.iter() {
                    let validated_checksum = line.validate_checksum();
                    assert!(
                        validated_checksum.clone().transpose().is_ok(),
                        "got {:?} for {:#?}",
                        validated_checksum,
                        line
                    );
                }

                parsed_file
                    .iter_fields()
                    .filter(|field| field.letters != "N")
                    .zip(
                        reparsed_file
                            .iter_fields()
                            .filter(|field| field.letters != "N"),
                    )
                    .for_each(|(expected, actual)| {
                        if expected.value != actual.value || expected.letters != actual.letters {
                            emit(
                                &mut writer,
                                &config,
                                &codespan_reporting::files::SimpleFile::new(
                                    "test_input.gcode",
                                    str,
                                ),
                                &Diagnostic::error()
                                    .with_message("fields do not match")
                                    .with_labels(vec![Label::primary((), expected.span)
                                        .with_message("this one here officer")]),
                            )
                            .unwrap();
                        }
                        assert_eq!(
                            expected.letters, actual.letters,
                            "{:?} vs {:?}",
                            expected, actual
                        );
                        assert_eq!(
                            expected.value, actual.value,
                            "{:?} vs {:?}",
                            expected, actual
                        );
                    })
            }
        }
    }
}
