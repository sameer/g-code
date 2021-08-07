use paste::paste;
use rust_decimal::prelude::ToPrimitive;
use rust_decimal::Decimal;

use std::borrow::Cow;
use std::fmt;

use crate::parse::token::Field as ParsedField;
use crate::parse::token::Value as ParsedValue;

#[derive(Clone, PartialEq, Debug)]
/// The output struct for gcode emission implementing [std::fmt::Display]
///
/// Any strings here are expected to have escaped characters, see <https://www.reprap.org/wiki/G-code#Quoted_strings>
pub enum Token<'a> {
    Field(Field<'a>),
    Comment {
        is_inline: bool,
        inner: Cow<'a, str>,
    },
    Checksum(u8),
}

impl<'input> From<&ParsedField<'input>> for Token<'input> {
    fn from(field: &ParsedField<'input>) -> Self {
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

/// Fundamental unit of g-code: a descriptive letter followed by a value.
///
/// Field type supports owned and partially-borrowed representations using [Cow].
#[derive(Clone, PartialEq, Debug)]
pub struct Field<'a> {
    pub letters: Cow<'a, str>,
    pub value: Value<'a>,
}

impl<'a> fmt::Display for Field<'a> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}{}", self.letters, self.value)
    }
}

impl<'input> From<&ParsedField<'input>> for Field<'input> {
    fn from(field: &ParsedField<'input>) -> Self {
        Self {
            letters: field.letters.into(),
            value: Value::from(&field.value),
        }
    }
}

impl<'a> From<Field<'a>> for Token<'a> {
    fn from(field: Field<'a>) -> Token<'a> {
        Self::Field(field)
    }
}

impl<'a> Field<'a> {
    /// Returns an owned representation of the Field valid for the `'static` lifetime.
    ///
    /// This will allocate any string types.
    pub fn into_owned(self) -> Field<'static> {
        Field {
            letters: self.letters.into_owned().into(),
            value: self.value.into_owned(),
        }
    }
}

/// All the possible variations of a field's value.
/// Some flavors of g-code also allow for strings.
///
/// Any strings here are expected to have escaped characters, see <https://www.reprap.org/wiki/G-code#Quoted_strings>
#[derive(Clone, PartialEq, Debug)]
pub enum Value<'a> {
    Rational(Decimal),
    Float(f64),
    Integer(usize),
    String(Cow<'a, str>),
}

impl Value<'_> {
    /// Interpret the value as an [f64]
    ///
    /// Returns [Option::None] for [Value::String] or a [Value::Rational] that can't be converted.
    pub fn as_f64(&self) -> Option<f64> {
        match self {
            Self::Rational(r) => r.to_f64(),
            Self::Integer(i) => Some(*i as f64),
            Self::Float(f) => Some(*f),
            Self::String(_) => None,
        }
    }

    /// Returns an owned representation of the Value valid for the `'static` lifetime.
    ///
    /// This will allocate a string for a [Value::String].
    pub fn into_owned(self) -> Value<'static> {
        match self {
            Self::String(s) => Value::String(s.into_owned().into()),
            Self::Rational(r) => Value::Rational(r),
            Self::Integer(i) => Value::Integer(i),
            Self::Float(f) => Value::Float(f),
        }
    }
}

impl<'input> From<&ParsedValue<'input>> for Value<'input> {
    fn from(val: &ParsedValue<'input>) -> Self {
        use ParsedValue::*;
        match val {
            Rational(r) => Self::Rational(*r),
            Integer(i) => Self::Integer(*i),
            String(s) => {
                // Remove enclosing quotes
                Self::String(Cow::Borrowed(
                    s.strip_prefix('"').unwrap().strip_suffix('"').unwrap(),
                ))
            }
        }
    }
}

impl fmt::Display for Value<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Rational(r) => {
                write!(f, "{}", r)?;
                // The only way this could've been interpreted
                // as rational is if there is a trailing decimal point,
                // so add it back in.
                if r.fract().is_zero() {
                    write!(f, ".")?;
                }
                Ok(())
            }
            Self::Float(float) => write!(f, "{}", float),
            Self::Integer(i) => write!(f, "{}", i),
            Self::String(s) => write!(f, "\"{}\"", s),
        }
    }
}

/// A macro for quickly instantiating a float-valued command
///
/// For instance:
///
/// ```
/// use g_code::command;
/// assert_eq!(command!(RapidPositioning { X: 0., Y: 1., }).iter().fold(String::default(), |s, f| s + &f.to_string()), "G0X0Y1");
/// ```
#[macro_export]
macro_rules! command {
    ($commandName: ident {
        $($arg: ident : $value: expr,)*
    }) => {
        {
            paste::expr!{
                g_code::emit::[<$commandName:snake:lower>](
                    vec![$(
                        g_code::emit::Field {
                            letters: std::borrow::Cow::Borrowed(stringify!([<$arg:upper>])),
                            value: g_code::emit::Value::Float($value),
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
                ///
                /// Call this function to instantiate the command.
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

                /// Constant for this command's name used to reduce allocations.
                pub const [<$commandName:snake:upper _FIELD>]: Field<'static> = Field {
                    letters: std::borrow::Cow::Borrowed($letters),
                    value: crate::emit::Value::Integer($value),
                };
            )*
        }

        /// Commands are the operational unit of g-code
        ///
        /// They consist of a G, M, or other top-level field followed by field arguments
        #[derive(Clone, PartialEq, Debug)]
        pub struct Command<'a> {
            name: Field<'a>,
            args: Vec<Field<'a>>,
        }

        impl<'a> Command<'a> {
            /// Add a field to the command.
            ///
            /// Returns [Err] if the Field's letters aren't recognized.
            pub fn push(&mut self, arg: Field<'a>) -> Result<(), &'static str> {
                paste!{
                    match &self.name {
                        $(x if *x == [<$commandName:snake:upper _FIELD>] => {
                            if match arg.letters.as_ref() {
                                $(stringify!([<$arg:upper>]) => {true},)*
                                $(stringify!([<$arg:lower>]) => {true},)*
                                _ => false,
                            } {
                                self.args.push(arg);
                                Ok(())
                            } else {
                                Err(concat!($(stringify!([<$arg:lower>]), " ", stringify!([<$arg:upper>]), " ", )*))
                            }
                        },)*
                        _ => {
                            unreachable!("a command's name cannot change");
                        }
                    }
                }
            }

            /// Iterate over all fields including the command's name (i.e. G0 for rapid positioning)
            pub fn iter(&self) -> impl Iterator<Item = &Field> {
                std::iter::once(&self.name).chain(self.args.iter())
            }

            /// Consumes the command to produce tokens suitable for output
            pub fn into_token_vec(mut self) -> Vec<Token<'a>> {
                std::iter::once(self.name).chain(self.args.drain(..)).map(|f| f.into()).collect()
            }

            /// Iterate over the fields after the command's name
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

            pub fn set(&mut self, letters: &str, value: Value<'a>) {
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
        "M", 2, {}
    },
);
