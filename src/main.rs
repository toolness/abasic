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

use clap::Parser;
use colored::*;
use ctrlc;
use interpreter::{Interpreter, InterpreterState};
use rustyline::{error::ReadlineError, DefaultEditor};
use stdio_printer::StdioPrinter;

const HISTORY_FILENAME: &'static str = ".interpreter-history.txt";

#[derive(Parser)]
#[command(author, version, about, long_about = None)]
struct CliArgs {
    /// BASIC source file to execute.
    source_filename: Option<String>,

    /// Enter interactive mode after running source file.
    #[arg(short, long)]
    interactive: bool,

    /// Enable warnings (e.g. use of undeclared variables).
    #[arg(short, long)]
    warnings: bool,
}

impl CliArgs {
    fn is_interactive(&self) -> bool {
        self.source_filename.is_none() || self.interactive
    }
}

fn show_warning(message: String, line: Option<u64>) {
    let line_str = line.map(|line| format!(" IN {}", line));

    eprintln!(
        "{}: {}",
        format!("WARNING{}", line_str.unwrap_or_default()).yellow(),
        message
    );
}

struct StdioInterpreter {
    args: CliArgs,
    printer: StdioPrinter,
    interpreter: Interpreter,
}

impl StdioInterpreter {
    fn new(args: CliArgs) -> Self {
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

    fn run(&mut self) -> i32 {
        let Ok(mut rl) = DefaultEditor::new() else {
            eprintln!("Initializing DefaultEditor failed!");
            return 1;
        };

        // Ignore the result, if it errors it's generally b/c the file doesn't exist.
        let _ = rl.load_history(HISTORY_FILENAME);

        let run_result = self.run_impl(&mut rl);

        // Ignore the result, if we fail no biggie.
        let _ = rl.save_history(HISTORY_FILENAME);

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

fn main() {
    let args = CliArgs::parse();
    let mut interpreter = StdioInterpreter::new(args);
    let exit_code = interpreter.run();
    std::process::exit(exit_code);
}
