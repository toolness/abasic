use std::{collections::HashMap, ops::Range};

use crate::{
    line_number_parser::parse_line_number, program::Program, string_manager::StringManager,
    tokenizer::Tokenizer, Interpreter, TracedInterpreterError,
};

use super::statement_analyzer::StatementAnalyzer;

#[derive(Debug)]
pub enum DiagnosticMessage {
    Warning(usize, String),
    Error(TracedInterpreterError, Option<String>),
}

#[derive(Default)]
struct SourceLineRanges {
    line_number_end: usize,
    token_ranges: Option<Vec<Range<usize>>>,
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
                Err(err) => self
                    .messages
                    .push(DiagnosticMessage::Error(err.into(), Some(line.clone()))),
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
                    self.messages.push(DiagnosticMessage::Error(err, None));
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
