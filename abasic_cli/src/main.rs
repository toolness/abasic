mod cli_args;
mod stdio_interpreter;
mod stdio_printer;

use clap::Parser;
use cli_args::CliArgs;
use stdio_interpreter::StdioInterpreter;

fn main() {
    let args = CliArgs::parse();
    let mut interpreter = StdioInterpreter::new(args);
    let exit_code = interpreter.run();
    std::process::exit(exit_code);
}
