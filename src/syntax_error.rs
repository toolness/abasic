use std::{error::Error, fmt::Display};

#[derive(Debug, PartialEq)]
pub enum SyntaxError {
    IllegalCharacter,
    UnterminatedStringLiteral,
    InvalidNumber,
    UnexpectedToken,
    UnexpectedEndOfInput,
}

impl Error for SyntaxError {}

impl Display for SyntaxError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "SYNTAX ERROR ({:?})", self)
    }
}
