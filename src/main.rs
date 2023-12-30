mod interpreter;
mod interpreter_error;
mod line_cruncher;
mod program;
mod syntax_error;
mod tokenizer;

use std::io::{stdin, IsTerminal};

use interpreter::{Interpreter, InterpreterState};
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
                loop {
                    let result = match interpreter.get_state() {
                        InterpreterState::Idle => interpreter.start_evaluating(&line),
                        InterpreterState::Running => interpreter.continue_evaluating(),
                    };

                    // Regardless of whether an error occurred, show any buffered output.
                    if let Some(output) = interpreter.get_and_clear_output_buffer() {
                        print!("{}", output);
                    }

                    if let Err(err) = result {
                        println!("{}", err);
                        if stdin().is_terminal() {
                            break;
                        } else {
                            // If we're not interactive, treat errors as fatal.
                            return 1;
                        }
                    }

                    match interpreter.get_state() {
                        InterpreterState::Idle => break,
                        InterpreterState::Running => {}
                    }
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
