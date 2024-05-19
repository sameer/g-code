#![no_main]

use icewrap::encode::{all_encoders, PullEncoder};
use libfuzzer_sys::fuzz_target;
use std::io::Read;

fuzz_target!(|data: &[u8]| {
    for encoder in all_encoders() {
        let mut pull = PullEncoder::new(encoder, data);

        let mut buf = vec![];
        pull.read_to_end(&mut buf).unwrap();
        pull.finish().read_to_end(&mut buf).unwrap();
    }
});
