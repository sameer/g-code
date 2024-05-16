//! Parsing for meatpacked g-code
//!
//! <https://github.com/scottmudge/OctoPrint-MeatPack/>

use std::{cell::RefCell, ops::RangeFrom, rc::Rc};

use nom::{
    bytes::complete::tag,
    combinator::{cond, flat_map, iterator},
    number::complete::le_u8,
    Compare, IResult, InputIter, InputLength, InputTake, Parser, Slice,
};

/// Present when two characters will not be found by [unpack_character]
pub(crate) const MP_BOTH_UNPACKABLE_HEADER: [u8; 1] = [0xFF];

/// Bitmask that indicates that a particular character in a pair cannot be unpacked
pub(crate) const MP_SINGLE_UNPACKABLE_MASK: u8 = 0xF;

/// Byte sequence preceding a command
pub(crate) const MP_COMMAND_HEADER: [u8; 2] = [0xFF, 0xFF];

/// Enables packing for size reduction
pub(crate) const MP_COMMAND_ENABLE_PACKING: u8 = 251;

/// Disables packing
pub(crate) const MP_COMMAND_DISABLE_PACKING: u8 = 250;

/// Reset configuration to defaults
///
/// Usually sent at the end of packed g-code
pub(crate) const MP_COMMAND_RESET_ALL: u8 = 249;

/// Ignored
pub(crate) const MP_COMMAND_QUERY_CONFIG: u8 = 248;

/// Special mode where packed g-code contains no spaces
///
/// `E` replaces ` ` in the lookup table
pub(crate) const MP_COMMAND_ENABLE_NO_SPACES: u8 = 247;
pub(crate) const MP_COMMAND_DISABLE_NO_SPACES: u8 = 246;

/// Unpack meatpacked g-code to a string using [nom]
///
/// Once unpacked, call [crate::parse::file_parser] or [crate::parse::snippet_parser] as appropriate
pub fn meatpacked_to_string<I>(input: I) -> IResult<I, String, nom::error::Error<I>>
where
    I: Clone
        + Slice<RangeFrom<usize>>
        + InputIter<Item = u8>
        + InputTake
        + InputLength
        + Compare<&'static [u8]>,
{
    let state = Rc::new(RefCell::new(MeatpackState::default()));
    let mut parser = iterator(input, |input| decode_next(state.clone()).parse(input));
    let it = &mut parser;
    let acc = String::from_utf8(it.flatten().collect::<Vec<u8>>()).unwrap();
    parser.finish().map(|(input, ())| (input, acc))
}

/// Used to make the [nom] parser stateful
#[derive(Debug, Default)]
struct MeatpackState {
    packing: bool,
    no_spaces: bool,
}

/// Decode the next command or character pair
fn decode_next<I>(
    state: Rc<RefCell<MeatpackState>>,
) -> impl Parser<I, Vec<u8>, nom::error::Error<I>>
where
    I: Clone
        + Slice<RangeFrom<usize>>
        + InputIter<Item = u8>
        + InputTake
        + InputLength
        + Compare<&'static [u8]>,
{
    let state_clone = state.clone();
    flat_map(tag(MP_COMMAND_HEADER.as_slice()), |_tag| le_u8)
        .map(move |command| {
            let mut state = state.borrow_mut();
            match command {
                MP_COMMAND_ENABLE_PACKING => state.packing = true,
                MP_COMMAND_DISABLE_PACKING => state.packing = false,
                MP_COMMAND_ENABLE_NO_SPACES => state.no_spaces = true,
                MP_COMMAND_DISABLE_NO_SPACES => state.no_spaces = false,
                MP_COMMAND_RESET_ALL => state.packing = false,
                MP_COMMAND_QUERY_CONFIG => {}
                _other => {}
            }
            vec![]
        })
        .or(decode_character_pair(state_clone).map(|pair| pair.to_vec()))
}

/// Decode the next pair of characters
fn decode_character_pair<I>(
    state: Rc<RefCell<MeatpackState>>,
) -> impl Parser<I, Vec<u8>, nom::error::Error<I>>
where
    I: Clone
        + Slice<RangeFrom<usize>>
        + InputIter<Item = u8>
        + InputTake
        + InputLength
        + Compare<&'static [u8]>,
{
    let both_unpacked_parser = tag(MP_BOTH_UNPACKABLE_HEADER.as_slice())
        .and(le_u8)
        .and(le_u8)
        .map(|((_tag, first), second)| [first, second].to_vec());

    let packed_parser = flat_map(le_u8, move |byte: u8| {
        let state = state.borrow();
        let first_unpacked = if state.packing {
            unpack_character(byte & MP_SINGLE_UNPACKABLE_MASK, state.no_spaces)
        } else {
            None
        };
        let second_unpacked = if state.packing {
            unpack_character((byte >> 4) & MP_SINGLE_UNPACKABLE_MASK, state.no_spaces)
        } else {
            None
        };
        cond(
            state.packing && (first_unpacked.is_none() || second_unpacked.is_none()),
            le_u8,
        )
        .map(move |next_byte| {
            let next_char = next_byte.map(|b| b);
            match (first_unpacked, second_unpacked) {
                (None, None) => [byte].to_vec(),
                (None, Some(second)) => [next_char.unwrap(), second].to_vec(),
                (Some(first), None) => [first, next_char.unwrap()].to_vec(),
                (Some(first), Some(second)) => [first, second].to_vec(),
            }
        })
    });

    both_unpacked_parser.or(packed_parser)
}

/// Lookup table for a 4-bit packed character
const fn unpack_character(x: u8, no_spaces: bool) -> Option<u8> {
    Some(match x {
        0 => b'0',
        1 => b'1',
        2 => b'2',
        3 => b'3',
        4 => b'4',
        5 => b'5',
        6 => b'6',
        7 => b'7',
        8 => b'8',
        9 => b'9',
        10 => b'.',
        11 if !no_spaces => b' ',
        11 if no_spaces => b'E',
        12 => b'\n',
        13 => b'G',
        14 => b'X',
        _other => return None,
    })
}
