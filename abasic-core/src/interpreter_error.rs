use std::{
    backtrace::{Backtrace, BacktraceStatus},
    error::Error,
    fmt::Display,
};

use crate::{
    program::{ProgramLine, ProgramLocation},
    syntax_error::{SyntaxError, TokenizationError},
    Interpreter,
};

#[derive(Debug)]
pub struct TracedInterpreterError {
    pub error: InterpreterError,
    pub location: Option<ProgramLocation>,
    backtrace: Backtrace,
}

impl TracedInterpreterError {
    pub fn with_location(error: InterpreterError, location: ProgramLocation) -> Self {
        TracedInterpreterError {
            error,
            location: Some(location),
            backtrace: Backtrace::capture(),
        }
    }

    /// Attempts to find the line that this error is pointing at, and, if found, returns
    /// it along with a second line containing one or more carets that, when printed
    /// below the line in a monospaced font, "points" at the part of the line that
    /// caused the error.
    ///
    /// Note that `line` is the most recent line passed to the interpreter, if any.
    /// This is used to retrieve information about tokenization errors, which aren't
    /// owned by the interpreter.
    pub fn get_line_with_pointer_caret<T: AsRef<str>>(
        &self,
        interpreter: &Interpreter,
        line: Option<T>,
    ) -> Vec<String> {
        if let Some(location) = self.location {
            let lines = interpreter.program.get_line_with_pointer_caret(location);
            if !lines.is_empty() {
                return lines;
            }
        }
        if let Some(line) = line {
            if let InterpreterError::Syntax(SyntaxError::Tokenization(tok)) = &self.error {
                let range = tok.string_range(line.as_ref().len());
                return vec![
                    line.as_ref().to_owned(),
                    format!(
                        "{}{}",
                        " ".repeat(range.start),
                        "^".repeat(range.end - range.start)
                    ),
                ];
            }
        }
        vec![]
    }
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
    RedimensionedArray,
    CannotContinue,
    IllegalDirect,
}

impl From<TokenizationError> for TracedInterpreterError {
    fn from(value: TokenizationError) -> Self {
        SyntaxError::Tokenization(value).into()
    }
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

impl Display for OutOfMemoryError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            OutOfMemoryError::StackOverflow => write!(f, "STACK OVERFLOW"),
            OutOfMemoryError::ArrayTooLarge => write!(f, "ARRAY TOO LARGE"),
        }
    }
}

impl From<SyntaxError> for TracedInterpreterError {
    fn from(value: SyntaxError) -> Self {
        TracedInterpreterError {
            error: value.into(),
            location: None,
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
            location: None,
            backtrace: Backtrace::capture(),
        }
    }
}

impl From<InterpreterError> for TracedInterpreterError {
    fn from(value: InterpreterError) -> Self {
        TracedInterpreterError {
            error: value,
            location: None,
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
                write!(f, "OUT OF MEMORY ERROR ({err})")?;
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
            InterpreterError::RedimensionedArray => {
                write!(f, "REDIM'D ARRAY ERROR")?;
            }
            InterpreterError::CannotContinue => {
                write!(f, "CAN'T CONTINUE ERROR")?;
            }
            InterpreterError::IllegalDirect => {
                write!(f, "ILLEGAL DIRECT ERROR")?;
            }
        }
        if let Some(ProgramLocation {
            line: ProgramLine::Line(line),
            ..
        }) = self.location
        {
            write!(f, " IN {}", line)?;
        }
        if self.backtrace.status() == BacktraceStatus::Captured {
            write!(f, "\nBacktrace:\n{}", self.backtrace)?;
        }
        Ok(())
    }
}
