use crate::{program::NumberedProgramLocation, TracedInterpreterError};

/// The way we're encoding error/warning locations here is
/// pretty gnarly right now:
///
///   * We don't really have a consistent way of pointing to "a place in
///     a BASIC program". Sometimes we want to point at a line number, which
///     isn't part of the tokenization process, while other times we want to
///     point at a particular place in the tokenized form of the program,
///     while other times a tokenization error has occurred, and we need
///     to point to a particular range in a string.
///
///   * This is further complicated by the fact that we sometimes want to
///     show errors in the parsed version of the code (e.g., so the user
///     can easily see that their "NOTCOOL" variable is actually parsed as
///     "NOT COOL"), while in other contexts we want to be able to point
///     at the original source file (e.g. for use by text editors).
#[derive(Debug)]
pub enum DiagnosticMessage {
    /// The first number is the file line number, then an optional program location,
    /// then the warning message.
    Warning(usize, Option<NumberedProgramLocation>, String),
    /// The first number is the file line number, then the error that occurred.
    Error(usize, TracedInterpreterError),
}
