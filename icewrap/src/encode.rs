use core::cmp::Ordering;

use crate::consts::{
    BACKREF_MARKER, LITERAL_MARKER, MAX_WINDOW_BITS, MIN_LOOKAHEAD_BITS, MIN_WINDOW_BITS, U8_BITS,
};

/// Convenience trait for working with an [Encoder] using generics or dynamic dispatch.
pub trait EncoderTrait {
    /// Convenience function for identifying a dyn [Encoder].
    fn window_bits(&self) -> u8;
    /// Convenience function for identifying a dyn [Encoder].
    fn lookahead_bits(&self) -> u8;
    /// Convenience function for identifying a dyn [Encoder].
    fn is_indexed(&self) -> bool;

    /// Offer data to encode. Returns the number of bytes consumed if any.
    fn sink(&mut self, data: &[u8]) -> Result<usize, SinkError>;
    /// Request a buffer in which to write data to encode. Must be paired with a subsequent call to [Encoder::ack_sink_to_offered].
    fn offer_sink_dest(&mut self) -> Result<&mut [u8], SinkError>;
    /// Acknowledges that `amount` bytes were written into the buffer offered via [Encoder::offer_sink_dest],
    fn ack_sink_to_offered(&mut self, amount: u16);

    /// Advance the internal state of the encoder, potentially yielding encoded data.
    fn poll(&mut self) -> PollResult;
    /// Signal that there is no more data to be encoded. See [`NotFinishedYet`] for how to handle an error here.
    fn finish(&mut self) -> Result<(), NotFinishedYet>;

    /// Reset the encoder to its initial state so it is ready to encode new data from scratch.
    fn reset(&mut self);
}

impl<E: EncoderTrait + ?Sized> EncoderTrait for &mut E {
    fn window_bits(&self) -> u8 {
        (**self).window_bits()
    }

    fn lookahead_bits(&self) -> u8 {
        (**self).lookahead_bits()
    }

    fn is_indexed(&self) -> bool {
        (**self).is_indexed()
    }

    fn sink(&mut self, data: &[u8]) -> Result<usize, SinkError> {
        (**self).sink(data)
    }

    fn offer_sink_dest(&mut self) -> Result<&mut [u8], SinkError> {
        (**self).offer_sink_dest()
    }

    fn ack_sink_to_offered(&mut self, amount: u16) {
        (**self).ack_sink_to_offered(amount)
    }

    fn poll(&mut self) -> PollResult {
        (**self).poll()
    }

    fn finish(&mut self) -> Result<(), NotFinishedYet> {
        (**self).finish()
    }

    fn reset(&mut self) {
        (**self).reset()
    }
}

#[derive(Clone)]
pub struct Encoder<
    const WINDOW: u8,
    const WINDOW_BUF_SIZE: usize,
    const LOOKAHEAD: u8,
    const USE_INDEX: bool,
> {
    buffer: [u8; WINDOW_BUF_SIZE],
    index: Option<[u16; WINDOW_BUF_SIZE]>,
    state: State,
    input_size: u16,
    match_scan_index: u16,
    match_len: u16,
    match_pos: u16,
    outgoing_bits: u16,
    outgoing_bits_count: u8,
    finishing: bool,
    current_byte: u8,
    num_bits_available: u8,
}

const INVALID_LINK: u16 = u16::MAX;

/// Internal state of an [Encoder]
#[repr(u8)]
#[derive(Debug, PartialEq, Eq, Clone, Copy)]
pub enum State {
    /// Input buffer is not full yet, need more data before returning [PollResult::Ready].
    ///
    /// May also become ready after the encoder is finished.
    NotFull,
    /// Input buffer is full and needs to be processed.
    Filled,
    /// Looks for a sequence of bytes that align with the backlog
    Search,
    /// Emits a tag bit 0 (backref) if there is a match, or 1 (literal) if there isn't.
    YieldTagBit,
    /// Emits a literal byte for data that didn't have a match.
    ///
    /// This state is guaranteed to return [PollResult::Ready].
    YieldLiteral,
    /// Emits the start index of the backreference
    ///
    /// Number of bits emitted depends on the window. If the window is 8 bits or higher, this state will always return [PollResult::Ready].
    YieldBackrefIndex,
    /// Emits the length of the backreference
    ///
    /// Number of bits depends on the lookahead. If the lookahead is 8 bits or higher, this state will always return [PollResult::Ready].
    YieldBackrefLen,
    /// Shifts data in the buffer towards the start when data that can be matched during a search has been exhausted
    SaveBacklog,
    /// Emits bits early when finishing
    FlushBits,
    /// [Encoder::finish] was called and there is no more data left
    Finished,
}

impl core::fmt::Display for State {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            State::NotFull => f.write_str("not full"),
            State::Filled => f.write_str("filled"),
            State::Search => f.write_str("searching"),
            State::YieldTagBit => f.write_str("yielding tag bit"),
            State::YieldLiteral => f.write_str("yielding literal"),
            State::YieldBackrefIndex => f.write_str("yielding backref index"),
            State::YieldBackrefLen => f.write_str("yielding backref length"),
            State::SaveBacklog => f.write_str("saving backlog"),
            State::FlushBits => f.write_str("flushing bits"),
            State::Finished => f.write_str("finished"),
        }
    }
}

/// Error type for [Encoder::sink]
#[derive(Debug)]
pub enum SinkError {
    /// Cannot accept more data, must be [polled](Encoder::poll) until [PollResult::Empty] and [reset](Encoder::reset).
    Finishing,
    /// Cannot accept more data until it is [reset](Encoder::reset).
    Finished,
    /// Encoder cannot accept more data until it is polled.
    MustPoll(State),
}

impl core::fmt::Display for SinkError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            SinkError::Finishing => f.write_str("encoder is finishing"),
            SinkError::Finished => f.write_str("encoder is finished"),
            SinkError::MustPoll(state) => {
                write!(f, "encoder must be polled, it is `{state}`")
            }
        }
    }
}

/// Result of calling [Encoder::poll]
#[derive(Debug)]
pub enum PollResult {
    /// Encoded byte of data is ready.
    Ready(u8),
    /// Encoder has additional work to do.
    Pending,
    /// Encoder needs more data.
    ///
    /// [Polling](Encoder::poll) again will do nothing until data is [sinked](Encoder::sink).
    /// It is also possible that the encoder is now [finished](Encoder::finish) and will not accept more data.
    Empty,
}

/// After calling [finish](Encoder::finish), there is some pending data that must be [polled](Encoder::poll) out of the encoder before it is truly finished.
#[derive(Debug)]
pub struct NotFinishedYet;

impl core::fmt::Display for NotFinishedYet {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.write_str("encoder must be polled before it is finished")
    }
}

impl<
        const WINDOW: u8,
        const WINDOW_BUF_SIZE: usize,
        const LOOKAHEAD: u8,
        const USE_INDEX: bool,
    > EncoderTrait for Encoder<WINDOW, WINDOW_BUF_SIZE, LOOKAHEAD, USE_INDEX>
{
    fn window_bits(&self) -> u8 {
        WINDOW
    }

    fn lookahead_bits(&self) -> u8 {
        LOOKAHEAD
    }

    fn is_indexed(&self) -> bool {
        USE_INDEX
    }

    fn sink(&mut self, data: &[u8]) -> Result<usize, SinkError> {
        if self.is_finishing() {
            return Err(SinkError::Finishing);
        }
        match self.state {
            State::NotFull => {}
            State::Finished => return Err(SinkError::Finished),
            other => return Err(SinkError::MustPoll(other)),
        }

        let write_offset = Self::INPUT_BUF_SIZE + self.input_size;
        let remaining = Self::INPUT_BUF_SIZE - self.input_size;
        let bytes_to_copy: u16 = (Into::<usize>::into(remaining))
            .min(data.len())
            .try_into()
            .unwrap();

        self.buffer[write_offset.into()..].copy_from_slice(&data[..bytes_to_copy.into()]);
        self.ack_sink_to_offered(bytes_to_copy);

        Ok(bytes_to_copy.into())
    }

    fn offer_sink_dest(&mut self) -> Result<&mut [u8], SinkError> {
        if self.is_finishing() {
            return Err(SinkError::Finishing);
        }
        match self.state {
            State::NotFull => {}
            State::Finished => return Err(SinkError::Finished),
            other => return Err(SinkError::MustPoll(other)),
        }

        let write_offset = Self::INPUT_BUF_SIZE + self.input_size;
        Ok(&mut self.buffer[write_offset.into()..])
    }

    fn ack_sink_to_offered(&mut self, amount: u16) {
        self.input_size += amount;
        if self.input_size == Self::INPUT_BUF_SIZE {
            self.state = State::Filled;
        }
    }

    fn poll(&mut self) -> PollResult {
        match self.state {
            State::NotFull => PollResult::Empty,
            State::Filled => {
                if USE_INDEX {
                    let index = self.index.as_mut().unwrap();
                    let mut last = [INVALID_LINK; 256];

                    // This can overflow for a 15-bit window, so it is inclusive
                    let end_inclusive = (Self::INPUT_BUF_SIZE - 1) + self.input_size;

                    for i in 0..=end_inclusive {
                        let v = self.buffer[Into::<usize>::into(i)];
                        let lv = last[Into::<usize>::into(v)];
                        index[Into::<usize>::into(i)] = lv;
                        last[Into::<usize>::into(v)] = i;
                    }
                }
                self.state = State::Search;
                PollResult::Pending
            }
            State::Search => {
                let current_lookahead = if self.is_finishing() {
                    1
                } else {
                    Self::LOOKAHEAD_SIZE
                };
                if self.match_scan_index > self.input_size - current_lookahead {
                    self.state = if self.is_finishing() {
                        State::FlushBits
                    } else {
                        State::SaveBacklog
                    };
                } else {
                    let start = self.match_scan_index;
                    let end = self.match_scan_index + Self::INPUT_BUF_SIZE;
                    let maximum_len =
                        Self::LOOKAHEAD_SIZE.min(self.input_size - self.match_scan_index);

                    let match_res = self.find_longest_match(start, end, maximum_len);
                    match match_res {
                        Some((pos, len)) => {
                            self.match_pos = pos;
                            self.match_len = len;
                            // This can overflow for a 15-bit window
                            debug_assert!(pos <= (Self::INPUT_BUF_SIZE - 1) + self.input_size);
                        }
                        None => {
                            self.match_scan_index += 1;
                            self.match_len = 0;
                        }
                    }
                    self.state = State::YieldTagBit;
                }

                PollResult::Pending
            }
            State::YieldTagBit => {
                if self.match_len == 0 {
                    let ret = self.push_bits(LITERAL_MARKER, 1);
                    self.state = State::YieldLiteral;
                    ret
                } else {
                    let ret = self.push_bits(BACKREF_MARKER, 1);
                    self.outgoing_bits = self.match_pos - 1;
                    self.outgoing_bits_count = WINDOW;
                    self.state = State::YieldBackrefIndex;
                    ret
                }
            }
            State::YieldLiteral => {
                let processed_offset = self.match_scan_index - 1;
                let input_offset = Self::INPUT_BUF_SIZE + processed_offset;
                let c = self.buffer[Into::<usize>::into(input_offset)];
                let res = self.push_bits(c, U8_BITS);
                self.state = State::Search;
                res
            }
            State::YieldBackrefIndex => {
                let (nonzero_count, res) = self.push_outgoing();
                self.state = if nonzero_count {
                    State::YieldBackrefIndex
                } else {
                    self.outgoing_bits = self.match_len - 1;
                    self.outgoing_bits_count = LOOKAHEAD;
                    State::YieldBackrefLen
                };

                res
            }
            State::YieldBackrefLen => {
                let (nonzero_count, res) = self.push_outgoing();
                self.state = if nonzero_count {
                    State::YieldBackrefLen
                } else {
                    self.match_scan_index += self.match_len;
                    self.match_len = 0;
                    State::Search
                };
                res
            }
            State::SaveBacklog => {
                let match_scan_index = self.match_scan_index;
                self.buffer
                    .copy_within(Into::<usize>::into(match_scan_index).., 0);
                self.match_scan_index = 0;
                self.input_size -= match_scan_index;
                self.state = State::NotFull;
                PollResult::Empty
            }
            State::FlushBits => {
                if self.num_bits_available < U8_BITS {
                    let res = PollResult::Ready(self.current_byte);
                    // Not strictly necessary
                    #[cfg(debug_assertions)]
                    {
                        self.num_bits_available = U8_BITS;
                        self.current_byte = 0;
                    }
                    self.state = State::Finished;
                    res
                } else {
                    self.state = State::Finished;
                    PollResult::Empty
                }
            }
            State::Finished => PollResult::Empty,
        }
    }

    fn finish(&mut self) -> Result<(), NotFinishedYet> {
        self.finishing = true;
        match self.state {
            State::Finished => Ok(()),
            State::NotFull => {
                self.state = if self.input_size == 0 {
                    if self.num_bits_available == U8_BITS {
                        State::Finished
                    } else {
                        State::FlushBits
                    }
                } else {
                    State::Filled
                };
                Err(NotFinishedYet)
            }
            _other => Err(NotFinishedYet),
        }
    }

    fn reset(&mut self) {
        *self = Self::new();
    }
}

impl<
        const WINDOW: u8,
        const WINDOW_BUF_SIZE: usize,
        const LOOKAHEAD: u8,
        const USE_INDEX: bool,
    > Encoder<WINDOW, WINDOW_BUF_SIZE, LOOKAHEAD, USE_INDEX>
{
    const VERIFY: () = {
        assert!(WINDOW >= MIN_WINDOW_BITS, "window too small");
        assert!(WINDOW <= MAX_WINDOW_BITS, "window too large");
        assert!(LOOKAHEAD >= MIN_LOOKAHEAD_BITS, "lookahead too small");
        assert!(LOOKAHEAD < WINDOW, "lookahead must be smaller than window");
        assert!(
            WINDOW_BUF_SIZE == 2 << WINDOW,
            "window buffer size must be 2 << window"
        );
    };
    const INPUT_BUF_SIZE: u16 = (WINDOW_BUF_SIZE / 2) as u16;
    const LOOKAHEAD_SIZE: u16 = (1 << LOOKAHEAD) as u16;
    const BREAK_EVEN_POINT: u8 = 1 + WINDOW + LOOKAHEAD;

    pub const fn new() -> Self {
        // Suppress unused variable warnings
        #[allow(clippy::let_unit_value)]
        let _ = Self::VERIFY;

        Self {
            buffer: [0; WINDOW_BUF_SIZE],
            index: if USE_INDEX {
                Some([INVALID_LINK; WINDOW_BUF_SIZE])
            } else {
                None
            },
            state: State::NotFull,
            input_size: 0,
            match_scan_index: 0,
            match_len: 0,
            match_pos: 0,
            outgoing_bits: 0,
            outgoing_bits_count: 0,
            finishing: false,
            current_byte: 0,
            num_bits_available: U8_BITS,
        }
    }

    #[inline]
    const fn is_finishing(&self) -> bool {
        self.finishing
    }

    #[inline]
    fn push_bits(&mut self, bits: u8, count: u8) -> PollResult {
        debug_assert!(count <= U8_BITS);
        // Will emit
        match count.cmp(&self.num_bits_available) {
            Ordering::Less => {
                self.num_bits_available -= count;
                self.current_byte |= bits << self.num_bits_available;
                PollResult::Pending
            }
            Ordering::Equal => {
                let shift = count - self.num_bits_available;
                let tmp_byte = self.current_byte | (bits >> shift);

                self.num_bits_available = U8_BITS;
                self.current_byte = 0;
                PollResult::Ready(tmp_byte)
            }
            Ordering::Greater => {
                let shift = count - self.num_bits_available;
                let tmp_byte = self.current_byte | (bits >> shift);

                let remaining = U8_BITS - shift;
                self.num_bits_available = remaining;
                self.current_byte = bits << remaining;

                PollResult::Ready(tmp_byte)
            }
        }
    }

    #[inline]
    fn push_outgoing(&mut self) -> (bool, PollResult) {
        let count;
        let bits;
        if self.outgoing_bits_count > U8_BITS {
            count = U8_BITS;
            bits = (self.outgoing_bits >> (self.outgoing_bits_count - U8_BITS))
                .try_into()
                .unwrap();
        } else {
            count = self.outgoing_bits_count;
            bits = (self.outgoing_bits & 0xFF).try_into().unwrap();
        }

        if count > 0 {
            let res = self.push_bits(bits, count);
            self.outgoing_bits_count -= count;
            (true, res)
        } else {
            (false, PollResult::Pending)
        }
    }

    #[inline]
    fn find_longest_match(&self, start: u16, end: u16, maximum_len: u16) -> Option<(u16, u16)> {
        let mut best_match_len = 0u16;
        let mut best_match_index: Option<u16> = None;
        let needle = &self.buffer[end.into()..];

        if USE_INDEX {
            let index = self.index.as_ref().unwrap();
            let mut pos = index[Into::<usize>::into(end)];
            while pos != INVALID_LINK && pos >= start {
                let slice_at_pos = &self.buffer[pos.into()..];

                // Optimization: this can only yield a new best if the bytes at `best_match_len` are equal.
                if slice_at_pos[Into::<usize>::into(best_match_len)]
                    == needle[Into::<usize>::into(best_match_len)]
                {
                    let len: u16 = slice_at_pos
                        .iter()
                        .copied()
                        .zip(needle.iter().copied())
                        .take(maximum_len.into())
                        .take_while(|(x, y)| *x == *y)
                        .count()
                        .try_into()
                        .unwrap();

                    if len > best_match_len {
                        best_match_len = len;
                        best_match_index = Some(pos);
                        if len == maximum_len {
                            break;
                        }
                    }
                }
                pos = index[Into::<usize>::into(pos)];
            }
        } else {
            // This "hashing" is really just summing to avoid running the inner counting logic if possible
            //
            // It should really be some kind of cyclic polynomial hash that can handle
            // adds to the end, removes from the beginning, and shifts to the right
            let mut needle_hash: u16 = 0;
            let mut search_hash: u16 = 0;

            for pos in (start..end).rev() {
                let slice_at_pos = &self.buffer[pos.into()..];
                #[cfg(not(fuzzing))]
                debug_assert_eq!(
                    needle_hash,
                    needle[..best_match_len.into()]
                        .iter()
                        .copied()
                        .fold(0u16, |acc, x| acc.wrapping_add(x.into()))
                );
                #[cfg(not(fuzzing))]
                debug_assert_eq!(
                    search_hash,
                    slice_at_pos[..best_match_len.into()]
                        .iter()
                        .copied()
                        .fold(0u16, |acc, x| acc.wrapping_add(x.into())),
                );

                // Optimization: this can only yield a new best if
                // - Search hash equals the needle hash
                // - The bytes at `best_match_len` are equal
                if search_hash == needle_hash
                    && slice_at_pos[Into::<usize>::into(best_match_len)]
                        == needle[Into::<usize>::into(best_match_len)]
                {
                    let confirmed_len: u16 = slice_at_pos
                        .iter()
                        .copied()
                        .zip(needle.iter().copied())
                        .take(best_match_len.into())
                        .take_while(|(x, y)| *x == *y)
                        .count()
                        .try_into()
                        .unwrap();
                    // (implicitly know it is better than the prev best, based on above, but don't include it in the check here)
                    let as_good_as_prev_best = confirmed_len == best_match_len;
                    if as_good_as_prev_best {
                        let mut match_len: u16 = best_match_len;

                        for i in best_match_len..maximum_len {
                            let i: usize = i.into();
                            if slice_at_pos[i] == needle[i] {
                                // add data to end of hash
                                search_hash =
                                    search_hash.wrapping_add(Into::<u16>::into(slice_at_pos[i]));
                                needle_hash =
                                    needle_hash.wrapping_add(Into::<u16>::into(needle[i]));
                                match_len += 1;
                            } else {
                                break;
                            }
                        }

                        best_match_index = Some(pos);
                        best_match_len = match_len;
                        if match_len == maximum_len {
                            break;
                        }
                    }
                }

                if best_match_len > 0 && pos > start {
                    // shift search hash to the right
                    search_hash = search_hash.wrapping_sub(Into::<u16>::into(
                        slice_at_pos[Into::<usize>::into(best_match_len - 1)],
                    ));
                    // add at the beginning
                    search_hash = search_hash
                        .wrapping_add(Into::<u16>::into(self.buffer[Into::<usize>::into(pos - 1)]));
                }
            }
        }

        let best_match_index = best_match_index?;
        if best_match_len > (Self::BREAK_EVEN_POINT / U8_BITS).into() {
            Some((end - best_match_index, best_match_len))
        } else {
            None
        }
    }
}

impl<
        const WINDOW: u8,
        const WINDOW_BUF_SIZE: usize,
        const LOOKAHEAD: u8,
        const USE_INDEX: bool,
    > Default for Encoder<WINDOW, WINDOW_BUF_SIZE, LOOKAHEAD, USE_INDEX>
{
    fn default() -> Self {
        Self::new()
    }
}

macro_rules! create_encoders {
    (
        $(
            $window: expr, $lookahead: expr;
        )*
    ) => {

        $(
            paste::paste! {
                #[doc = stringify!(Encoder with window $window, lookahead $lookahead, indexed) ]
                pub const fn [<encoder _ $window _ $lookahead _indexed>]() ->
                    Encoder<
                    $window,
                    {2 << $window },
                    $lookahead,
                    true,
                > {
                    Encoder::new()
                }

                #[doc = stringify!(Encoder with window $window, lookahead $lookahead) ]
                pub const fn [<encoder _ $window _ $lookahead>]() ->
                    Encoder<
                    $window,
                    {2 << $window },
                    $lookahead,
                    false,
                > {
                    Encoder::new()
                }
            }
        )*

        paste::paste! {
            /// Get the builder for an encoder at runtime
            ///
            /// This will return [None] if the parameters do not adhere to these constraints:
            ///
            /// - window size in the range `[MIN_WINDOW_BITS]..=[MAX_WINDOW_BITS]`
            /// - lookahead in the range `[MIN_LOOKAHEAD_BITS]..=(window - 1)`
            /// - indexed = true/false
            #[cfg(feature = "std")]
            pub fn dyn_encoder_builder(window: u8, lookahead: u8, indexed: bool) -> Option<Box<dyn Fn() -> Box<dyn EncoderTrait>>> {
                match (window, lookahead, indexed) {
                    $(
                        ($window, $lookahead, true) => Some(Box::new(|| Box::new([<encoder _ $window _ $lookahead>]()))),
                        ($window, $lookahead, false) => Some(Box::new(|| Box::new([<encoder _ $window _ $lookahead _indexed>]()))),
                    )*
                    _other => None,
                }
            }

            #[cfg(all(any(fuzzing, test), feature = "std"))]
            pub fn all_encoders() -> impl Iterator<Item = Box<dyn EncoderTrait>> {
                all_indexed_encoders().chain(all_non_indexed_encoders())
            }

            #[cfg(all(any(fuzzing, test), feature = "std"))]
            pub fn all_indexed_encoders() -> impl Iterator<Item = Box<dyn EncoderTrait>> {
                (MIN_WINDOW_BITS..=MAX_WINDOW_BITS).flat_map(|w| {
                    (MIN_LOOKAHEAD_BITS..w-1).map(move |l| {
                        dyn_encoder_builder(w, l, true).unwrap()()
                    })
                })
            }

            #[cfg(all(any(fuzzing, test), feature = "std"))]
            pub fn all_non_indexed_encoders() -> impl Iterator<Item = Box<dyn EncoderTrait>> {
                (MIN_WINDOW_BITS..=MAX_WINDOW_BITS).flat_map(|w| {
                    (MIN_LOOKAHEAD_BITS..w-1).map(move |l| {
                        dyn_encoder_builder(w, l, false).unwrap()()
                    })
                })
            }
        }
    };
}

create_encoders! {
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
    use super::*;

    impl std::error::Error for SinkError {}
    impl std::error::Error for NotFinishedYet {}

    impl<E: EncoderTrait + ?Sized> EncoderTrait for Box<E> {
        fn window_bits(&self) -> u8 {
            (**self).window_bits()
        }

        fn lookahead_bits(&self) -> u8 {
            (**self).lookahead_bits()
        }

        fn is_indexed(&self) -> bool {
            (**self).is_indexed()
        }

        fn sink(&mut self, data: &[u8]) -> Result<usize, SinkError> {
            (**self).sink(data)
        }

        fn offer_sink_dest(&mut self) -> Result<&mut [u8], SinkError> {
            (**self).offer_sink_dest()
        }

        fn ack_sink_to_offered(&mut self, amount: u16) {
            (**self).ack_sink_to_offered(amount)
        }

        fn poll(&mut self) -> PollResult {
            (**self).poll()
        }

        fn finish(&mut self) -> Result<(), NotFinishedYet> {
            (**self).finish()
        }

        fn reset(&mut self) {
            (**self).reset()
        }
    }

    /// [Read](std::io::Read) wrapper for an encoder
    ///
    /// Please read the docs for [Self::finish] before using.
    pub struct PullEncoder<E, R> {
        encoder: E,
        reader: R,
    }

    impl<E: EncoderTrait, R: std::io::Read> PullEncoder<E, R> {
        pub fn new(encoder: E, reader: R) -> Self {
            Self { encoder, reader }
        }

        /// This _MUST_ be called when you want to stop encoding and the underlying reader has no remaining data.
        ///
        /// It is necessary because there is no explicit end of data in a [std::io::Read] and an [Encoder] holds data in an internal buffer until it is explicitly finished.
        pub fn finish(mut self) -> FinishingPullEncoder<E> {
            FinishingPullEncoder {
                finish_res: self.encoder.finish(),
                encoder: self.encoder,
            }
        }
    }

    impl<E: EncoderTrait, R: std::io::Read> std::io::Read for PullEncoder<E, R> {
        fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
            let mut read = 0;
            let mut it = buf.iter_mut().peekable();

            loop {
                if it.peek().is_none() {
                    break Ok(read);
                }

                match self.encoder.poll() {
                    PollResult::Ready(b) => {
                        *it.next().unwrap() = b;
                        read += 1;
                    }
                    PollResult::Pending => {}
                    PollResult::Empty => match self.encoder.offer_sink_dest() {
                        Ok(dest) => {
                            let bytes_copied = self.reader.read(dest)?;
                            let bytes_copied_u16: u16 = bytes_copied.try_into().unwrap();
                            self.encoder.ack_sink_to_offered(bytes_copied_u16);

                            if bytes_copied == 0 {
                                break Ok(read);
                            }
                        }
                        Err(err) => unreachable!("{err:?}"),
                    },
                }
            }
        }
    }

    /// Represents a [PullEncoder] that is [finishing][Encoder::finish].
    ///
    /// [Reset](Encoder::reset) the encoder to use it again after done reading from this .
    pub struct FinishingPullEncoder<E> {
        finish_res: Result<(), NotFinishedYet>,
        encoder: E,
    }

    impl<E: EncoderTrait> std::io::Read for FinishingPullEncoder<E> {
        fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
            if self.is_finished() {
                return Ok(0);
            }

            let mut read = 0;
            let mut it = buf.iter_mut().peekable();

            loop {
                if it.peek().is_none() {
                    break Ok(read);
                }

                match self.encoder.poll() {
                    PollResult::Ready(b) => {
                        *it.next().unwrap() = b;
                        read += 1;
                    }
                    PollResult::Pending => {}
                    PollResult::Empty => {
                        self.finish_res = self.encoder.finish();
                        break Ok(read);
                    }
                }
            }
        }
    }

    impl<E: EncoderTrait> FinishingPullEncoder<E> {
        /// Convenience function for determining if the encoder has any more data to emit
        pub fn is_finished(&self) -> bool {
            match self.finish_res {
                Ok(()) => true,
                Err(NotFinishedYet) => false,
            }
        }
    }

    /// [Write](std::io::Write) wrapper for an encoder
    ///
    /// Please read the docs for [Self::finish] before using.
    pub struct PushEncoder<E, W> {
        pub encoder: E,
        pub output: W,
    }
    impl<E: EncoderTrait, W: std::io::Write> PushEncoder<E, W> {
        pub fn new(encoder: E, output: W) -> Self {
            Self { encoder, output }
        }

        /// This _MUST_ be called when you want to stop encoding and all data has been written.
        ///
        /// It is necessary because there is no explicit end of data when writing and an [Encoder] holds data in an internal buffer until it is explicitly finished.
        ///
        /// [Reset](Encoder::reset) the encoder to use it again after this call.
        pub fn finish(mut self) -> std::io::Result<()> {
            match self.encoder.finish() {
                Ok(()) => {}
                Err(NotFinishedYet) => loop {
                    match self.encoder.poll() {
                        PollResult::Ready(b) => {
                            self.output.write_all(&[b])?;
                        }
                        PollResult::Pending => {}
                        PollResult::Empty => break,
                    }
                },
            }
            Ok(())
        }
    }

    impl<E: EncoderTrait, W: std::io::Write> std::io::Write for PushEncoder<E, W> {
        fn write(&mut self, mut buf: &[u8]) -> std::io::Result<usize> {
            let mut written = 0;
            loop {
                if buf.is_empty() {
                    break Ok(written);
                }

                match self.encoder.sink(buf) {
                    Ok(bytes_copied) => {
                        buf = &buf[bytes_copied..];
                        written += bytes_copied;
                    }
                    Err(SinkError::MustPoll(_)) => loop {
                        match self.encoder.poll() {
                            PollResult::Ready(b) => {
                                self.output.write_all(&[b])?;
                            }
                            PollResult::Pending => {}
                            PollResult::Empty => break,
                        }
                    },
                    Err(SinkError::Finishing | SinkError::Finished) => unreachable!(),
                }
            }
        }

        fn flush(&mut self) -> std::io::Result<()> {
            loop {
                match self.encoder.poll() {
                    PollResult::Ready(b) => {
                        self.output.write_all(&[b])?;
                    }
                    PollResult::Pending => {}
                    PollResult::Empty => break,
                }
            }
            self.output.flush()
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
    fn test_encoder_encodes_as_expected() {
        let cases: Vec<(Box<dyn EncoderTrait>, _, _)> = vec![
            (
                Box::new(encoder_8_7()),
                [0, 1, 2, 3, 4].as_slice(),
                [0x80, 0x40, 0x60, 0x50, 0x38, 0x20].as_slice(),
            ),
            (
                Box::new(encoder_8_7()),
                b"aaaaa".as_slice(),
                [0xb0, 0x80, 0x01, 0x80].as_slice(),
            ),
            (
                Box::new(encoder_8_7()),
                [0, 0, 4].as_slice(),
                [0x80, 0x40, 0x20, 0x80].as_slice(),
            ),
        ];

        for (encoder, data, expected) in cases {
            let mut pull = PullEncoder::new(encoder, data);

            let mut buf = vec![];
            pull.read_to_end(&mut buf).unwrap();
            pull.finish().read_to_end(&mut buf).unwrap();

            assert_eq!(buf, expected);
        }
    }
}
