#![no_main]

use std::io::Read;

use icewrap::encode::{encoder_12_4, encoder_12_4_indexed, PullEncoder};
use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
    let encoder = encoder_12_4();

    let mut pull = PullEncoder::new(encoder, data);
    let mut buf = vec![];
    pull.read_to_end(&mut buf).unwrap();
    pull.finish().read_to_end(&mut buf).unwrap();

    let encoder_indexed = encoder_12_4_indexed();
    let mut pull_indexed = PullEncoder::new(encoder_indexed, data);
    let mut buf_indexed = vec![];
    pull_indexed.read_to_end(&mut buf_indexed).unwrap();
    pull_indexed.finish().read_to_end(&mut buf_indexed).unwrap();

    // Non-indexed and indexed encoders should be functionally equivalent
    assert_eq!(buf, buf_indexed);
});
