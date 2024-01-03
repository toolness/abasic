mod builtins;
mod data;
mod dim;
mod interpreter;
mod interpreter_error;
mod line_cruncher;
mod operators;
mod program;
mod stdio_printer;
mod syntax_error;
mod tokenizer;
mod value;

use std::io::{stdin, IsTerminal};
use std::sync::mpsc::channel;

use ctrlc;
use interpreter::{Interpreter, InterpreterState};
use rustyline::{error::ReadlineError, DefaultEditor};
use stdio_printer::StdioPrinter;

const HISTORY_FILENAME: &'static str = ".interpreter-history.txt";

fn break_interpreter(printer: &mut StdioPrinter, interpreter: &mut Interpreter) {
    // TODO: Applesoft BASIC actually lets the user use "CONT" to resume
    // program execution after a break or "STOP" statement, it'd be nice
    // to support that. Instead, we're currently just stopping the program
    // and preventing it from being resumed.
    if let Some(line_number) = interpreter.stop_evaluating() {
        printer.eprintln(format!("BREAK IN {}", line_number));
    } else {
        printer.eprintln("BREAK");
    }
}

fn load_source_file(interpreter: &mut Interpreter, filename: &String) -> Result<(), ()> {
    let Ok(code) = std::fs::read_to_string(filename) else {
        println!("ERROR READING FILE: {}", filename);
        return Err(());
    };
    let lines = code.split('\n');
    for (i, line) in lines.enumerate() {
        let Some(first_char) = line.chars().next() else {
            continue;
        };
        if !first_char.is_ascii_digit() {
            eprintln!(
                "WARNING: Line {} of '{}' is not a numbered line, ignoring it.",
                i + 1,
                filename
            );
            continue;
        }
        if let Err(err) = interpreter.start_evaluating(line) {
            println!("{}", err);
            return Err(());
        }
    }
    Ok(())
}

fn run_interpreter(source_filename: Option<String>) -> i32 {
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

    let mut initial_command = None;

    if let Some(filename) = source_filename {
        if let Err(_) = load_source_file(&mut interpreter, &filename) {
            return 1;
        }
        initial_command = Some("RUN");
    }

    let mut printer = StdioPrinter::new();

    loop {
        printer.print_buffered_output();
        let readline = if let Some(command) = initial_command.take() {
            Ok(command.to_string())
        } else {
            rl.readline("] ")
        };
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
                        printer.print(output);
                    }

                    if rx.try_recv().is_ok() {
                        break_interpreter(&mut printer, &mut interpreter);
                        break;
                    }

                    if let Err(err) = result {
                        printer.eprintln(err.to_string());
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
                            let prompt = format!("{}? ", printer.pop_buffered_output());
                            let readline = rl.readline(&prompt);
                            match readline {
                                Ok(line) => {
                                    interpreter.provide_input(line);
                                }
                                Err(ReadlineError::Interrupted) => {
                                    break_interpreter(&mut printer, &mut interpreter);
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
                printer.eprintln("CTRL-C pressed, exiting.");
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
    let exit_code = run_interpreter(std::env::args().nth(1));
    std::process::exit(exit_code);
}
