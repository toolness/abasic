mod interpreter;
mod syntax_error;
mod tokenizer;

use std::io::{stdin, IsTerminal};

use interpreter::{Interpreter, InterpreterError};
use rustyline::{error::ReadlineError, DefaultEditor};

const HISTORY_FILENAME: &'static str = ".interpreter-history.txt";

fn run_interpreter() -> i32 {
    let Ok(mut rl) = DefaultEditor::new() else {
        eprintln!("Initializing DefaultEditor failed!");
        return 1;
    };

    // Ignore the result, if it errors it's generally b/c the file doesn't exist.
    let _ = rl.load_history(HISTORY_FILENAME);

    let mut interpreter = Interpreter::new();

    loop {
        let readline = rl.readline("] ");
        match readline {
            Ok(line) => {
                if let Err(err) = rl.add_history_entry(line.as_str()) {
                    eprintln!("WARNING: Failed to add history entry (${:?}).", err);
                }
                if let Err(err) = interpreter.evaluate(line) {
                    match err {
                        InterpreterError::SyntaxError(err) => {
                            println!("SYNTAX ERROR ({:?})", err);
                        }
                    }
                    // If we're not interactive, treat errors as fatal.
                    if !stdin().is_terminal() {
                        return 1;
                    }
                }
                if let Some(output) = interpreter.get_and_clear_output_buffer() {
                    print!("{}", output);
                }
            }
            Err(ReadlineError::Interrupted) => {
                eprintln!("CTRL-C pressed, exiting.");
                break;
            }
            Err(ReadlineError::Eof) => {
                break;
            }
            Err(err) => {
                eprintln!("Error: {:?}", err);
                return 1;
            }
        }
    }

    // Ignore the result, if we fail no biggie.
    let _ = rl.save_history(HISTORY_FILENAME);

    return 0;
}

fn main() {
    let exit_code = run_interpreter();
    std::process::exit(exit_code);
}
