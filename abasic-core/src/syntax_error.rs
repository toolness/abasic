use std::{error::Error, fmt::Display, ops::Range};

use crate::tokenizer::Token;

#[derive(Debug, PartialEq)]
pub enum TokenizationError {
    /// The argument is the string index of the illegal character.
    IllegalCharacter(usize),
    /// The argument is the string index of the string's opening quote.
    UnterminatedStringLiteral(usize),
    /// The argument is the span (as string indices) that represents an invalid number.
    InvalidNumber(Range<usize>),
}

impl TokenizationError {
    pub fn string_range(&self, string: &str) -> Range<usize> {
        match &self {
            TokenizationError::IllegalCharacter(i) => *i..*i + 1,
            TokenizationError::UnterminatedStringLiteral(i) => *i..string.len(),
            TokenizationError::InvalidNumber(range) => range.clone(),
        }
    }
}

impl Display for TokenizationError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            TokenizationError::IllegalCharacter(_) => write!(f, "ILLEGAL CHARACTER"),
            TokenizationError::UnterminatedStringLiteral(_) => write!(f, "UNTERMINATED STRING"),
            TokenizationError::InvalidNumber(_) => write!(f, "INVALID NUMBER"),
        }
    }
}

impl From<TokenizationError> for SyntaxError {
    fn from(value: TokenizationError) -> Self {
        SyntaxError::Tokenization(value)
    }
}

#[derive(Debug, PartialEq)]
pub enum SyntaxError {
    Tokenization(TokenizationError),
    UnexpectedToken,
    ExpectedToken(Token),
    UnexpectedEndOfInput,
}

impl Error for SyntaxError {}

impl Display for SyntaxError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "SYNTAX ERROR (")?;
        match self {
            SyntaxError::Tokenization(t) => write!(f, "{t})"),
            SyntaxError::UnexpectedToken => write!(f, "UNEXPECTED TOKEN)"),
            SyntaxError::ExpectedToken(tok) => write!(f, "EXPECTED TOKEN '{tok}')"),
            SyntaxError::UnexpectedEndOfInput => write!(f, "UNEXPECTED END OF INPUT)"),
        }
    }
}
