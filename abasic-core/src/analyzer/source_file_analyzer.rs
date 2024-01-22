use std::ops::Range;

use crate::{
    line_number_parser::parse_line_number, program::Program, string_manager::StringManager,
    tokenizer::Tokenizer, DiagnosticMessage, Interpreter, SourceFileMap, TokenType,
};

use super::{
    source_map::SourceLineRanges,
    statement_analyzer::StatementAnalyzer,
    symbol_access::{SymbolAccessMap, SymbolAccessWarning},
};

#[derive(Default)]
pub struct SourceFileAnalyzer {
    lines: Vec<String>,
    line_tokens: Vec<Vec<(TokenType, Range<usize>)>>,
    program: Program,
    messages: Vec<DiagnosticMessage>,
    string_manager: StringManager,
    source_file_map: SourceFileMap,
    symbol_accesses: SymbolAccessMap,
}

impl SourceFileAnalyzer {
    pub fn analyze(contents: String) -> Self {
        Self::analyze_lines(
            contents
                .split('\n')
                .map(|s| s.to_owned())
                .collect::<Vec<_>>(),
        )
    }

    pub fn analyze_lines(lines: Vec<String>) -> Self {
        let mut analyzer = SourceFileAnalyzer::default();
        analyzer.run(lines);
        analyzer
    }

    pub fn messages(&self) -> &Vec<DiagnosticMessage> {
        &self.messages
    }

    pub fn source_file_lines(&self) -> &Vec<String> {
        &self.lines
    }

    pub fn source_file_map(&self) -> &SourceFileMap {
        &self.source_file_map
    }

    pub fn take_messages(&mut self) -> Vec<DiagnosticMessage> {
        std::mem::take(&mut self.messages)
    }

    pub fn take_source_file_lines(&mut self) -> Vec<String> {
        std::mem::take(&mut self.lines)
    }

    pub fn token_types(&self) -> &Vec<Vec<(TokenType, Range<usize>)>> {
        &self.line_tokens
    }

    fn warn_line<T: AsRef<str>>(&mut self, line_number: usize, message: T) {
        self.messages.push(DiagnosticMessage::Warning(
            line_number,
            None,
            message.as_ref().to_string(),
        ));
    }

    fn run(&mut self, lines: Vec<String>) {
        for (i, line) in lines.iter().enumerate() {
            if line.is_empty() {
                self.source_file_map.add_empty();
                self.line_tokens.push(vec![]);
                continue;
            }
            let Some((basic_line_number, line_number_end)) = parse_line_number(line) else {
                self.source_file_map.add_empty();
                self.line_tokens.push(vec![]);
                self.warn_line(i, "Line has no line number, ignoring it.");
                continue;
            };
            let mut source_line_ranges = SourceLineRanges {
                line_number_end,
                length: line.len(),
                ..Default::default()
            };
            let mut line_tokens: Vec<(TokenType, Range<usize>)> =
                vec![(TokenType::Number, 0..line_number_end)];
            if self.program.has_line_number(basic_line_number) {
                self.warn_line(i, "Redefinition of pre-existing BASIC line.");
            }
            let tokenize_result = Tokenizer::new(line, &mut self.string_manager)
                .skip_bytes(line_number_end)
                .remaining_tokens_and_ranges();
            match tokenize_result {
                Ok((tokens, token_ranges)) => {
                    for (token, range) in tokens.iter().zip(&token_ranges) {
                        line_tokens.push((token.into(), range.clone()));
                    }
                    source_line_ranges.token_ranges = Some(token_ranges);
                    if tokens.is_empty() {
                        self.warn_line(i, "Line contains no statements and will not be defined.");
                    } else {
                        self.program.set_numbered_line(basic_line_number, tokens);
                    }
                }
                Err(err) => self.messages.push(DiagnosticMessage::Error(i, err.into())),
            }
            self.source_file_map
                .add(basic_line_number, source_line_ranges);
            self.line_tokens.push(line_tokens);
        }
        self.lines = lines;
        self.program.run_from_first_numbered_line();
        loop {
            while self.program.has_next_token() {
                let result = StatementAnalyzer::new(&mut self.program, &mut self.symbol_accesses)
                    .evaluate_statement();
                if let Err(mut err) = result {
                    self.program.populate_error_location(&mut err);
                    let Some((file_line_number, _)) = self
                        .source_file_map()
                        .map_location_to_source(&err.location.unwrap())
                    else {
                        panic!("Expected error to have a numbered program line: {:?}", err);
                    };
                    self.messages
                        .push(DiagnosticMessage::Error(file_line_number, err));
                    break;
                }
            }
            if !self.program.next_line() {
                break;
            }
        }
        self.populate_symbol_access_warnings();
    }

    fn populate_symbol_access_warnings(&mut self) {
        for (warning, symbol, location) in self.symbol_accesses.get_warnings() {
            let message = match warning {
                SymbolAccessWarning::UndefinedSymbol => format!("'{symbol}' is never defined"),
                SymbolAccessWarning::UnusedSymbol => format!("'{symbol}' is never used"),
            };
            let source_line = self
                .source_file_map
                .map_location_to_source(&location.into())
                .unwrap()
                .0;
            self.messages.push(DiagnosticMessage::Warning(
                source_line,
                Some(location),
                message,
            ));
        }
    }

    pub fn into_interpreter(mut self) -> Interpreter {
        self.program.reset_runtime_state();
        Interpreter::from_program(self.program, self.string_manager)
    }
}
