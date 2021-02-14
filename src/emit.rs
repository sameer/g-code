use lazy_static::lazy_static;
use num::ToPrimitive;
use num_rational::Ratio;
use paste::paste;

use std::fmt;

use crate::parse::token::Field as TokField;
use crate::parse::token::Value as TokValue;

#[derive(Clone, PartialEq, Debug)]
pub enum Token {
    Field(Field),
    Comment { is_inline: bool, inner: String },
    Checksum(u8),
}

impl<'a, 'input: 'a> From<&'a TokField<'input>> for Token {
    fn from(field: &'a TokField<'input>) -> Self {
        Self::Field(field.into())
    }
}

impl fmt::Display for Token {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        use Token::*;
        match self {
            Field(field) => write!(f, "{}", field),
            Comment { is_inline, inner } => match is_inline {
                true => write!(f, "({})", inner),
                false => write!(f, ";{}", inner),
            },
            Checksum(c) => write!(f, "{}", c),
        }
    }
}

/// Fundamental unit of GCode: a value preceded by a descriptive letter.
#[derive(Clone, PartialEq, Debug)]
pub struct Field {
    pub letters: String,
    pub value: Value,
}

impl fmt::Display for Field {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}{}", self.letters, self.value)
    }
}

impl<'a, 'input: 'a> From<&'a TokField<'input>> for Field {
    fn from(field: &'a TokField<'input>) -> Self {
        Self {
            letters: field.letters.to_string(),
            value: Value::from(&field.value),
        }
    }
}

impl Into<Token> for Field {
    fn into(self) -> Token {
        Token::Field(self)
    }
}

/// All the possible variations of a field's value.
/// Some flavors of GCode also allow for strings.
#[derive(Clone, PartialEq, Debug)]
pub enum Value {
    Rational(Ratio<i64>),
    Float(f64),
    Integer(usize),
    String(String),
}

impl Value {
    pub fn as_f64(&self) -> Option<f64> {
        match self {
            Self::Rational(r) => r.to_f64(),
            Self::Integer(i) => Some(*i as f64),
            Self::Float(f) => Some(*f),
            Self::String(_) => None,
        }
    }
}

impl<'a, 'input: 'a> From<&'a TokValue<'input>> for Value {
    fn from(val: &'a TokValue<'input>) -> Self {
        use TokValue::*;
        match val {
            Rational(r) => Self::Rational(*r),
            Integer(i) => Self::Integer(*i),
            String(s) => Self::String(s.to_string()),
        }
    }
}

impl fmt::Display for Value {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Rational(r) => write!(f, "{}", r.to_f64().ok_or(fmt::Error)?),
            Self::Float(float) => write!(f, "{}", float),
            Self::Integer(i) => write!(f, "{}", i),
            Self::String(s) => write!(f, "\"{}\"", s),
        }
    }
}

/// A macro for quickly instantiating a float-valued command
#[macro_export]
macro_rules! command {
    ($commandName: ident {
        $($arg: ident : $value: expr,)*
    }) => {
        {
            use g_code::emit::*;
            use paste::paste;
            paste::expr!{
                [<$commandName:snake:lower>](
                    vec![$(
                        Field {
                            letters: stringify!([<$arg:upper>]).to_string(),
                            value: Value::Float($value),
                        }
                    ,)*].drain(..)
                )
            }
        }
    };
}

macro_rules! impl_commands {
    ($($(#[$outer:meta])* $commandName: ident {$letters: expr, $value: literal, {$($(#[$inner:meta])* $arg: ident), *} },)*) => {

        paste! {
            $(
                $(#[$outer])*
                pub fn [<$commandName:snake:lower>]<I: Iterator<Item = Field>>(args: I) -> Command {
                    Command {
                        name: [<$commandName:snake:upper _FIELD>].clone(),
                        args: args.filter(|arg| {
                            match arg.letters.to_lowercase().as_str() {
                                $(stringify!($arg) => true,)*
                                _ => false
                            }
                        }).collect(),
                    }
                }

                lazy_static! {
                    pub static ref [<$commandName:snake:upper _FIELD>]: Field = Field {
                        letters: $letters.to_string(),
                        value:Value::Integer($value),
                    };
                }
            )*
        }

        /// Commands are the operational unit of GCode
        /// They consist of a G, M, or other top-level field followed by field arguments
        #[derive(Clone, PartialEq, Debug)]
        pub struct Command {
            name: Field,
            args: Vec<Field>,
        }

        impl Command {
            pub fn push(&mut self, arg: Field) {
                match self.name.letters.as_str() {
                    $(stringify!($letters) => match arg.letters.to_lowercase().as_str() {
                        $(stringify!($arg) => {
                            self.args.push(arg);
                        })*
                        _ => {}
                    },)*
                    _ => {}
                }
            }

            pub fn iter(&self) -> impl Iterator<Item = &Field> {
                std::iter::once(&self.name).chain(self.args.iter())
            }

            pub fn iter_args(&self) -> impl Iterator<Item = &Field> {
                self.iter().skip(1)
            }

            pub fn iter_mut_args(&mut self) -> impl Iterator<Item = &mut Field> {
                self.args.iter_mut()
            }

            pub fn get(&'_ self, letters: &str) -> Option<&'_ Field> {
                let letters = letters.to_ascii_uppercase();
                self.iter_args().find(|arg| arg.letters == letters)
            }

            pub fn set(&mut self, letters: &str, value: Value) {
                let letters = letters.to_ascii_uppercase();
                for i in 0..self.args.len() {
                    if self.args[i].letters == letters {
                        self.args[i].value = value;
                        break;
                    }
                }
            }
        }
    };
}

impl_commands!(
    /// Moves the head at the fastest possible speed to the desired speed
    /// Never enter a cut with rapid positioning
    /// Some older machines may "dog leg" rapid positioning, moving one axis at a time
    RapidPositioning {
        "G", 0, {
            x,
            y,
            z,
            e,
            f,
            h,
            r,
            s,
            a,
            b,
            c
        }
    },
    /// Typically used for "cutting" motion
    LinearInterpolation {
        "G", 1, {
            x,
            y,
            z,
            e,
            f,
            h,
            r,
            s,
            a,
            b,
            c
        }
    },
    /// This will keep the axes unmoving for the period of time in seconds specified by the P number
    Dwell {
        "G", 4, {
            /// Time in seconds
            p
        }
    },
    /// Use inches for length units
    UnitsInches {
        "G", 20, {}
    },
    /// Use millimeters for length units
    UnitsMillimeters {
        "G", 21, {}
    },
    /// In absolute distance mode, axis numbers usually represent positions in terms of the currently active coordinate system.
    AbsoluteDistanceMode {
        "G", 90, {}
    },
    /// In relative distance mode, axis numbers usually represent increments from the current values of the numbers
    RelativeDistanceMode {
        "G", 91, {}
    },
    FeedRateUnitsPerMinute {
        "G", 94, {}
    },
    /// Start spinning the spindle clockwise with speed `p`
    StartSpindleClockwise {
        "M", 3, {
            /// Speed
            p
        }
    },
    /// Start spinning the spindle counterclockwise with speed `p`
    StartSpindleCounterclockwise {
        "M", 4, {
            /// Speed
            p
        }
    },
    /// Stop spinning the spindle
    StopSpindle {
        "M", 5, {}
    },
    /// Signals the end of a program
    ProgramEnd {
        "M", 20, {}
    },
);
