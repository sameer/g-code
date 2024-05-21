use core::cmp::Ordering;

use crate::consts::{
    LITERAL_MARKER, MAX_WINDOW_BITS, MIN_LOOKAHEAD_BITS, MIN_WINDOW_BITS, U8_BITS,
};

/// Internal state of a [Decoder]
#[repr(u8)]
#[derive(Debug, Clone)]
enum State {
    TagBit,
    YieldLiteral,
    BackrefIndexMsb,
    BackrefIndexLsb,
    BackrefCountMsb,
    BackrefCountLsb,
    YieldBackref,
}

#[derive(Debug, Clone)]
pub struct Decoder<const WINDOW: u8, const LOOKAHEAD: u8, const BUFFER_SIZE: usize> {
    output_count: u16,
    output_index: u16,
    head_index: u16,
    state: State,
    loaded_byte: Option<u8>,
    current_byte: u8,
    bit_index: u8,

    buffer: [u8; BUFFER_SIZE],
}

/// Error type for [Decoder::load] indicating that the internal buffer is full
#[derive(Debug, Clone, Copy)]
pub struct Full;

impl core::fmt::Display for Full {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.write_str("decoder must be polled before it can load more data")
    }
}

/// Result of calling [Decoder::poll]
pub enum PollResult {
    /// Decoded byte of data is ready.
    Ready(u8),
    /// Decoder has additional work to do.
    Pending,
    /// Decoder needs more data.
    ///
    /// [Polling](Decoder::poll) again will do nothing until data is [loaded](Decoder::load).
    NeedsLoad,
}

pub trait DecoderTrait {
    /// Convenience function for identifying a dyn [Decoder].
    fn window_bits(&self) -> u8;
    /// Convenience function for identifying a dyn [Decoder].
    fn lookahead_bits(&self) -> u8;

    /// Offer data to decode. Returns [Full] if the internal buffer is full.
    ///
    /// If the previous [poll](Decoder::poll) result was [PollResult::NeedsLoad], this will not fail.
    fn load(&mut self, byte: u8) -> Result<(), Full>;
    //// Advance internal state of the decoder, potentially yielding decoded data.
    fn poll(&mut self) -> PollResult;

    /// Indicates whether decoding can stop now, or if there is pending data.
    fn is_finished(&self) -> bool;

    //// Reset decoder to its initial state so it is ready to decode new data from scratch.
    fn reset(&mut self);
}

impl<D: DecoderTrait> DecoderTrait for &mut D {
    fn window_bits(&self) -> u8 {
        (**self).window_bits()
    }

    fn lookahead_bits(&self) -> u8 {
        (**self).lookahead_bits()
    }

    fn load(&mut self, byte: u8) -> Result<(), Full> {
        (**self).load(byte)
    }

    fn poll(&mut self) -> PollResult {
        (**self).poll()
    }

    fn reset(&mut self) {
        (**self).reset()
    }

    fn is_finished(&self) -> bool {
        (**self).is_finished()
    }
}

impl<const WINDOW: u8, const LOOKAHEAD: u8, const BUFFER_SIZE: usize>
    Decoder<WINDOW, LOOKAHEAD, BUFFER_SIZE>
{
    const WINDOW_SIZE: u16 = 1u16 << WINDOW;
    const LOOKAHEAD_SIZE: u16 = 1u16 << LOOKAHEAD;
    const BACKREF_INDEX_LSB_BITS_TO_PULL: u8 = if WINDOW > 8 { 8 } else { WINDOW };
    const BACKREF_COUNT_MSB_BITS_TO_PULL: u8 = if LOOKAHEAD > 8 { 8 } else { LOOKAHEAD };

    const VERIFY: () = {
        assert!(WINDOW >= MIN_WINDOW_BITS, "window too small");
        assert!(WINDOW <= MAX_WINDOW_BITS, "window too large");
        assert!(LOOKAHEAD >= MIN_LOOKAHEAD_BITS, "lookahead too small");
        assert!(WINDOW > LOOKAHEAD, "window must be larger than lookahead");
        assert!(
            Self::WINDOW_SIZE as usize == BUFFER_SIZE,
            "buffer size must be 2^(window size)"
        );
    };

    pub const fn new() -> Self {
        #[allow(clippy::let_unit_value)]
        let _ = Self::VERIFY;

        Self {
            state: State::TagBit,
            output_count: 0,
            output_index: 0,
            head_index: 0,
            loaded_byte: None,
            current_byte: 0,
            bit_index: 0,
            buffer: [0; BUFFER_SIZE],
        }
    }

    fn pull_bits(&mut self, count: u8) -> Option<u16> {
        debug_assert!(count <= U8_BITS);
        if self.loaded_byte.is_none() && self.bit_index < count {
            return None;
        }

        let mut acc: u16 = self.current_byte.into();
        acc &= (1 << self.bit_index) - 1;
        match count.cmp(&self.bit_index) {
            Ordering::Less => {
                acc >>= self.bit_index - count;
                self.bit_index -= count;
            }
            Ordering::Equal => match self.loaded_byte.take() {
                Some(byte) => {
                    self.current_byte = byte;
                    self.bit_index = U8_BITS;
                }
                None => {
                    self.bit_index = 0;
                }
            },
            Ordering::Greater => {
                acc <<= U8_BITS;
                self.current_byte = self.loaded_byte.take().unwrap();
                acc += Into::<u16>::into(self.current_byte);
                self.bit_index += U8_BITS - count;
                acc >>= self.bit_index;
            }
        }

        Some(acc)
    }
}

impl<const WINDOW: u8, const LOOKAHEAD: u8, const BUFFER_SIZE: usize> Default
    for Decoder<WINDOW, LOOKAHEAD, BUFFER_SIZE>
{
    fn default() -> Self {
        Self::new()
    }
}

impl<const WINDOW: u8, const LOOKAHEAD: u8, const BUFFER_SIZE: usize> DecoderTrait
    for Decoder<WINDOW, LOOKAHEAD, BUFFER_SIZE>
{
    fn window_bits(&self) -> u8 {
        WINDOW
    }

    fn lookahead_bits(&self) -> u8 {
        LOOKAHEAD
    }

    fn load(&mut self, byte: u8) -> Result<(), Full> {
        match self.loaded_byte {
            Some(_) => Err(Full),
            None => {
                self.loaded_byte = Some(byte);
                Ok(())
            }
        }
    }

    fn poll(&mut self) -> PollResult {
        match self.state {
            State::TagBit => match self.pull_bits(1) {
                Some(bits) => {
                    self.state =
                        if bits & Into::<u16>::into(LITERAL_MARKER) == LITERAL_MARKER.into() {
                            State::YieldLiteral
                        } else if WINDOW > U8_BITS {
                            State::BackrefIndexMsb
                        } else {
                            self.output_index = 0;
                            State::BackrefIndexLsb
                        };
                    PollResult::Pending
                }
                None => PollResult::NeedsLoad,
            },
            State::YieldLiteral => match self.pull_bits(8) {
                Some(bits) => {
                    let byte: u8 = (bits & 0xFF).try_into().unwrap();
                    // Ensures indexing does not go out of bounds on the buffer
                    let mask = Self::WINDOW_SIZE - 1;
                    self.buffer[Into::<usize>::into(self.head_index & mask)] = byte;
                    self.head_index = self.head_index.wrapping_add(1);
                    self.state = State::TagBit;
                    PollResult::Ready(byte)
                }
                None => PollResult::NeedsLoad,
            },
            State::BackrefIndexMsb => {
                debug_assert!(WINDOW > U8_BITS);
                match self.pull_bits(WINDOW - U8_BITS) {
                    Some(bits) => {
                        self.output_index = bits << 8;
                        self.state = State::BackrefIndexLsb;
                        PollResult::Pending
                    }
                    None => PollResult::NeedsLoad,
                }
            }
            State::BackrefIndexLsb => match self.pull_bits(Self::BACKREF_INDEX_LSB_BITS_TO_PULL) {
                Some(bits) => {
                    self.output_index |= bits;
                    self.output_index += 1;
                    self.output_count = 0;

                    self.state = if LOOKAHEAD > U8_BITS {
                        State::BackrefCountMsb
                    } else {
                        State::BackrefCountLsb
                    };

                    PollResult::Pending
                }
                None => PollResult::NeedsLoad,
            },
            State::BackrefCountMsb => {
                debug_assert!(LOOKAHEAD > U8_BITS);
                match self.pull_bits(LOOKAHEAD - U8_BITS) {
                    Some(bits) => {
                        self.output_count = bits << U8_BITS;
                        self.state = State::BackrefCountLsb;
                        PollResult::Pending
                    }
                    None => PollResult::NeedsLoad,
                }
            }
            State::BackrefCountLsb => match self.pull_bits(Self::BACKREF_COUNT_MSB_BITS_TO_PULL) {
                Some(bits) => {
                    self.output_count |= bits;
                    self.output_count += 1;
                    self.state = State::YieldBackref;
                    PollResult::Pending
                }
                None => PollResult::NeedsLoad,
            },
            State::YieldBackref => {
                // Ensures indexing does not go out of bounds on the buffer
                let mask = Self::WINDOW_SIZE - 1;
                let neg_offset = self.output_index;
                debug_assert!(neg_offset <= mask + 1);
                debug_assert!(self.output_count <= Self::LOOKAHEAD_SIZE);

                let c = self.buffer
                    [Into::<usize>::into((self.head_index.wrapping_sub(neg_offset)) & mask)];
                self.buffer[Into::<usize>::into(self.head_index & mask)] = c;
                self.head_index = self.head_index.wrapping_add(1);
                self.output_count -= 1;

                if self.output_count == 0 {
                    self.state = State::TagBit;
                }
                PollResult::Ready(c)
            }
        }
    }

    fn reset(&mut self) {
        *self = Self::new();
    }

    fn is_finished(&self) -> bool {
        let more = match self.state {
            State::TagBit
            | State::YieldLiteral
            | State::BackrefIndexMsb
            | State::BackrefIndexLsb
            | State::BackrefCountMsb
            | State::BackrefCountLsb => self.loaded_byte.is_some(),
            State::YieldBackref => true,
        };

        !more
    }
}

macro_rules! create_decoders {
    (
        $(
            $window: expr, $lookahead: expr;
        )*
    ) => {

        $(
            paste::paste! {
                #[doc = stringify!(Decoder with window $window, lookahead $lookahead) ]
                pub const fn [<decoder _ $window _ $lookahead>]() ->
                    Decoder<
                    $window,
                    $lookahead,
                    {1 << $window },
                > {
                    Decoder::new()
                }
            }
        )*

        paste::paste! {
            /// Get the builder for a decoder at runtime
            ///
            /// This will return [None] if the parameters do not adhere to these constraints:
            ///
            /// - window size in the range [MIN_WINDOW_BITS]..=[MAX_WINDOW_BITS]
            /// - lookahead in the range [MIN_LOOKAHEAD_BITS]..=(window - 1)
            #[cfg(feature = "std")]
            pub fn dyn_decoder_builder(window: u8, lookahead: u8) -> Option<Box<dyn Fn() -> Box<dyn DecoderTrait>>> {
                match (window, lookahead) {
                    $(
                        ($window, $lookahead) => Some(Box::new(|| Box::new([<decoder _ $window _ $lookahead>]()))),
                    )*
                    _other => None,
                }
            }

            #[cfg(all(any(fuzzing, test), feature = "std"))]
            pub fn all_decoders() -> impl Iterator<Item = Box<dyn DecoderTrait>> {
                (MIN_WINDOW_BITS..=MAX_WINDOW_BITS).flat_map(|w| {
                    (MIN_LOOKAHEAD_BITS..w-1).map(move |l| {
                        dyn_decoder_builder(w, l).unwrap()()
                    })
                })
            }
        }
    };
}

create_decoders! {
    4, 3;
    5, 3;
    5, 4;
    6, 3;
    6, 4;
    6, 5;
    7, 3;
    7, 4;
    7, 5;
    7, 6;
    8, 3;
    8, 4;
    8, 5;
    8, 6;
    8, 7;
    9, 3;
    9, 4;
    9, 5;
    9, 6;
    9, 7;
    9, 8;
    10, 3;
    10, 4;
    10, 5;
    10, 6;
    10, 7;
    10, 8;
    10, 9;
    11, 3;
    11, 4;
    11, 5;
    11, 6;
    11, 7;
    11, 8;
    11, 9;
    11, 10;
    12, 3;
    12, 4;
    12, 5;
    12, 6;
    12, 7;
    12, 8;
    12, 9;
    12, 10;
    12, 11;
    13, 3;
    13, 4;
    13, 5;
    13, 6;
    13, 7;
    13, 8;
    13, 9;
    13, 10;
    13, 11;
    13, 12;
    14, 3;
    14, 4;
    14, 5;
    14, 6;
    14, 7;
    14, 8;
    14, 9;
    14, 10;
    14, 11;
    14, 12;
    14, 13;
    15, 3;
    15, 4;
    15, 5;
    15, 6;
    15, 7;
    15, 8;
    15, 9;
    15, 10;
    15, 11;
    15, 12;
    15, 13;
    15, 14;
}

#[cfg(feature = "std")]
mod std_support {
    use std::io::{Read, Write};

    use super::*;

    impl DecoderTrait for Box<dyn DecoderTrait> {
        fn window_bits(&self) -> u8 {
            (**self).window_bits()
        }

        fn lookahead_bits(&self) -> u8 {
            (**self).lookahead_bits()
        }

        fn load(&mut self, byte: u8) -> Result<(), Full> {
            (**self).load(byte)
        }

        fn poll(&mut self) -> PollResult {
            (**self).poll()
        }

        fn reset(&mut self) {
            (**self).reset()
        }

        fn is_finished(&self) -> bool {
            (**self).is_finished()
        }
    }

    impl std::error::Error for Full {}

    /// [Read](std::io::Read) wrapper for a decoder
    pub struct PullDecoder<D: DecoderTrait, R: Read> {
        decoder: D,
        reader: R,
    }

    impl<D: DecoderTrait, R: Read> PullDecoder<D, R> {
        pub const fn new(decoder: D, reader: R) -> Self {
            Self { decoder, reader }
        }
    }

    impl<D: DecoderTrait, R: Read> Read for PullDecoder<D, R> {
        fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
            let mut it = buf.iter_mut().peekable();
            let mut read = 0;

            loop {
                if it.peek().is_none() {
                    break Ok(read);
                }
                match self.decoder.poll() {
                    PollResult::Ready(byte) => {
                        *it.next().unwrap() = byte;
                        read += 1;
                    }
                    PollResult::Pending => {}
                    PollResult::NeedsLoad => {
                        let mut buf = [0];
                        let num_read = self.reader.read(&mut buf)?;
                        if num_read == 0 {
                            break Ok(read);
                        } else {
                            self.decoder.load(buf[0]).unwrap();
                        }
                    }
                }
            }
        }
    }

    pub struct PushDecoder<D: DecoderTrait, W: Write> {
        decoder: D,
        writer: W,
    }

    /// [Write](std::io::Write) wrapper for an encoder
    impl<D: DecoderTrait, W: Write> PushDecoder<D, W> {
        pub const fn new(decoder: D, writer: W) -> Self {
            Self { decoder, writer }
        }
    }

    impl<D: DecoderTrait, W: Write> Write for PushDecoder<D, W> {
        fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
            let mut written = 0;
            let mut it = buf.iter();
            loop {
                match self.decoder.poll() {
                    PollResult::Ready(byte) => {
                        if self.writer.write(&[byte])? == 0 {
                            break Err(std::io::Error::new(
                                std::io::ErrorKind::WriteZero,
                                "failed to write ready byte",
                            ));
                        }
                    }
                    PollResult::Pending => {}
                    PollResult::NeedsLoad => match it.next() {
                        Some(load) => {
                            self.decoder.load(*load).unwrap();
                            written += 1;
                        }
                        None => {
                            break Ok(written);
                        }
                    },
                }
            }
        }

        fn flush(&mut self) -> std::io::Result<()> {
            match self.decoder.poll() {
                PollResult::Ready(byte) => {
                    if self.writer.write(&[byte])? == 0 {
                        return Err(std::io::Error::new(
                            std::io::ErrorKind::WriteZero,
                            "failed to flush ready byte",
                        ));
                    }
                }
                PollResult::Pending | PollResult::NeedsLoad => {}
            }
            self.writer.flush()
        }
    }
}

#[cfg(feature = "std")]
pub use std_support::*;

#[cfg(all(test, feature = "std"))]
mod tests {
    use std::io::Read;

    use super::*;

    #[test]
    fn test_decoder_decodes_as_expected() {
        let cases: [(Box<dyn DecoderTrait>, _, _); 3] = [
            (
                Box::new(decoder_7_6()),
                [0xb3, 0x5b, 0xed, 0xe0].as_slice(),
                b"foo".as_slice(),
            ),
            (
                Box::new(decoder_7_6()),
                [0xb3, 0x5b, 0xed, 0xe0, 0x41, 0x00].as_slice(),
                b"foofoo".as_slice(),
            ),
            (
                Box::new(decoder_8_7()),
                [0xb0, 0x80, 0x01, 0x80].as_slice(),
                b"aaaaa".as_slice(),
            ),
        ];

        for (decoder, data, expected) in cases {
            let mut buf = vec![];
            let mut decoder = PullDecoder::new(decoder, data);

            decoder.read_to_end(&mut buf).unwrap();

            assert_eq!(buf, expected);
        }
    }
}
