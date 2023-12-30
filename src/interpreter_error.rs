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
    UndefinedStatement,
    OutOfMemory(OutOfMemoryError),
    ReturnWithoutGosub,
    NextWithoutFor,
}

#[derive(Debug, PartialEq)]
pub enum OutOfMemoryError {
    StackOverflow,
}

impl TracedInterpreterError {
    pub fn set_line_number(&mut self, line_number: u64) {
        self.line_number = Some(line_number);
    }
}

impl From<SyntaxError> for TracedInterpreterError {
    fn from(value: SyntaxError) -> Self {
        TracedInterpreterError {
            error: InterpreterError::Syntax(value),
            line_number: None,
            backtrace: Backtrace::capture(),
        }
    }
}

impl From<OutOfMemoryError> for TracedInterpreterError {
    fn from(value: OutOfMemoryError) -> Self {
        TracedInterpreterError {
            error: InterpreterError::OutOfMemory(value),
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
