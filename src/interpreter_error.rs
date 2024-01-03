use std::{
    backtrace::{Backtrace, BacktraceStatus},
    error::Error,
    fmt::Display,
};

use crate::syntax_error::SyntaxError;

#[derive(Debug)]
pub struct TracedInterpreterError {
    pub error: InterpreterError,
    line_number: Option<u64>,
    backtrace: Backtrace,
}

#[derive(Debug, PartialEq)]
pub enum InterpreterError {
    Syntax(SyntaxError),
    TypeMismatch,
    /// Applesoft BASIC doesn't have this type, but it seems useful. It basically
    /// means we're trying to read data that can't be coerced into the type of
    /// variable we're trying to store it in. Applesoft BASIC instead classifies
    /// this kind of error as a syntax error, which seems confusing.
    DataTypeMismatch,
    UndefinedStatement,
    OutOfMemory(OutOfMemoryError),
    OutOfData,
    ReturnWithoutGosub,
    NextWithoutFor,
    BadSubscript,
    IllegalQuantity,
    Unimplemented,
    DivisionByZero,
}

impl From<SyntaxError> for InterpreterError {
    fn from(value: SyntaxError) -> Self {
        InterpreterError::Syntax(value)
    }
}

#[derive(Debug, PartialEq)]
pub enum OutOfMemoryError {
    StackOverflow,
    ArrayTooLarge,
}

impl TracedInterpreterError {
    pub fn set_line_number(&mut self, line_number: u64) {
        self.line_number = Some(line_number);
    }
}

impl From<SyntaxError> for TracedInterpreterError {
    fn from(value: SyntaxError) -> Self {
        TracedInterpreterError {
            error: value.into(),
            line_number: None,
            backtrace: Backtrace::capture(),
        }
    }
}

impl From<OutOfMemoryError> for InterpreterError {
    fn from(value: OutOfMemoryError) -> Self {
        InterpreterError::OutOfMemory(value)
    }
}

impl From<OutOfMemoryError> for TracedInterpreterError {
    fn from(value: OutOfMemoryError) -> Self {
        TracedInterpreterError {
            error: value.into(),
            line_number: None,
            backtrace: Backtrace::capture(),
        }
    }
}

impl From<InterpreterError> for TracedInterpreterError {
    fn from(value: InterpreterError) -> Self {
        TracedInterpreterError {
            error: value,
            line_number: None,
            backtrace: Backtrace::capture(),
        }
    }
}

impl Error for TracedInterpreterError {}

impl Display for TracedInterpreterError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match &self.error {
            InterpreterError::Syntax(err) => {
                write!(f, "{}", err)?;
            }
            InterpreterError::TypeMismatch => {
                write!(f, "TYPE MISMATCH")?;
            }
            InterpreterError::UndefinedStatement => {
                write!(f, "UNDEF'D STATEMENT ERROR")?;
            }
            InterpreterError::OutOfMemory(err) => {
                write!(f, "OUT OF MEMORY ERROR ({:?})", err)?;
            }
            InterpreterError::ReturnWithoutGosub => {
                write!(f, "RETURN WITHOUT GOSUB ERROR")?;
            }
            InterpreterError::NextWithoutFor => {
                write!(f, "NEXT WITHOUT FOR ERROR")?;
            }
            InterpreterError::OutOfData => {
                write!(f, "OUT OF DATA ERROR")?;
            }
            InterpreterError::DataTypeMismatch => {
                write!(f, "DATA TYPE MISMATCH")?;
            }
            InterpreterError::Unimplemented => {
                write!(f, "UNIMPLEMENTED ERROR")?;
            }
            InterpreterError::BadSubscript => {
                write!(f, "BAD SUBSCRIPT ERROR")?;
            }
            InterpreterError::IllegalQuantity => {
                write!(f, "ILLEGAL QUANTITY ERROR")?;
            }
            InterpreterError::DivisionByZero => {
                write!(f, "DIVISION BY ZERO ERROR")?;
            }
        }
        if let Some(line) = self.line_number {
            write!(f, " IN {}", line)?;
        }
        if self.backtrace.status() == BacktraceStatus::Captured {
            write!(f, "\nBacktrace:\n{}", self.backtrace)?;
        }
        Ok(())
    }
}
