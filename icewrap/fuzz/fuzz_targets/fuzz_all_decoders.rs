#![no_main]

use icewrap::decode::{all_decoders, PullDecoder};
use libfuzzer_sys::fuzz_target;
use std::io::Read;

fuzz_target!(|data: &[u8]| {
    for decoder in all_decoders() {
        let mut pull = PullDecoder::new(decoder, data);

        let mut buf = vec![];
        pull.read_to_end(&mut buf).unwrap();
    }
});
