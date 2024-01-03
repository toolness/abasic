use std::io::Write;

/// This is a weird class that buffers lines internally, which gives us
/// control over how we output buffered data.  We need it in part because
/// rustyline appears to overwrite any content on the current line that
/// it's prompting, which requires us to pass buffered output to it
/// so prompts work as expected when running BASIC programs.
#[derive(Default)]
pub struct StdioPrinter {
    line_buffer: String,
}

impl StdioPrinter {
    fn flush_line_buffer(&mut self) {
        std::io::stdout()
            .write(self.line_buffer.as_bytes())
            .unwrap();
        self.line_buffer.clear();
    }

    /// Returns any buffered output that hasn't yet been printed.
    pub fn pop_buffered_output(&mut self) -> String {
        std::mem::replace(&mut self.line_buffer, String::new())
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
            if ch == '\n' {
                self.flush_line_buffer();
            }
        }
    }

    /// Print any buffered output, then write the given string to stderr
    /// followed by a newline.
    ///
    /// This ensures that users see any partially printed output before
    /// error output in BASIC programs.
    pub fn eprintln<T: AsRef<str>>(&mut self, value: T) {
        self.print_buffered_output();
        std::io::stderr().write(value.as_ref().as_bytes()).unwrap();
        std::io::stderr().write("\n".as_bytes()).unwrap();
    }
}
