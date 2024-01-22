use std::{collections::HashMap, ops::Range};

use crate::{
    program::{ProgramLine, ProgramLocation},
    DiagnosticMessage, InterpreterError, SyntaxError,
};

#[derive(Default)]
pub(crate) struct SourceLineRanges {
    pub(crate) line_number_end: usize,
    pub(crate) token_ranges: Option<Vec<Range<usize>>>,
    pub(crate) length: usize,
}

#[derive(Default)]
pub struct SourceFileMap {
    basic_lines_to_file_lines: HashMap<u64, usize>,
    file_line_ranges: Vec<SourceLineRanges>,
}

impl SourceFileMap {
    pub(crate) fn add_empty(&mut self) {
        self.file_line_ranges.push(SourceLineRanges::default());
    }

    pub(crate) fn add(&mut self, basic_line: u64, ranges: SourceLineRanges) {
        let file_line_number = self.file_line_ranges.len();
        self.basic_lines_to_file_lines
            .insert(basic_line, file_line_number);
        self.file_line_ranges.push(ranges);
    }

    pub fn map_location_to_source(
        &self,
        location: &ProgramLocation,
    ) -> Option<(usize, Range<usize>)> {
        if let ProgramLine::Line(basic_line_number) = location.line {
            if let Some(&file_line_number) = self.basic_lines_to_file_lines.get(&basic_line_number)
            {
                let source_line_ranges = &self.file_line_ranges[file_line_number];
                if let Some(token_ranges) = &source_line_ranges.token_ranges {
                    let token_index =
                        if location.token_index == token_ranges.len() && !token_ranges.is_empty() {
                            // The error is at the very end of the line, just use the very last token for now.
                            token_ranges.len() - 1
                        } else {
                            location.token_index
                        };
                    if let Some(range) = token_ranges.get(token_index) {
                        return Some((file_line_number, range.clone()));
                    }
                }
            }
        }
        None
    }

    pub fn map_to_source(&self, message: &DiagnosticMessage) -> Option<(usize, Range<usize>)> {
        match message {
            DiagnosticMessage::Warning(file_line_number, location, _) => {
                if let Some(location) = location {
                    self.map_location_to_source(&(*location).into())
                } else {
                    let source_line_ranges = &self.file_line_ranges[*file_line_number];
                    Some((*file_line_number, 0..source_line_ranges.line_number_end))
                }
            }
            DiagnosticMessage::Error(file_line_number, err) => {
                match &err.error {
                    InterpreterError::Syntax(SyntaxError::Tokenization(t)) => {
                        let range = t.string_range(self.file_line_ranges[*file_line_number].length);
                        return Some((*file_line_number, range));
                    }
                    _ => {}
                }
                if let Some(location) = err.location {
                    self.map_location_to_source(&location)
                } else {
                    None
                }
            }
        }
    }
}
