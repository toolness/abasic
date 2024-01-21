use std::io::{stdin, IsTerminal};
use std::path::PathBuf;
use std::sync::mpsc::channel;

use crate::cli_args::CliArgs;
use crate::stdio_printer::StdioPrinter;
use abasic_core::{
    Interpreter, InterpreterOutput, InterpreterState, SourceFileAnalyzer, TracedInterpreterError,
};
use colored::*;
use ctrlc;
use rustyline::{error::ReadlineError, DefaultEditor};

const HISTORY_FILENAME: &'static str = ".abasic-history.txt";

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
        let interpreter = args.create_interpreter();
        StdioInterpreter {
            args,
            printer: StdioPrinter::new(),
            interpreter,
        }
    }

    fn show_interpreter_output(&mut self) {
        for output in self.interpreter.take_output() {
            match output {
                InterpreterOutput::Print(string) => {
                    self.printer.print(string);
                }
                InterpreterOutput::Trace(line) => {
                    self.printer.print(format!("#{} ", line).blue().to_string());
                }
                _ => {
                    self.printer.eprintln(output.to_string().yellow());
                }
            }
        }
    }

    fn break_interpreter(&mut self) -> Result<(), i32> {
        self.interpreter.break_at_current_location();
        self.show_interpreter_output();
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
        let mut analyzer = SourceFileAnalyzer::analyze(code);
        let messages = analyzer.take_messages();
        let lines = analyzer.take_source_file_lines();
        self.interpreter = analyzer.into_interpreter();
        let mut errored = false;
        for message in messages {
            match message {
                abasic_core::DiagnosticMessage::Warning(file_line_number, _, message) => {
                    self.printer.eprintln(
                        format!(
                            "Warning on line {} of '{}': {}",
                            file_line_number + 1,
                            filename,
                            message
                        )
                        .yellow(),
                    );
                }
                abasic_core::DiagnosticMessage::Error(line_number, err) => {
                    if !errored {
                        self.printer.eprintln(format!(
                            "Errors were encountered when analyzing '{filename}':"
                        ));
                    }
                    errored = true;
                    let line = Some(lines[line_number].clone());
                    self.show_error(err, line);
                }
            }
        }
        if errored {
            self.printer
                .eprintln("Please fix the above errors before running the program again.");
            Err(1)
        } else {
            Ok(())
        }
    }

    fn show_error<T: AsRef<str>>(&mut self, err: TracedInterpreterError, line: Option<T>) {
        self.printer.eprintln(err.to_string().red());
        for line in err.get_line_with_pointer_caret(&self.interpreter, line) {
            self.printer.eprintln(format!("| {line}").dimmed());
        }
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
            let mut last_line: Option<String> = None;
            let result = match self.interpreter.get_state() {
                InterpreterState::Idle => {
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
                            let result = self.interpreter.start_evaluating(&line);
                            last_line = Some(line);
                            result
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
                InterpreterState::Running => self.interpreter.continue_evaluating(),
                InterpreterState::AwaitingInput => {
                    let prompt = format!("{}? ", self.printer.pop_buffered_output());
                    let readline = rl.readline(&prompt);
                    match readline {
                        Ok(line) => {
                            self.interpreter.provide_input(line);
                            Ok(())
                        }
                        Err(ReadlineError::Interrupted) => {
                            self.break_interpreter()?;
                            Ok(())
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
                InterpreterState::NewInterpreterRequested => {
                    self.interpreter = self.args.create_interpreter();
                    Ok(())
                }
            };

            // Regardless of whether an error occurred, show any buffered output.
            self.show_interpreter_output();

            if let Err(err) = result {
                self.show_error(err, last_line);
                if !(self.args.is_interactive() && stdin().is_terminal()) {
                    // If we're not interactive, treat errors as fatal.
                    return Err(1);
                }
            }

            if rx.try_recv().is_ok() {
                self.break_interpreter()?;
            }
        }

        Ok(())
    }
}
