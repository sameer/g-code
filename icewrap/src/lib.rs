//! A Rust port of the [Heatshrink](https://github.com/atomicobject/heatshrink/) compression library for embedded/real-time systems.

#![cfg_attr(not(feature = "std"), no_std)]

#[cfg(feature = "std")]
extern crate std;

mod consts {
    /// Minimum window size in bits
    pub const MIN_WINDOW_BITS: u8 = 4;

    /// Maximum window size in bits
    pub const MAX_WINDOW_BITS: u8 = 15;

    /// Minimum lookahead in bits
    ///
    /// The maximum lookahead is `window_bits - 1`
    pub const MIN_LOOKAHEAD_BITS: u8 = 3;

    /// Bit marker for a literal
    pub(crate) const LITERAL_MARKER: u8 = 1;

    /// Bit marker for a backref
    pub(crate) const BACKREF_MARKER: u8 = 0;

    pub(crate) const U8_BITS: u8 = u8::BITS as u8;
}

/// Internal constants exposed for documentation purposes
pub mod _consts {
    pub use crate::consts::{MAX_WINDOW_BITS, MIN_LOOKAHEAD_BITS, MIN_WINDOW_BITS};
}

/// Decode data encoded with Heatshrink
pub mod decode;

/// Encode data in Heatshrink
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
        let data = include_bytes!("../tests/input/crash-c58fa467dc18f85dce08996eabb8c55f66febd89")
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
