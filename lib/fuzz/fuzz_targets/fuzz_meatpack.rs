//! Fuzzing target for meatpacked g-code parsing/emission
//!
//! ```bash
//! mkdir -p corpus/fuzz_meatpack && cp ../tests/* corpus/fuzz_meatpack
//! cargo fuzz run fuzz_meatpack -O -- -only_ascii
//! ```

#![no_main]

use g_code::{
    emit::{
        compact::{format_gcode_meatpack, MeatpackOptions},
        FormatOptions,
    },
    parse::{compact::meatpacked_to_string, file_parser},
};

use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
    let opts = FormatOptions {
        checksums: false,
        line_numbers: false,
        delimit_with_percent: false,
        newline_before_comment: false,
    };
    let meatpack_opts = MeatpackOptions { no_spaces: true };

    let Ok(gcode) = std::str::from_utf8(data) else {
        return;
    };
    let Ok(file) = file_parser(gcode) else { return };

    let mut parsed_packed = vec![];
    format_gcode_meatpack(
        file.iter_emit_tokens(),
        opts.clone(),
        meatpack_opts.clone(),
        &mut parsed_packed,
    )
    .unwrap();

    let (_, unpacked_gcode) = meatpacked_to_string(parsed_packed.as_slice()).unwrap();
    let reparsed_file = file_parser(&unpacked_gcode).unwrap();

    let mut reparsed_packed = vec![];
    format_gcode_meatpack(
        reparsed_file.iter_emit_tokens(),
        opts.clone(),
        meatpack_opts.clone(),
        &mut reparsed_packed,
    )
    .unwrap();

    assert_eq!(
        parsed_packed,
        reparsed_packed,
        "{gcode:?} vs {unpacked_gcode:?} vs {:?}",
        { meatpacked_to_string(reparsed_packed.as_slice()).unwrap().1 }
    );
});
