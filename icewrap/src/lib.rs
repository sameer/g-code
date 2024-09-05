//! A Rust port of the [Heatshrink](https://github.com/atomicobject/heatshrink/) compression library for embedded/real-time systems.
//!
//! # How it works
//!
//! Heatshrink is based on [LZSS](https://en.wikipedia.org/wiki/Lempel%E2%80%93Ziv%E2%80%93Storer%E2%80%93Szymanski).
//! It recognizes repeated byte sequences in data and encodes them as _backreferences_ to the original sequence.
//! A _backreference_ consists of an index and length. The index is a negative offset from the current position in the data that can go as far back as `2^window`.
//! The length can be any value in the inclusive range `[1, 2^lookahead]`.
//!
//! While a backreference of length 1 or 2 could technically be constructed, it would waste space (`sizeof(index) + sizeof(length) > sizeof(single byte)`).
//! Instead, Heatshrink encodes these smaller sequences of data as _literals_; the raw data is emitted.
//! To indicate whether data is a backreference or literal, the encoder includes a 1-bit identifer flag: `0` for a backreference, `1` for a literal.
//!
//! ## Encoder
//!
//! The encoder buffers up incoming data internally to try and generate backreferences in the available window of data.
//! The search for backreferences is the most computationally intensive operation.
//! There is an indexing optimization that speeds it up, but it uses more memory.
//!
//! ## Decoder
//!
//! The decoder immediately yields literals as they are received and maintains a circular buffer of the last `2^window` bytes for handling backreferences.
//! There aren't really any computationally intensive operations performed.
//!
//! ## Raw Format
//!
//! ```text
//! encoded_data = [entry]*
//! entry = [tag_bit][tag_data]
//! if tag_bit == [0] then tag_data = [index][len] (backreference)
//! if tag_bit == [1] then tag_data = [byte] (literal)
//! index = window-bits number
//! len = lookahead-bits number
//! byte = 8-bit number
//! ```
//!
//! # Credits
//!
//! Scott Vokes is the author of the original C library.
//! His [blog post](https://spin.atomicobject.com/heatshrink-embedded-data-compression/) goes into more detail on the origins and performance.

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

/// Internal constants exposed for documentation purposes.
pub mod _consts {
    pub use crate::consts::{MAX_WINDOW_BITS, MIN_LOOKAHEAD_BITS, MIN_WINDOW_BITS};
}

/// Decode data encoded with Heatshrink.
pub mod decode;

/// Encode data in Heatshrink.
pub mod encode;

#[cfg(all(feature = "std", test))]
mod tests {
    use super::decode::*;
    use super::encode::*;
    use std::io::Read;

    #[test]
    #[cfg_attr(debug_assertions, ignore = "Test takes too long for debug profile")]
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
