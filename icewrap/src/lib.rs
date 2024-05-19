#![cfg_attr(not(feature = "std"), no_std)]

#[cfg(feature = "std")]
extern crate std;

mod consts {
    pub(crate) const MIN_WINDOW_BITS: u8 = 4;
    pub(crate) const MAX_WINDOW_BITS: u8 = 15;
    pub(crate) const MIN_LOOKAHEAD_BITS: u8 = 3;
    pub(crate) const LITERAL_MARKER: u8 = 1;
    pub(crate) const BACKREF_MARKER: u8 = 0;

    pub(crate) const U8_BITS: u8 = u8::BITS as u8;
}

pub mod decode;
pub mod encode;

#[cfg(all(feature = "std", test))]
mod tests {
    use super::decode::*;
    use super::encode::*;
    use std::io::Read;

    #[test]
    #[cfg_attr(debug, ignore = "Test takes too long for debug profile")]
    fn test_all_end_to_end_with_big_file() {
        // let data = include_str!("./encode.rs");
        let data = include_bytes!(
            "../fuzz/artifacts/fuzz_single_e2e/crash-c58fa467dc18f85dce08996eabb8c55f66febd89"
        )
        .as_slice();

        let it = all_non_indexed_encoders()
            .zip(all_decoders())
            .chain(all_indexed_encoders().zip(all_decoders()));

        for (encoder, mut decoder) in it {
            let mut pull_encoder = PullEncoder::new(encoder, data);
            let mut enc_buf = vec![];
            pull_encoder.read_to_end(&mut enc_buf).unwrap();
            pull_encoder.finish().read_to_end(&mut enc_buf).unwrap();

            let mut dec_buf = vec![];
            let mut pull_decoder = PullDecoder::new(&mut decoder, enc_buf.as_slice());
            pull_decoder.read_to_end(&mut dec_buf).unwrap();
            pull_decoder.finish().read_to_end(&mut dec_buf).unwrap();

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
    }
}
