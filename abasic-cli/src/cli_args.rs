use std::time::SystemTime;

use abasic_core::Interpreter;
use clap::Parser;

#[derive(Parser)]
#[command(author, version, about, long_about = None)]
pub struct CliArgs {
    /// BASIC source file to execute.
    pub source_filename: Option<String>,

    /// Enter interactive mode after running source file.
    #[arg(short, long)]
    interactive: bool,

    /// Enable runtime warnings (e.g. use of undeclared variables).
    #[arg(short, long)]
    pub warnings: bool,

    /// Skip static analysis of input files.
    #[arg(short, long)]
    pub skip_check: bool,

    /// Enable line number tracing
    #[arg(short, long)]
    pub tracing: bool,
}

impl CliArgs {
    pub fn is_interactive(&self) -> bool {
        self.source_filename.is_none() || self.interactive
    }

    pub fn create_interpreter(&self) -> Interpreter {
        let mut interpreter = Interpreter::default();
        interpreter.enable_warnings = self.warnings;
        interpreter.enable_tracing = self.tracing;

        let now = SystemTime::now();
        let seed = now.elapsed().unwrap().as_millis() as u64;
        interpreter.randomize(seed);

        interpreter
    }
}
