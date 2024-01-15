use std::{error::Error, fmt::Display, ops::Range};

use crate::tokenizer::Token;

#[derive(Debug, PartialEq)]
pub enum SyntaxError {
    /// The argument is the string index of the illegal character.
    IllegalCharacter(usize),
    /// The argument is the string index of the string's opening quote.
    UnterminatedStringLiteral(usize),
    /// The argument is the span (as string indices) that represents an invalid number.
    InvalidNumber(Range<usize>),
    UnexpectedToken,
    ExpectedToken(Token),
    UnexpectedEndOfInput,
}

impl SyntaxError {
    pub fn string_range(&self, string: &str) -> Option<Range<usize>> {
        match &self {
            SyntaxError::IllegalCharacter(i) => Some(*i..*i + 1),
            SyntaxError::UnterminatedStringLiteral(i) => Some(*i..string.len()),
            SyntaxError::InvalidNumber(range) => Some(range.clone()),
            _ => None,
        }
    }
}

impl Error for SyntaxError {}

impl Display for SyntaxError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "SYNTAX ERROR ({:?})", self)
    }
}
