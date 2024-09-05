use rust_decimal::Error;

fn decimal_err_into_str(err: Error) -> &'static str {
    match err {
        Error::ExceedsMaximumPossibleValue => {
            "number is exceeds than the maximum that can be represented"
        }
        Error::LessThanMinimumPossibleValue => {
            "number is less than the minimum that can be represented"
        }
        Error::ScaleExceedsMaximumPrecision(_) => {
            "precision necessary to represent exceeds the maximum possible"
        }
        Error::ErrorString(_) => "cannot parse as decimal (unable to display root cause)",
        Error::Underflow => "number contains more fractional digits than can be represented",
        Error::ConversionTo(_) => "cannot convert to/from decimal type",
    }
}

peg::parser! {
    pub grammar g_code() for str {
        use super::super::token::*;
        use super::super::ast::*;
        use rust_decimal::Decimal;

        pub rule newline() -> Newline = pos:position!() inner:(quiet!{ $("\r\n" / "\r" / "\n") } / expected!("newline")) {
            Newline {
                pos
            }
        };
        pub rule dot() -> &'input str = quiet! { $(".") } / expected!("decimal point");
        pub rule star() -> &'input str = quiet!{ $("*") } / expected!("checksum asterisk");
        pub rule minus() -> &'input str = quiet!{ $("-") } / expected!("minus sign");
        pub rule percent() -> Percent = pos:position!()  inner:(quiet! { $("%") } / expected!("percent sign")) {
            Percent {
                pos,
            }
        };
        rule quotation_mark() -> &'input str = quiet! { $("\"") } / expected!("quotation mark");
        rule ascii_except_quote_or_newline() -> &'input str = quiet! { $(['\t'  | ' '..='!'| '#'..='~']*) } / expected!("ASCII character except quote or newline");
        pub rule string() -> &'input str = $(quotation_mark() ascii_except_quote_or_newline() ((quotation_mark() quotation_mark())+ ascii_except_quote_or_newline())* quotation_mark());
        rule ascii_except_closing_parenthesis_or_newline() -> &'input str = quiet! { $(['\t' | ' '..='(' | '*'..='~']*) } / expected!("ASCII character except closing parenthesis or newline");
        rule opening_parenthesis() -> &'input str = quiet! { $("(") } / expected!("opening parenthesis");
        rule closing_parenthesis() -> &'input str = quiet! { $(")") } / expected!("closing parenthesis");

        rule inline_comment_raw() -> &'input str = precedence! {
            x:$(opening_parenthesis() inline_comment_raw() closing_parenthesis()) { x }
            --
            x:$(opening_parenthesis() ascii_except_closing_parenthesis_or_newline() closing_parenthesis()) { x }
        };
        pub rule inline_comment() -> InlineComment<'input> = pos:position!() inner:inline_comment_raw() {
            InlineComment {
                inner,
                pos,
            }
        };
        rule ascii_character_except_newline() -> &'input str = quiet!{ $(['\t' | ' '..='~']*) } / expected!("ASCII character");
        rule semicolon() -> &'input str = quiet! { $(";") } / expected!("semicolon");
        pub rule comment() -> Comment<'input> = pos:position!() inner:$(semicolon() ascii_character_except_newline()) {
            Comment {
                inner,
                pos,
            }
        };
        pub rule integer() -> &'input str = quiet! { $(['0'..='9']+) } / expected!("integer");
        pub rule letter() -> &'input str = quiet! { $(['a'..='z' | 'A'..='Z']) } / expected!("letter");
        pub rule letters() -> &'input str = quiet! { $(['a'..='z' | 'A'..='Z']+) } / expected!("letters");
        pub rule whitespace() -> Whitespace<'input> = pos:position!() inner:(quiet! { $([' ' | '\t' ]+) } / expected!("whitespace")) {
            Whitespace {
                inner,
                pos,
            }
        };

        rule checksum() -> Checksum = left:position!() star:star() checksum:integer() right:position!() {?
            Ok(Checksum {
                inner: checksum.parse::<u8>().map_err(|e| "checksum is not an unsigned byte")?,
                span: Span(left, right)
            })
        };

        rule field() -> Field<'input>
            = left:position!() letters:letters() neg:minus()? lhs:integer()  dot:dot() rhs:integer()? right:position!() {?
                let lhs_start = left + letters.len() + neg.as_ref().map(|_| 1).unwrap_or(0);
                let lhs_end = lhs_start + lhs.len();
                let rhs_start = lhs_end + 1;
                let rhs_end = rhs_start + rhs.map(|x| x.len()).unwrap_or(0);
                Ok(Field {
                    letters,
                    value: Value::Rational(lhs.parse::<Decimal>()
                        .map_err(decimal_err_into_str)
                        .and_then(|lhs| if let Some(rhs_str) = rhs {
                            rhs_str.parse::<i64>()
                                .map_err(|e| "fractional part does not fit in an i64")
                                .and_then(|rhs| if rhs_str.len() > 28 { Err("scale is higher than allowed maximum precision") } else { Ok(rhs)})
                                .map(|rhs| Decimal::new(rhs, rhs_str.len() as u32))
                                .map(|rhs| lhs + rhs)
                                .map(|value| if neg.is_some() { -value } else { value })
                        } else {
                            Ok(
                                if neg.is_some() { -lhs } else { lhs }
                            )
                        })?),
                    raw_value: if neg.is_some() { vec!["-", lhs, ".", rhs.unwrap_or("")] } else { vec![lhs, ".", rhs.unwrap_or("")] },
                    span: Span(left, right)
                })
            }
            / left:position!() letters:letters() neg:minus()? dot:dot() rhs_str:integer() right:position!() {?
                let rhs_start = left + letters.len() + neg.as_ref().map(|_| 1).unwrap_or(0) + 1;
                let rhs_end = right;
                Ok(Field {
                    letters,
                    value: Value::Rational(rhs_str.parse::<i64>()
                        .map_err(|e| "fractional part does not fit in an i64")
                        .and_then(|rhs| if rhs_str.len() > 28 { Err("scale is higher than allowed maximum precision") } else { Ok(rhs)})
                        .map(|rhs| Decimal::new(rhs, rhs_str.len() as u32))
                        .map(|rhs| if neg.is_some() { -rhs } else { rhs })?),
                    raw_value: if neg.is_some() { vec!["-", ".", rhs_str] } else { vec![".", rhs_str] },
                    span: Span(left, right)
                })
            }
            / left:position!() letters:letters() value:integer() right:position!() {?
                Ok(Field {
                    letters,
                    value: Value::Integer(value.parse::<usize>().map_err(|e| "integer does not fit in usize")?),
                    raw_value: vec![value],
                    span: Span(left, right)
                })
            }

            / left:position!() letters:letters() minus:minus() value:integer() right:position!() {?
                let value_start = left + letters.len() + 1;
                let value_end = right;
                Ok(Field {
                    letters,
                    value: Value::Rational(-value.parse::<Decimal>().map_err(decimal_err_into_str)?),
                    raw_value: vec!["-", value],
                    span: Span(left, right)
                })
            }
            / left:position!() letters:letters() string:string() right:position!() {
                Field {
                    letters,
                    value: Value::String(string),
                    raw_value: vec![string],
                    span: Span(left, right)
                }
            };

        pub rule flag() -> Flag<'input> = left:position!() letter:letter() right:position!() {
            Flag { letter, span: Span(left, right) }
        };

        rule line_component() -> LineComponent<'input>
            = field:field() { LineComponent { field: Some(field), ..Default::default() } }
            / flag:flag() { LineComponent { flag: Some(flag), ..Default::default() }}
            / whitespace:whitespace() { LineComponent { whitespace: Some(whitespace), ..Default::default() } }
            / inline_comment:inline_comment() { LineComponent { inline_comment: Some(inline_comment), ..Default::default() } };

        pub rule line() -> Line<'input> =
                left:position!()
                     // Hacky way of imitating lalrpop following https://github.com/kevinmehall/rust-peg/blob/master/peg-macros/grammar.rustpeg#L90
                    line_components:line_component()*
                    checksum:checksum()?
                    comment:comment()?
                right:position!() {
            Line {
                line_components,
                checksum,
                comment,
                span: Span(left, right)
            }
        };

        /// Parse a g-code file
        pub rule file_parser() -> File<'input>
            = left:position!() start_percent:percent() lines:(a:line() b:newline() { (a, b) })* last_line:line() end_percent:percent() right:position!() {
                File {
                    percents: vec![start_percent, end_percent],
                    lines,
                    last_line: if last_line.line_components.is_empty() && last_line.checksum.is_none() && last_line.comment.is_none() {
                        None
                    } else {
                        Some(last_line)
                    },
                    span: Span(left, right)
                }
            }
            / left:position!() lines:(a:line() b:newline() { (a, b) })* last_line:line() right:position!() {
                File {
                    percents: vec![],
                    lines,
                    last_line: if last_line.line_components.is_empty() && last_line.checksum.is_none() && last_line.comment.is_none() {
                        None
                    } else {
                        Some(last_line)
                    },
                    span: Span(left, right)
                }
            };

        /// The snippet parser is identical to the [file_parser], but it does not allow a leading and trailing percent symbol
        pub rule snippet_parser() -> Snippet<'input> = left:position!() lines:(a:line() b:newline() { (a, b) })* last_line:line() right:position!() {
            Snippet {
                lines,
                last_line: if last_line.line_components.is_empty() && last_line.checksum.is_none() && last_line.comment.is_none() {
                    None
                } else {
                    Some(last_line)
                },
                span: Span(left, right)
            }
        }
    }
}
