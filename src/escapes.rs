//! Escape sequences
//!
//! Thanks, stack overflow!
//!
//! (modifications were made)

use std::fmt::Display;

/// Escape error
#[derive(Debug, PartialEq)]
pub enum EscapeError {
    /// there's an escape at the end of the string
    EscapeAtEndOfString,
    /// unknown unicode character in a \u escape
    InvalidUnicodeChar(char),
    /// invalid unicode codepoint
    InvalidUnicodeCodepoint(u32),
}

impl Display for EscapeError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            EscapeError::EscapeAtEndOfString => f.write_str("escape at end of statement"),
            EscapeError::InvalidUnicodeChar(c) => f.write_fmt(format_args!(
                "invalid character in a unicode escape: {}",
                *c
            )),
            EscapeError::InvalidUnicodeCodepoint(c) => {
                f.write_fmt(format_args!("invalid unicode codepoint in escape: {}", *c))
            }
        }
    }
}

/// iterator
struct InterpretEscapedString<'a> {
    /// chars
    s: std::str::Chars<'a>,
}

impl<'a> Iterator for InterpretEscapedString<'a> {
    type Item = Result<char, EscapeError>;

    fn next(&mut self) -> Option<Self::Item> {
        let mut ret_next = false;
        let out = self.s.next().map(|c| match c {
            '\\' => match self.s.next() {
                None => Err(EscapeError::EscapeAtEndOfString),
                Some('n') => Ok('\n'),
                Some('t') => Ok('\t'),
                Some('\\') => Ok('\\'),
                Some('"') => Ok('"'),
                Some('\'') => Ok('\''),
                Some('e') => Ok('\x1b'),
                Some('\n') => {
                    ret_next = true;
                    Err(EscapeError::EscapeAtEndOfString)
                }
                Some('u') | Some('U') | Some('x') => {
                    let code = [self.s.next(), self.s.next(), self.s.next(), self.s.next()];
                    if code.iter().any(|val| val.is_none()) {
                        return Err(EscapeError::EscapeAtEndOfString);
                    }
                    let code = TryInto::<[char; 4]>::try_into(
                        code.iter()
                            .map(|ch| ch.unwrap().to_ascii_lowercase())
                            .collect::<Vec<char>>(),
                    )
                    .unwrap();

                    for c in code {
                        if !(c.is_numeric() || ['a', 'b', 'c', 'd', 'e', 'f'].contains(&c)) {
                            return Err(EscapeError::InvalidUnicodeChar(c));
                        }
                    }

                    let code = u32::from_str_radix(&String::from_iter(code), 16).unwrap();
                    let out = char::from_u32(code);
                    if out.is_none() {
                        return Err(EscapeError::InvalidUnicodeCodepoint(code));
                    }
                    Ok(out.unwrap())
                }
                Some(c) => Ok(c),
            },
            c => Ok(c),
        });
        if ret_next { self.next() } else { out }
    }
}

/// interpret an escaped string
pub fn interpret_escaped_string(s: &str) -> Result<String, EscapeError> {
    (InterpretEscapedString { s: s.chars() }).collect()
}
