use num::ToPrimitive;
use num_rational::Ratio;
use paste::paste;

use std::borrow::Cow;
use std::fmt;

use crate::parse::token::Field as ParsedField;
use crate::parse::token::Value as ParsedValue;

#[derive(Clone, PartialEq, Debug)]
pub enum Token<'a> {
    Field(Field<'a>),
    Comment { is_inline: bool, inner: String },
    Checksum(u8),
}

impl<'a, 'input: 'a> From<&'a ParsedField<'input>> for Token<'a> {
    fn from(field: &'a ParsedField<'input>) -> Self {
        Self::Field(field.into())
    }
}

impl fmt::Display for Token<'_> {
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
pub struct Field<'a> {
    pub letters: Cow<'a, str>,
    pub value: Value,
}

impl<'a> fmt::Display for Field<'a> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}{}", self.letters, self.value)
    }
}

impl<'a, 'input: 'a> From<&'a ParsedField<'input>> for Field<'a> {
    fn from(field: &'a ParsedField<'input>) -> Self {
        Self {
            letters: field.letters.into(),
            value: Value::from(&field.value),
        }
    }
}

impl<'a> From<Field<'a>> for Token<'a> {
    fn from(field: Field<'a>) -> Token<'a> {
        Token::Field(field)
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

impl<'a, 'input: 'a> From<&'a ParsedValue<'input>> for Value {
    fn from(val: &'a ParsedValue<'input>) -> Self {
        use ParsedValue::*;
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
                            letters: Cow::Borrowed(stringify!([<$arg:upper>])),
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
                pub fn [<$commandName:snake:lower>]<'a, I: Iterator<Item = Field<'a>>>(args: I) -> Command<'a> {
                    Command {
                        name: [<$commandName:snake:upper _FIELD>].clone(),
                        args: args.filter(|arg| {
                            match arg.letters.to_ascii_uppercase().as_str() {
                                $(stringify!($arg) => true,)*
                                _ => false
                            }
                        }).collect(),
                    }
                }
                pub const [<$commandName:snake:upper _FIELD>]: Field = Field {
                    letters: Cow::Borrowed($letters),
                    value: Value::Integer($value),
                };
            )*
        }

        /// Commands are the operational unit of GCode
        /// They consist of a G, M, or other top-level field followed by field arguments
        #[derive(Clone, PartialEq, Debug)]
        pub struct Command<'a> {
            name: Field<'a>,
            args: Vec<Field<'a>>,
        }

        impl<'a> Command<'a> {
            pub fn push(&mut self, arg: Field<'a>) {
                match &self.name {
                    $(x if *x == paste!{[<$commandName:snake:upper _FIELD>]}.clone() => {
                        if match arg.letters.to_ascii_uppercase().as_str() {
                            $(stringify!($arg) => {true},)*
                            _ => false,
                        } {
                            self.args.push(arg);
                        } else {
                        }
                    },)*
                    _ => {
                        dbg!(&self.name);
                        dbg!(&arg);
                    }
                }
            }

            pub fn iter(&self) -> impl Iterator<Item = &Field> {
                std::iter::once(&self.name).chain(self.args.iter())
            }

            pub fn into_token_vec(mut self) -> Vec<Token<'a>> {
                std::iter::once(self.name).chain(self.args.drain(..)).map(|f| f.into()).collect()
            }

            pub fn iter_args(&self) -> impl Iterator<Item = &Field> {
                self.iter().skip(1)
            }

            pub fn iter_mut_args(&mut self) -> impl Iterator<Item = &mut Field<'a>> {
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
            X,
            Y,
            Z,
            E,
            F,
            H,
            R,
            S,
            A,
            B,
            C
        }
    },
    /// Typically used for "cutting" motion
    LinearInterpolation {
        "G", 1, {
            X,
            Y,
            Z,
            E,
            F,
            H,
            R,
            S,
            A,
            B,
            C
        }
    },
    /// This will keep the axes unmoving for the period of time in seconds specified by the P number
    Dwell {
        "G", 4, {
            /// Time in seconds
            P
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
            P
        }
    },
    /// Start spinning the spindle counterclockwise with speed `p`
    StartSpindleCounterclockwise {
        "M", 4, {
            /// Speed
            P
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
