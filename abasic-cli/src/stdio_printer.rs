use std::{fmt::Display, io::Write};

/// If we don't get a newline for these many characters, flush the output
/// to stdout.
const MAX_BUFFER_SIZE: usize = 255;

/// This is a weird class that buffers lines internally, which gives us
/// control over how we output buffered data.  We need it in part because
/// rustyline appears to overwrite any content on the current line that
/// it's prompting, which requires us to pass buffered output to it
/// so prompts work as expected when running BASIC programs.
pub struct StdioPrinter {
    line_buffer: String,
}

impl StdioPrinter {
    pub fn new() -> Self {
        StdioPrinter {
            line_buffer: String::with_capacity(MAX_BUFFER_SIZE),
        }
    }

    fn flush_line_buffer(&mut self) {
        std::io::stdout()
            .write(self.line_buffer.as_bytes())
            .unwrap();
        self.line_buffer.clear();
    }

    /// Returns any buffered output that hasn't yet been printed.
    pub fn pop_buffered_output(&mut self) -> String {
        std::mem::replace(
            &mut self.line_buffer,
            String::with_capacity(MAX_BUFFER_SIZE),
        )
    }

    /// Print out any buffered output followed by a newline.
    pub fn print_buffered_output(&mut self) {
        if !self.line_buffer.is_empty() {
            self.line_buffer.push('\n');
            self.flush_line_buffer();
        }
    }

    /// Print the given string to stdout in a line-buffered way.
    pub fn print(&mut self, value: String) {
        for ch in value.chars() {
            self.line_buffer.push(ch);
            if ch == '\n' || self.line_buffer.len() == MAX_BUFFER_SIZE {
                self.flush_line_buffer();
            }
        }
    }

    /// Print any buffered output, then write the given string to stderr
    /// followed by a newline.
    ///
    /// This ensures that users see any partially printed output before
    /// error output in BASIC programs.
    pub fn eprintln<T: Display>(&mut self, value: T) {
        self.print_buffered_output();
        eprintln!("{}", value);
    }
}
