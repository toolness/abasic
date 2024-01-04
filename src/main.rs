mod builtins;
mod cli_args;
mod data;
mod dim;
mod interpreter;
mod interpreter_error;
mod line_cruncher;
mod operators;
mod program;
mod stdio_interpreter;
mod stdio_printer;
mod syntax_error;
mod tokenizer;
mod value;

use clap::Parser;
use cli_args::CliArgs;
use stdio_interpreter::StdioInterpreter;

fn main() {
    let args = CliArgs::parse();
    let mut interpreter = StdioInterpreter::new(args);
    let exit_code = interpreter.run();
    std::process::exit(exit_code);
}
