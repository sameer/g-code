//! Compact g-code representation using meatpack
//!
//! <https://github.com/scottmudge/OctoPrint-MeatPack/>

use crate::{
    emit::{token::Flag, Field, FormatOptions, Token, Value},
    parse::compact::meatpack::*,
};

use std::{borrow::Borrow, fmt::Arguments, io::Write as IoWrite};

#[derive(Clone)]
pub struct MeatpackOptions {
    pub no_spaces: bool,
}

struct MeatpackEncodingWriter<W> {
    downstream: W,
    meatpack_opts: MeatpackOptions,
    checksum_acc: u8,
    enabled: bool,
    pending: Option<(u8, bool)>,
}

impl<W> MeatpackEncodingWriter<W>
where
    W: IoWrite,
{
    fn new(downstream: W, meatpack_opts: MeatpackOptions) -> Self {
        Self {
            downstream,
            meatpack_opts,
            checksum_acc: 0,
            enabled: false,
            pending: None,
        }
    }

    fn start_meatpack(&mut self) -> std::io::Result<()> {
        if self.meatpack_opts.no_spaces {
            self.downstream.write_all(&MP_COMMAND_HEADER)?;
            self.downstream.write_all(&[MP_COMMAND_ENABLE_NO_SPACES])?;
        }
        Ok(())
    }

    fn stop_meatpack(&mut self) -> std::io::Result<()> {
        if self.pending.is_some() {
            self.enable_packing()?;
            self.write_pending()?;
        }
        self.downstream.write_all(&MP_COMMAND_HEADER)?;
        self.downstream.write_all(&[MP_COMMAND_RESET_ALL])?;
        self.enabled = false;
        Ok(())
    }

    fn enable_packing(&mut self) -> std::io::Result<()> {
        if !self.enabled {
            self.downstream.write_all(&MP_COMMAND_HEADER)?;
            self.downstream.write_all(&[MP_COMMAND_ENABLE_PACKING])?;
        }
        self.enabled = true;
        Ok(())
    }

    fn disable_packing(&mut self) -> std::io::Result<()> {
        if self.enabled {
            self.write_pending()?;
            self.downstream.write_all(&MP_COMMAND_HEADER)?;
            self.downstream.write_all(&[MP_COMMAND_DISABLE_PACKING])?;
        }
        self.enabled = false;
        Ok(())
    }

    fn will_write_newline(&mut self) -> bool {
        self.enabled && self.meatpack_opts.no_spaces && self.pending.is_some()
    }

    fn write_pending(&mut self) -> std::io::Result<()> {
        if let Some((pending_byte, _)) = self.pending.take() {
            self.write_slice(
                [
                    pending_byte,
                    if self.meatpack_opts.no_spaces {
                        b'\n'
                    } else {
                        b' '
                    },
                ]
                .as_slice(),
            )?;
        }
        Ok(())
    }

    fn checksum(&self) -> u8 {
        let mut checksum = self.checksum_acc;
        if let Some((pending_byte, include_pending_in_checksum)) = self.pending {
            if include_pending_in_checksum {
                if self.enabled {
                    checksum ^= packable_to_uppercase(pending_byte, self.meatpack_opts.no_spaces);
                } else {
                    checksum ^= pending_byte;
                }
            }
        }
        checksum
    }

    fn reset_checksum(&mut self) {
        self.checksum_acc = 0;
        if let Some((_, ref mut include_pending_in_checksum)) = self.pending {
            *include_pending_in_checksum = false;
        }
    }

    fn write_fmt(&mut self, arguments: Arguments<'_>) -> std::io::Result<()> {
        let input = arguments.to_string();
        if !self.enabled {
            self.checksum_acc = input.bytes().fold(self.checksum_acc, |acc, b| acc ^ b);
            self.downstream.write_all(input.as_bytes())?;
        } else if !input.is_empty() {
            assert!(input.is_ascii(), "Meatpack can only encode ASCII");
            if self.meatpack_opts.no_spaces {
                for substr in input.split(' ') {
                    self.write_slice(substr.as_bytes())?;
                }
            } else {
                self.write_slice(input.as_bytes())?;
            }
        }

        Ok(())
    }

    fn write_slice(&mut self, mut slice: &[u8]) -> std::io::Result<()> {
        if slice.is_empty() {
            return Ok(());
        }
        let mut pending_slice_opt = None;
        let mut _pending_array_opt = None;
        if let Some((pending, include_pending_in_checksum)) = self.pending.take() {
            _pending_array_opt = Some([pending, slice[0]]);
            pending_slice_opt = _pending_array_opt
                .as_ref()
                .map(|array| (array.as_slice(), include_pending_in_checksum));
            slice = &slice[1..];
        }

        for (chunk, include_first_in_checksum) in pending_slice_opt
            .into_iter()
            .chain(slice.chunks(2).map(|c| (c, true)))
        {
            match chunk {
                [first, second] => {
                    let first = packable_to_uppercase(*first, self.meatpack_opts.no_spaces);
                    let second = packable_to_uppercase(*second, self.meatpack_opts.no_spaces);
                    if include_first_in_checksum {
                        self.checksum_acc ^= first;
                    }
                    self.checksum_acc ^= second;
                    write_packed_characters(
                        [first, second],
                        self.meatpack_opts.no_spaces,
                        &mut self.downstream,
                    )?;
                }
                [odd] => self.pending = Some((*odd, true)),
                _ => unreachable!(),
            }
        }
        Ok(())
    }
}

/// Write g-code to a [std::io::Write] in a meatpacked representation
pub fn format_gcode_meatpack<'a: 'b, 'b, W, I, T>(
    program: I,
    opts: FormatOptions,
    meatpack_opts: MeatpackOptions,
    w: W,
) -> std::io::Result<()>
where
    W: IoWrite,
    I: IntoIterator<Item = T>,
    T: Borrow<Token<'a>> + 'b,
{
    let mut preceded_by_newline = true;
    let mut line_number = 0usize;

    let mut w = MeatpackEncodingWriter::new(w, meatpack_opts);
    w.start_meatpack()?;

    if opts.delimit_with_percent {
        writeln!(w, "%")?;
        w.reset_checksum();
    }

    for token in program {
        let token = token.borrow();
        if let Token::Field(ref f) = token {
            // Can't handle user-provided line numbers
            if preceded_by_newline && f.letters == "N" {
                continue;
            }
        }

        // Disable meatpack if there are non-ASCII characters,
        // it doesn't handle multi-byte chars.
        let disable_meatpack = match token {
            Token::Field(Field { letters, value }) => {
                !letters.is_ascii()
                    || match value {
                        Value::String(_) => true,
                        Value::Rational(_) | Value::Float(_) | Value::Integer(_) => false,
                    }
            }
            Token::Flag(Flag { letter }) => !letter.is_ascii(),
            Token::Comment { .. } => true,
        };

        if disable_meatpack {
            let will_write_newline = w.will_write_newline();
            if will_write_newline {
                if opts.line_numbers && preceded_by_newline {
                    write!(w, "N{line_number} ")?;
                }

                if opts.checksums {
                    write!(w, "*{}", w.checksum())?;
                }
                line_number += 1;
            }
            let still_going_to_write_newline = w.will_write_newline();
            w.disable_packing()?;
            if will_write_newline {
                // Degenerate case: odd number of chars became even, oops
                if !still_going_to_write_newline {
                    writeln!(w)?;
                }
                w.reset_checksum();
            }
            preceded_by_newline = will_write_newline;
        } else {
            w.enable_packing()?;
        }

        if opts.line_numbers && preceded_by_newline {
            write!(w, "N{line_number} ")?;
        }

        match token {
            Token::Field(f) => {
                if !preceded_by_newline {
                    if matches!(f.letters.as_ref(), "G" | "g" | "M" | "m" | "D" | "d") {
                        if opts.checksums {
                            write!(w, "*{}", w.checksum())?;
                        }
                        line_number += 1;
                        writeln!(w)?;
                        w.reset_checksum();
                        if opts.line_numbers {
                            write!(w, "N{line_number} ")?;
                        }
                    } else {
                        write!(w, " ")?;
                    }
                }

                write!(w, "{f}")?;
                preceded_by_newline = false;
            }
            Token::Flag(f) => {
                if !preceded_by_newline {
                    write!(w, " ")?;
                }
                write!(w, "{f}")?;
            }
            Token::Comment {
                is_inline: true,
                inner,
            } => {
                write!(w, "({inner})")?;
                preceded_by_newline = false;
            }
            Token::Comment {
                is_inline: false,
                inner,
            } => {
                if opts.checksums {
                    write!(w, "*{}", w.checksum())?;
                }
                if !preceded_by_newline && opts.newline_before_comment {
                    line_number += 1;
                    writeln!(w)?;
                    w.reset_checksum();
                    if opts.line_numbers {
                        write!(w, "N{line_number} ")?;
                    }
                    if opts.checksums {
                        write!(w, "*{}", w.checksum())?;
                    }
                }
                line_number += 1;
                writeln!(w, ";{inner}")?;
                w.reset_checksum();
                preceded_by_newline = true;
            }
        }
    }
    // Ensure presence of trailing newline
    if !preceded_by_newline {
        if opts.checksums {
            write!(w, "*{}", w.checksum())?;
            w.reset_checksum();
        }
        writeln!(w)?;
    }

    w.stop_meatpack()?;

    if opts.delimit_with_percent {
        write!(w, "%")?;
    }
    Ok(())
}

/// Pack a pair of characters into an [IoWrite]
fn write_packed_characters<W>(
    [first, second]: [u8; 2],
    no_spaces: bool,
    dest: &mut W,
) -> std::io::Result<()>
where
    W: IoWrite,
{
    match (pack_char(first, no_spaces), pack_char(second, no_spaces)) {
        (None, None) => {
            dest.write_all(&MP_BOTH_UNPACKABLE_HEADER)?;
            dest.write_all(&[first, second])
        }
        (None, Some(second)) => dest.write_all(&[(second << 4) | MP_SINGLE_UNPACKABLE_MASK, first]),
        (Some(first), None) => dest.write_all(&[first | (MP_SINGLE_UNPACKABLE_MASK << 4), second]),
        (Some(first), Some(second)) => dest.write_all(&[first | (second << 4)]),
    }
}

const fn pack_char(c: u8, no_spaces: bool) -> Option<u8> {
    Some(match c {
        b'0' => 0,
        b'1' => 1,
        b'2' => 2,
        b'3' => 3,
        b'4' => 4,
        b'5' => 5,
        b'6' => 6,
        b'7' => 7,
        b'8' => 8,
        b'9' => 9,
        b'.' => 10,
        b' ' if !no_spaces => 11,
        b'E' if no_spaces => 11,
        b'\n' => 12,
        b'G' => 13,
        b'X' => 14,
        _other => return None,
    })
}

const fn packable_to_uppercase(c: u8, no_spaces: bool) -> u8 {
    match c {
        b'e' if !no_spaces => b'E',
        b'g' => b'G',
        b'x' => b'X',
        other => other,
    }
}
