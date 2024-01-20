use std::{collections::HashMap, ops::Range};

use crate::{
    line_number_parser::parse_line_number,
    program::{Program, ProgramLine},
    string_manager::StringManager,
    tokenizer::Tokenizer,
    Interpreter, InterpreterError, SyntaxError, TracedInterpreterError,
};

use super::statement_analyzer::StatementAnalyzer;

#[derive(Debug)]
pub enum DiagnosticMessage {
    Warning(usize, String),
    Error(TracedInterpreterError, Option<String>, Option<usize>),
}

#[derive(Default)]
struct SourceLineRanges {
    line_number_end: usize,
    token_ranges: Option<Vec<Range<usize>>>,
    length: usize,
}

#[derive(Default)]
struct SourceFileMap {
    basic_lines_to_file_lines: HashMap<u64, usize>,
    file_line_ranges: Vec<SourceLineRanges>,
}

impl SourceFileMap {
    fn add_empty(&mut self) {
        self.file_line_ranges.push(SourceLineRanges::default());
    }

    fn add(&mut self, basic_line: u64, ranges: SourceLineRanges) {
        let file_line_number = self.file_line_ranges.len();
        self.basic_lines_to_file_lines
            .insert(basic_line, file_line_number);
        self.file_line_ranges.push(ranges);
    }

    fn map_to_source(&self, message: &DiagnosticMessage) -> Option<(usize, Range<usize>)> {
        match message {
            DiagnosticMessage::Warning(file_line_number, _) => {
                let source_line_ranges = &self.file_line_ranges[*file_line_number];
                Some((*file_line_number, 0..source_line_ranges.line_number_end))
            }
            DiagnosticMessage::Error(err, _, file_line_number) => {
                if let Some(file_line_number) = file_line_number {
                    match &err.error {
                        InterpreterError::Syntax(SyntaxError::Tokenization(t)) => {
                            let range =
                                t.string_range(self.file_line_ranges[*file_line_number].length);
                            return Some((*file_line_number, range));
                        }
                        _ => {}
                    }
                }
                if let Some(location) = err.location {
                    if let ProgramLine::Line(basic_line_number) = location.line {
                        if let Some(&file_line_number) =
                            self.basic_lines_to_file_lines.get(&basic_line_number)
                        {
                            let source_line_ranges = &self.file_line_ranges[file_line_number];
                            if let Some(token_ranges) = &source_line_ranges.token_ranges {
                                if let Some(range) = token_ranges.get(location.token_index) {
                                    return Some((file_line_number, range.clone()));
                                }
                            }
                        }
                    }
                }
                None
            }
        }
    }
}

#[derive(Default)]
pub struct SourceFileAnalyzer {
    lines: Vec<String>,
    program: Program,
    messages: Vec<DiagnosticMessage>,
    string_manager: StringManager,
    source_file_map: SourceFileMap,
}

impl SourceFileAnalyzer {
    pub fn analyze(contents: String) -> Self {
        let mut analyzer = SourceFileAnalyzer::default();
        analyzer.run(contents);
        analyzer
    }

    pub fn take_messages(&mut self) -> Vec<DiagnosticMessage> {
        std::mem::take(&mut self.messages)
    }

    fn warn<T: AsRef<str>>(&mut self, line_number: usize, message: T) {
        self.messages.push(DiagnosticMessage::Warning(
            line_number,
            message.as_ref().to_string(),
        ));
    }

    fn run(&mut self, contents: String) {
        let lines = contents
            .split('\n')
            .map(|s| s.to_owned())
            .collect::<Vec<_>>();
        for (i, line) in lines.iter().enumerate() {
            if line.is_empty() {
                self.source_file_map.add_empty();
                continue;
            }
            let Some((basic_line_number, line_number_end)) = parse_line_number(line) else {
                self.source_file_map.add_empty();
                self.warn(i, "Line has no line number, ignoring it.");
                continue;
            };
            let mut source_line_ranges = SourceLineRanges {
                line_number_end,
                length: line.len(),
                ..Default::default()
            };
            if self.program.has_line_number(basic_line_number) {
                self.warn(i, "Redefinition of pre-existing BASIC line.");
            }
            let tokenize_result = Tokenizer::new(line, &mut self.string_manager)
                .skip_bytes(line_number_end)
                .remaining_tokens_and_ranges();
            match tokenize_result {
                Ok((tokens, token_ranges)) => {
                    source_line_ranges.token_ranges = Some(token_ranges);
                    if tokens.is_empty() {
                        self.warn(i, "Line contains no statements and will not be defined.");
                    } else {
                        self.program.set_numbered_line(basic_line_number, tokens);
                    }
                }
                Err(err) => self.messages.push(DiagnosticMessage::Error(
                    err.into(),
                    Some(line.clone()),
                    Some(i),
                )),
            }
            self.source_file_map
                .add(basic_line_number, source_line_ranges);
        }
        self.lines = lines;
        self.program.run_from_first_numbered_line();
        loop {
            while self.program.has_next_token() {
                let result = StatementAnalyzer::new(&mut self.program).evaluate_statement();
                if let Err(mut err) = result {
                    self.program.populate_error_location(&mut err);
                    self.messages
                        .push(DiagnosticMessage::Error(err, None, None));
                    break;
                }
            }
            if !self.program.next_line() {
                break;
            }
        }
    }

    pub fn into_interpreter(mut self) -> Interpreter {
        self.program.reset_runtime_state();
        Interpreter::from_program(self.program, self.string_manager)
    }
}
