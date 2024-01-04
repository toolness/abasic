use clap::Parser;

#[derive(Parser)]
#[command(author, version, about, long_about = None)]
pub struct CliArgs {
    /// BASIC source file to execute.
    pub source_filename: Option<String>,

    /// Enter interactive mode after running source file.
    #[arg(short, long)]
    interactive: bool,

    /// Enable warnings (e.g. use of undeclared variables).
    #[arg(short, long)]
    pub warnings: bool,
}

impl CliArgs {
    pub fn is_interactive(&self) -> bool {
        self.source_filename.is_none() || self.interactive
    }
}
