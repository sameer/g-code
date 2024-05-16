//! Higher-level constructs for g-code emission

use paste::paste;

use super::{Field, Token, Value};

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
        $($arg: ident : $value: expr),* $(,)?
    }) => {
        {
            paste::expr!{
                g_code::emit::command::[<$commandName:snake:lower>](
                    vec![$(
                        g_code::emit::Field {
                            letters: std::borrow::Cow::Borrowed(stringify!([<$arg:upper>])),
                            value: g_code::emit::Value::Float($value),
                        }
                    ,)*].into_iter()
                )
            }
        }
    };
}

macro_rules! impl_commands {
    ($($(#[$outer:meta])* $commandName: ident {$letters: expr, $value: literal, {$($(#[$inner:meta])* $arg: ident), *} } )*) => {

        paste! {
            $(
                $(#[$outer])*
                ///
                /// To instantiate the command, call this function
                /// or use the [crate::command] macro.
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
            /// Returns an error if the Field's letters aren't recognized.
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
    /// Moves the head to the desired position
    /// at the fastest possible speed.
    ///
    /// *NEVER* enter a cut with rapid positioning.
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
    }
    /// Interpolate along a line to the desired position
    ///
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
    }
    /// Interpolate along an arc to the desired position
    ///
    /// The machine will maintain either a constant distance
    /// from the arc's center `(I, J, K)` or a constant radius `R`.
    ///
    /// Not all machines support this command. Those that do typically
    /// recommend short arcs. Some may have a maximum supported radius.
    ClockwiseCircularInterpolation {
        "G", 2, {
            X,
            Y,
            Z,
            I,
            J,
            K,
            E,
            F,
            R
        }
    }
    /// See guidance on [clockwise_circular_interpolation]
    CounterclockwiseCircularInterpolation {
        "G", 3, {
            X,
            Y,
            Z,
            I,
            J,
            K,
            E,
            F,
            R
        }
    }
    /// This will keep the axes unmoving for the period of time in seconds specified by the P number
    Dwell {
        "G", 4, {
            /// Time in seconds
            P
        }
    }
    /// Use inches for length units
    UnitsInches {
        "G", 20, {}
    }
    /// Use millimeters for length units
    UnitsMillimeters {
        "G", 21, {}
    }
    /// In absolute distance mode, axis numbers usually represent positions in terms of the currently active coordinate system.
    AbsoluteDistanceMode {
        "G", 90, {}
    }
    /// In relative distance mode, axis numbers usually represent increments from the current values of the numbers
    RelativeDistanceMode {
        "G", 91, {}
    }
    FeedRateUnitsPerMinute {
        "G", 94, {}
    }
    /// Start spinning the spindle clockwise with speed `p`
    StartSpindleClockwise {
        "M", 3, {
            /// Speed
            P
        }
    }
    /// Start spinning the spindle counterclockwise with speed `p`
    StartSpindleCounterclockwise {
        "M", 4, {
            /// Speed
            P
        }
    }
    /// Stop spinning the spindle
    StopSpindle {
        "M", 5, {}
    }
    /// Signals the end of a program
    ProgramEnd {
        "M", 2, {}
    }
);
