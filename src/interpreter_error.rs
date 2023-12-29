use std::{
    backtrace::{Backtrace, BacktraceStatus},
    error::Error,
    fmt::Display,
};

use crate::syntax_error::SyntaxError;

#[derive(Debug)]
pub struct TracedInterpreterError {
    pub error: InterpreterError,
    // TODO: Add line number so we can show that when errors occur!
    backtrace: Backtrace,
}

#[derive(Debug, PartialEq)]
pub enum InterpreterError {
    SyntaxError(SyntaxError),
    TypeMismatch,
}

impl TracedInterpreterError {
    pub fn unexpected_token<T>() -> Result<T, TracedInterpreterError> {
        Err(SyntaxError::UnexpectedToken.into())
    }
}

impl From<SyntaxError> for TracedInterpreterError {
    fn from(value: SyntaxError) -> Self {
        TracedInterpreterError {
            error: InterpreterError::SyntaxError(value),
            backtrace: Backtrace::capture(),
        }
    }
}

impl From<InterpreterError> for TracedInterpreterError {
    fn from(value: InterpreterError) -> Self {
        TracedInterpreterError {
            error: value,
            backtrace: Backtrace::capture(),
        }
    }
}

impl Error for TracedInterpreterError {}

impl Display for TracedInterpreterError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match &self.error {
            InterpreterError::SyntaxError(err) => {
                write!(f, "{}", err)?;
            }
            InterpreterError::TypeMismatch => {
                write!(f, "TYPE MISMATCH")?;
            }
        }
        if self.backtrace.status() == BacktraceStatus::Captured {
            write!(f, "\nBacktrace:\n{}", self.backtrace)?;
        }
        Ok(())
    }
}
