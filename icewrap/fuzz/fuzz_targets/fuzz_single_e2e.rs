#![no_main]

use std::io::Read;

use icewrap::{decode::*, encode::*};
use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
    for encoder in [
        Box::new(encoder_12_4_indexed()) as Box<dyn EncoderTrait>,
        Box::new(encoder_12_4()),
    ] {
        let mut pull_encoder = PullEncoder::new(encoder, data);
        let mut enc_buf = vec![];
        pull_encoder.read_to_end(&mut enc_buf).unwrap();
        pull_encoder.finish().read_to_end(&mut enc_buf).unwrap();

        let mut dec_buf = vec![];
        let mut pull_decoder = PullDecoder::new(decoder_12_4(), enc_buf.as_slice());
        pull_decoder.read_to_end(&mut dec_buf).unwrap();

        let equal = data
            .iter()
            .copied()
            .zip(dec_buf.iter().copied())
            .take_while(|(x, y)| *x == *y)
            .count();

        assert!(
            equal == data.len() && equal == dec_buf.len(),
            "Difference (len {} vs {}) {:?} vs {:?}",
            data.len(),
            dec_buf.len(),
            &data[equal..],
            &dec_buf[equal..]
        );
    }
});
