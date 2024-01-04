use std::io::{stdin, IsTerminal};
use std::path::PathBuf;
use std::sync::mpsc::channel;

use crate::cli_args::CliArgs;
use crate::stdio_printer::StdioPrinter;
use abasic_core::{Interpreter, InterpreterState};
use colored::*;
use ctrlc;
use rustyline::{error::ReadlineError, DefaultEditor};

const HISTORY_FILENAME: &'static str = ".abasic-history.txt";

fn show_warning(message: String, line: Option<u64>) {
    let line_str = line.map(|line| format!(" IN {}", line));

    eprintln!(
        "{}: {}",
        format!("WARNING{}", line_str.unwrap_or_default()).yellow(),
        message
    );
}

fn get_history_path() -> Option<PathBuf> {
    // Note that we're using the deprecated std::env::home_dir() here, which
    // doesn't give correct paths under some environments like Cygwin and Mingw,
    // but we'll just not support those for now--the few alternatives I found on
    // crates.io seem to have a lot of dependencies and supporting those other
    // platforms isn't a high priority right now anyways.
    #[allow(deprecated)]
    if let Some(path) = std::env::home_dir() {
        if path.exists() {
            Some(path.join(HISTORY_FILENAME))
        } else {
            None
        }
    } else {
        None
    }
}

pub struct StdioInterpreter {
    args: CliArgs,
    printer: StdioPrinter,
    interpreter: Interpreter,
}

impl StdioInterpreter {
    pub fn new(args: CliArgs) -> Self {
        let interpreter = Interpreter::new(if args.warnings {
            show_warning
        } else {
            |_message, _line| {}
        });
        StdioInterpreter {
            args,
            printer: StdioPrinter::new(),
            interpreter,
        }
    }

    fn break_interpreter(&mut self) -> Result<(), i32> {
        // TODO: Applesoft BASIC actually lets the user use "CONT" to resume
        // program execution after a break or "STOP" statement, it'd be nice
        // to support that. Instead, we're currently just stopping the program
        // and preventing it from being resumed.
        if let Some(line_number) = self.interpreter.stop_evaluating() {
            self.printer.eprintln(format!("BREAK IN {}", line_number));
        } else {
            self.printer.eprintln("BREAK");
        }
        if !self.args.is_interactive() {
            return Err(1);
        }
        Ok(())
    }

    fn load_source_file(&mut self, filename: &String) -> Result<(), i32> {
        let Ok(code) = std::fs::read_to_string(filename) else {
            println!("ERROR READING FILE: {}", filename);
            return Err(1);
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
            if let Err(err) = self.interpreter.start_evaluating(line) {
                println!("{}", err);
                return Err(1);
            }
        }
        Ok(())
    }

    pub fn run(&mut self) -> i32 {
        let Ok(mut rl) = DefaultEditor::new() else {
            eprintln!("Initializing DefaultEditor failed!");
            return 1;
        };

        let history_path = get_history_path();

        // Note that we're ignoring the result here, which is generally OK--if it
        // errors, it's probably because the file doesn't exist, and even then
        // history is optional anyways.
        history_path.clone().map(|path| rl.load_history(&path));

        let run_result = self.run_impl(&mut rl);

        // Again, we're ignoring the result here, see above for rationale.
        history_path.map(|path| rl.save_history(&path));

        match run_result {
            Ok(_) => 0,
            Err(exit_code) => exit_code,
        }
    }

    fn run_impl(&mut self, rl: &mut DefaultEditor) -> Result<(), i32> {
        let (tx, rx) = channel();

        ctrlc::set_handler(move || tx.send(()).expect("Could not send signal on channel."))
            .expect("Error setting Ctrl-C handler.");

        let mut initial_command = None;

        if let Some(filename) = &self.args.source_filename.clone() {
            self.load_source_file(&filename)?;
            initial_command = Some("RUN");
        }

        if self.args.is_interactive() {
            println!(
                "Welcome to Atul's BASIC Interpreter v{}.",
                env!("CARGO_PKG_VERSION")
            );
            println!("Press CTRL-C to exit.");
        }

        loop {
            self.printer.print_buffered_output();
            let add_to_history = initial_command.is_none() && self.args.is_interactive();
            let readline = if let Some(command) = initial_command.take() {
                Ok(command.to_string())
            } else if self.args.is_interactive() {
                rl.readline("] ")
            } else {
                return Ok(());
            };
            match readline {
                Ok(line) => {
                    if add_to_history {
                        if let Err(err) = rl.add_history_entry(line.as_str()) {
                            eprintln!("WARNING: Failed to add history entry (${:?}).", err);
                        }
                    }
                    loop {
                        let result = match self.interpreter.get_state() {
                            InterpreterState::Idle => self.interpreter.start_evaluating(&line),
                            InterpreterState::Running => self.interpreter.continue_evaluating(),
                            InterpreterState::AwaitingInput => {
                                panic!(
                                    "We should never be in this state at the beginning of the loop"
                                )
                            }
                        };

                        // Regardless of whether an error occurred, show any buffered output.
                        if let Some(output) = self.interpreter.get_and_clear_output_buffer() {
                            self.printer.print(output);
                        }

                        if rx.try_recv().is_ok() {
                            self.break_interpreter()?;
                            break;
                        }

                        if let Err(err) = result {
                            self.printer.eprintln(err.to_string());
                            if self.args.is_interactive() && stdin().is_terminal() {
                                break;
                            } else {
                                // If we're not interactive, treat errors as fatal.
                                return Err(1);
                            }
                        }

                        match self.interpreter.get_state() {
                            InterpreterState::Idle => break,
                            InterpreterState::Running => {}
                            InterpreterState::AwaitingInput => {
                                let prompt = format!("{}? ", self.printer.pop_buffered_output());
                                let readline = rl.readline(&prompt);
                                match readline {
                                    Ok(line) => {
                                        self.interpreter.provide_input(line);
                                    }
                                    Err(ReadlineError::Interrupted) => {
                                        self.break_interpreter()?;
                                        break;
                                    }
                                    Err(ReadlineError::Eof) => {
                                        return Ok(());
                                    }
                                    Err(err) => {
                                        eprintln!("Error: {:?}", err);
                                        return Err(1);
                                    }
                                }
                            }
                        }
                    }
                }
                Err(ReadlineError::Interrupted) => {
                    self.printer.eprintln("CTRL-C pressed, exiting.");
                    break;
                }
                Err(ReadlineError::Eof) => {
                    break;
                }
                Err(err) => {
                    eprintln!("Error: {:?}", err);
                    return Err(1);
                }
            }
        }

        Ok(())
    }
}
