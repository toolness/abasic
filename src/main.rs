mod data;
mod interpreter;
mod interpreter_error;
mod line_cruncher;
mod operators;
mod program;
mod syntax_error;
mod tokenizer;
mod value;

use std::io::{stdin, IsTerminal};
use std::sync::mpsc::channel;

use ctrlc;
use interpreter::{Interpreter, InterpreterState};
use rustyline::{error::ReadlineError, DefaultEditor};

const HISTORY_FILENAME: &'static str = ".interpreter-history.txt";

fn break_interpreter(interpreter: &mut Interpreter) {
    // TODO: Applesoft BASIC actually lets the user use "CONT" to resume
    // program execution after a break or "STOP" statement, it'd be nice
    // to support that. Instead, we're currently just stopping the program
    // and preventing it from being resumed.
    if let Some(line_number) = interpreter.stop_evaluating() {
        println!("BREAK IN {}", line_number);
    } else {
        println!("BREAK");
    }
}

fn run_interpreter() -> i32 {
    let (tx, rx) = channel();

    ctrlc::set_handler(move || tx.send(()).expect("Could not send signal on channel."))
        .expect("Error setting Ctrl-C handler.");

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
                        InterpreterState::AwaitingInput => {
                            panic!("We should never be in this state at the beginning of the loop")
                        }
                    };

                    // Regardless of whether an error occurred, show any buffered output.
                    if let Some(output) = interpreter.get_and_clear_output_buffer() {
                        print!("{}", output);
                    }

                    if rx.try_recv().is_ok() {
                        break_interpreter(&mut interpreter);
                        break;
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
                        InterpreterState::AwaitingInput => {
                            let readline = rl.readline("? ");
                            match readline {
                                Ok(line) => {
                                    interpreter.provide_input(line);
                                }
                                Err(ReadlineError::Interrupted) => {
                                    break_interpreter(&mut interpreter);
                                    break;
                                }
                                Err(ReadlineError::Eof) => {
                                    return 0;
                                }
                                Err(err) => {
                                    eprintln!("Error: {:?}", err);
                                    return 1;
                                }
                            }
                        }
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
