use std::fmt::Display;

#[derive(Debug)]
pub enum InterpreterOutput {
    Print(String),
    Break(Option<u64>),
    Warning(String, Option<u64>),
    Trace(u64),
    ExtraIgnored,
    Reenter,
}

impl InterpreterOutput {
    fn get_in_line_number_string(line: &Option<u64>) -> String {
        line.map(|line| format!(" IN {}", line)).unwrap_or_default()
    }
}

impl Display for InterpreterOutput {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            InterpreterOutput::Print(string) => string.fmt(f),
            InterpreterOutput::Warning(message, line) => {
                write!(
                    f,
                    "WARNING{}: {}",
                    InterpreterOutput::get_in_line_number_string(line),
                    message
                )
            }
            InterpreterOutput::Break(line) => {
                write!(
                    f,
                    "BREAK{}",
                    InterpreterOutput::get_in_line_number_string(line)
                )
            }
            InterpreterOutput::ExtraIgnored => write!(f, "EXTRA IGNORED"),
            InterpreterOutput::Reenter => write!(f, "REENTER"),
            InterpreterOutput::Trace(line) => write!(f, "#{}", line),
        }
    }
}
