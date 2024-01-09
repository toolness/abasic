mod builtins;
mod data;
mod dim;
mod interpreter;
mod interpreter_error;
mod line_cruncher;
mod line_number_parser;
mod operators;
mod program;
mod program_lines;
mod syntax_error;
mod tokenizer;
mod value;

pub use interpreter::{Interpreter, InterpreterOutput, InterpreterState};
pub use interpreter_error::{InterpreterError, TracedInterpreterError};
pub use line_number_parser::parse_line_number;

/// Set the seed for the random number generator, used by the
/// interpreter's `RND` function.
pub fn set_rnd_seed(seed: u64) {
    fastrand::seed(seed)
}
