//! Fuzzing target that checks if parsed g-code is functionally equivalent when emitted and re-parsed
//!
//! ```bash
//! mkdir -p corpus/fuzz_e2e && cp ../tests/* corpus/fuzz_e2e
//! cargo fuzz run fuzz_e2e -O -- -only_ascii=1
//! ```

#![no_main]

use g_code::{
    emit::{format_gcode_fmt, FormatOptions},
    parse::{file_parser, snippet_parser},
};
use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
    let opts = FormatOptions {
        checksums: true,
        line_numbers: false,
        delimit_with_percent: false,
        newline_before_comment: false,
    };
    let Ok(gcode) = std::str::from_utf8(data) else {
        return;
    };
    let Ok(file) = file_parser(gcode) else { return };

    let mut emitted_gcode = String::new();
    format_gcode_fmt(file.iter_emit_tokens(), opts.clone(), &mut emitted_gcode).unwrap();

    let reparsed_file = file_parser(&emitted_gcode).unwrap();
    for line in reparsed_file.iter() {
        let validate_res = line.validate_checksum().transpose();
        assert!(
            validate_res.is_ok(),
            "Invalid checksum for {line:#?}, should be {validate_res:?}"
        );
    }

    let mut reemitted_gcode = String::new();
    format_gcode_fmt(
        reparsed_file.iter_emit_tokens(),
        opts.clone(),
        &mut reemitted_gcode,
    )
    .unwrap();

    assert_eq!(emitted_gcode, reemitted_gcode);

    let _ = snippet_parser(gcode);
});
