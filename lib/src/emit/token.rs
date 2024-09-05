use rust_decimal::prelude::ToPrimitive;
use rust_decimal::Decimal;

use std::borrow::Cow;

use crate::parse::token::{
    Comment as ParsedComment, Field as ParsedField, Flag as ParsedFlag,
    InlineComment as ParsedInlineComment, Value as ParsedValue,
};

#[derive(Clone, PartialEq, Debug)]
/// The output struct for g-code emission implementing [std::fmt::Display]
///
/// Any strings here are expected to have escaped characters, see <https://www.reprap.org/wiki/G-code#Quoted_strings>
pub enum Token<'a> {
    Field(Field<'a>),
    Flag(Flag<'a>),
    Comment {
        is_inline: bool,
        inner: Cow<'a, str>,
    },
}

impl<'input> From<&ParsedField<'input>> for Token<'input> {
    fn from(field: &ParsedField<'input>) -> Self {
        Self::Field(field.into())
    }
}

impl<'input> From<&ParsedFlag<'input>> for Token<'input> {
    fn from(flag: &ParsedFlag<'input>) -> Self {
        Self::Flag(flag.into())
    }
}

impl<'a, 'input: 'a> From<&'a ParsedInlineComment<'input>> for Token<'input> {
    fn from(comment: &'a ParsedInlineComment<'input>) -> Self {
        Self::Comment {
            is_inline: true,
            inner: Cow::Borrowed(
                comment
                    .inner
                    .strip_prefix('(')
                    .unwrap()
                    .strip_suffix(')')
                    .unwrap(),
            ),
        }
    }
}

impl<'input> From<&ParsedComment<'input>> for Token<'input> {
    fn from(comment: &ParsedComment<'input>) -> Self {
        Self::Comment {
            is_inline: false,
            inner: Cow::Borrowed(comment.inner.strip_prefix(';').unwrap()),
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

#[derive(Clone, PartialEq, Debug)]
pub struct Flag<'a> {
    pub letter: Cow<'a, str>,
}

impl<'input> From<&ParsedFlag<'input>> for Flag<'input> {
    fn from(flag: &ParsedFlag<'input>) -> Self {
        Self {
            letter: flag.letter.into(),
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
