//! Fuzzing target for g-code parser
//!
//! ```bash
//! mkdir -p corpus/fuzz_parser && cp ../tests/* corpus/fuzz_parser
//! cargo fuzz run fuzz_parser -O -- -only_ascii=1
//! ```

#![no_main]

use g_code::parse::{file_parser, snippet_parser};
use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
    let Ok(gcode) = std::str::from_utf8(data) else {
        return;
    };
    let _ = file_parser(gcode);
    let _ = snippet_parser(gcode);
});
