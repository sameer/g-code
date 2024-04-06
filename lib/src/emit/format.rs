use rust_decimal::prelude::ToPrimitive;

use std::fmt::{self, Write as FmtWrite};
use std::io::Write as IoWrite;

use super::{Field, Token, Value};

#[cfg(feature = "serde")]
use serde::{Deserialize, Serialize};

struct XorAndPipe<W> {
    acc: u8,
    downstream: W,
}

impl<W> IoWrite for XorAndPipe<W>
where
    W: IoWrite,
{
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        self.acc = buf.iter().fold(self.acc, |acc, b| acc ^ b);
        self.downstream.write(buf)
    }

    fn flush(&mut self) -> std::io::Result<()> {
        self.downstream.flush()
    }
}

impl<W> FmtWrite for XorAndPipe<W>
where
    W: FmtWrite,
{
    fn write_str(&mut self, s: &str) -> fmt::Result {
        self.acc = s.bytes().fold(self.acc, |acc, b| acc ^ b);
        self.downstream.write_str(s)
    }
}

impl<W> XorAndPipe<W> {
    fn reset(&mut self) {
        self.acc = 0;
    }

    fn checksum(&self) -> u8 {
        self.acc
    }
}

#[derive(Debug, Clone, Default)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub struct FormatOptions {
    /// Include checksums
    pub checksums: bool,
    /// Add line numbers
    pub line_numbers: bool,
    /// Delimit the start and end of data with percent symbols
    pub delimit_with_percent: bool,
    /// Whether to add a newline before each comment
    ///
    /// Some g-code viewers like [NCViewer](https://ncviewer.com/)
    /// do not correctly handle comments on the same line as g-code commands
    #[cfg_attr(feature = "serde", serde(default))]
    pub newline_before_comment: bool,
}

macro_rules! formatter_core {
    ($program: expr, $opts: ident, $downstream: ident) => {
        use Token::*;
        let mut preceded_by_newline = true;
        let mut line_number = 0usize;

        let mut w = XorAndPipe {
            acc: 0,
            downstream: $downstream,
        };
        if $opts.delimit_with_percent {
            writeln!(w, "%")?;
            w.reset();
        }

        for token in $program {
            if let Token::Field(ref f) = token {
                // Can't handle user-provided line numbers
                if preceded_by_newline && f.letters == "N" {
                    continue;
                }
            }

            if $opts.line_numbers && preceded_by_newline {
                write!(w, "N{line_number} ")?;
            }

            match token {
                Field(f) => {
                    if !preceded_by_newline {
                        if matches!(f.letters.as_ref(), "G" | "M" | "D") {
                            if $opts.checksums {
                                write!(w, "*{}", w.checksum())?;
                            }
                            line_number += 1;
                            writeln!(w)?;
                            w.reset();
                            if $opts.line_numbers {
                                write!(w, "N{line_number} ")?;
                            }
                        } else {
                            write!(w, " ")?;
                        }
                    }
                    write!(w, "{f}")?;
                    preceded_by_newline = false;
                }
                Comment {
                    is_inline: true,
                    inner,
                } => {
                    write!(w, "({inner})")?;
                    preceded_by_newline = false;
                }
                Comment {
                    is_inline: false,
                    inner,
                } => {
                    if $opts.checksums {
                        write!(w, "*{}", w.checksum())?;
                    }
                    if !preceded_by_newline && $opts.newline_before_comment {
                        line_number += 1;
                        writeln!(w)?;
                    }
                    line_number += 1;
                    writeln!(w, ";{inner}")?;
                    w.reset();
                    preceded_by_newline = true;
                }
            }
        }
        // Ensure presence of trailing newline
        if !preceded_by_newline {
            if $opts.checksums {
                write!(w, "*{}", w.checksum())?;
                w.reset();
            }
            writeln!(w)?;
        }
        if $opts.delimit_with_percent {
            write!(w, "%")?;
        }
    };
}

/// Write GCode tokens to an [IoWrite] in a nicely formatted manner
pub fn format_gcode_io<'a: 'b, 'b, W>(
    program: impl IntoIterator<Item = &'b Token<'a>>,
    opts: FormatOptions,
    w: W,
) -> std::io::Result<()>
where
    W: IoWrite,
{
    formatter_core!(program.into_iter(), opts, w);
    Ok(())
}

/// Write formatted GCode to a [FmtWrite] in a nicely formatted manner
pub fn format_gcode_fmt<'a: 'b, 'b, W>(
    program: impl IntoIterator<Item = &'b Token<'a>>,
    opts: FormatOptions,
    w: W,
) -> fmt::Result
where
    W: FmtWrite,
{
    formatter_core!(program.into_iter(), opts, w);
    Ok(())
}

impl fmt::Display for Token<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        use Token::*;
        match self {
            Field(field) => write!(f, "{field}"),
            Comment { is_inline, inner } => match is_inline {
                true => write!(f, "({inner})"),
                false => write!(f, ";{inner}"),
            },
        }
    }
}

impl<'a> fmt::Display for Field<'a> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}{}", self.letters, self.value)
    }
}

impl fmt::Display for Value<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Rational(r) => {
                // The only way this could've been interpreted
                // as rational is if there is a trailing decimal point,
                // so add it back in.
                if r.fract().is_zero() {
                    if let Some(i64_rep) = r.to_i64() {
                        return write!(f, "{i64_rep}.");
                    }
                }
                write!(f, "{r}")
            }
            Self::Float(float) => write!(f, "{float}"),
            Self::Integer(i) => write!(f, "{i}"),
            Self::String(s) => write!(f, "\"{s}\""),
        }
    }
}
