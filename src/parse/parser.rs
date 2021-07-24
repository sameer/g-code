peg::parser! {
    pub grammar g_code() for str{
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
        pub rule percent() -> &'input str = quiet! { $("%") } / expected!("percent sign");
        rule quotation_mark() -> &'input str = quiet! { $("\"") } / expected!("quotation mark");
        rule ascii_except_quote_or_newline() -> &'input str = quiet! { $(['\t'  | ' '..='!'| '#'..='~']*) } / expected!("ASCII character except quote or newline");
        pub rule string() -> &'input str = $(quotation_mark() ascii_except_quote_or_newline() ((quotation_mark() quotation_mark())+ ascii_except_quote_or_newline())* quotation_mark());
        rule ascii_except_closing_parenthesis_or_newline() -> &'input str = quiet! { $(['\t' | ' '..='(' | '*'..='~']*) } / expected!("ASCII character except closing parenthesis or newline");
        rule opening_parenthesis() -> &'input str = quiet! { $("(") } / expected!("opening parenthesis");
        rule closing_parenthesis() -> &'input str = quiet! { $(")") } / expected!("closing parenthesis");
        pub rule inline_comment() -> InlineComment<'input> = pos:position!() inner:$(opening_parenthesis() ascii_except_closing_parenthesis_or_newline() closing_parenthesis()) {
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
        pub rule letters() -> &'input str = quiet! { $(['a'..='z' | 'A'..='Z']+) } / expected!("letters");
        pub rule whitespace() -> Whitespace<'input> = pos:position!() inner:(quiet! { $([' ' | '\t' ]+) } / expected!("whitespace")) {
            Whitespace {
                inner,
                pos,
            }
        };

        pub rule checksum() -> Checksum = left:position!() star:star() checksum:integer() right:position!() {?
            Ok(Checksum {
                inner: checksum.parse::<u8>().map_err(|e| "checksum is not byte-sized")?,
                span: Span(left, right)
            })
        };

        pub rule field() -> Field<'input>
            = left:position!() letters:letters() neg:minus()? lhs:integer()  dot:dot() rhs:integer()? right:position!() {?
                let lhs_start = left + letters.len() + neg.as_ref().map(|_| 1).unwrap_or(0);
                let lhs_end = lhs_start + lhs.len();
                let rhs_start = lhs_end + 1;
                let rhs_end = rhs_start + rhs.map(|x| x.len()).unwrap_or(0);
                Ok(Field {
                    letters,
                    value: Value::Rational(lhs.parse::<Decimal>()
                        .map_err(|e| "integer part does not fit in an i64")
                        .and_then(|lhs| if let Some(rhs_str) = rhs {
                            rhs_str.parse::<i64>()
                                .map(|rhs| Decimal::new(rhs, rhs_str.len() as u32))
                                .map(|rhs| lhs + rhs)
                                .map(|value| if neg.is_some() { -value } else { value })
                                .map_err(|e| "fractional part does not fit in an i64")
                        } else {
                            Ok(lhs)
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
                        .map(|rhs| Decimal::new(rhs, rhs_str.len() as u32))
                        .map(|rhs| if neg.is_some() { -rhs } else { rhs })
                        .map_err(|e| "fractional part does not fit in an i64")?),
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
                    value: Value::Rational(-value.parse::<Decimal>().map_err(|e| "integer does not fit in i64")?),
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

        rule line_component() -> LineComponent<'input>
            = field:field() { LineComponent { field: Some(field), ..Default::default() } }
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

        /// Parse a GCode file
        pub rule file_parser() -> File<'input>
            = left:position!() start_percent:percent() lines:(a:line() b:newline() { (a, b) })* last_line:line() end_percent:percent() right:position!() {
                File {
                    start_percent: true,
                    lines,
                    last_line: if last_line.line_components.is_empty() && last_line.checksum.is_none() && last_line.comment.is_none() {
                        None
                    } else {
                        Some(last_line)
                    },
                    end_percent: true,
                    span: Span(left, right)
                }
            }
            / left:position!() lines:(a:line() b:newline() { (a, b) })* last_line:line() right:position!() {
                File {
                    start_percent: false,
                    lines,
                    last_line: if last_line.line_components.is_empty() && last_line.checksum.is_none() && last_line.comment.is_none() {
                        None
                    } else {
                        Some(last_line)
                    },
                    end_percent: false,
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
