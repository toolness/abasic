use std::{collections::HashMap, ops::Range};

use crate::{
    line_number_parser::parse_line_number,
    program::{NumberedProgramLocation, Program, ProgramLine, ProgramLocation},
    string_manager::StringManager,
    symbol::Symbol,
    tokenizer::Tokenizer,
    Interpreter, InterpreterError, SyntaxError, Token, TracedInterpreterError,
};

use super::statement_analyzer::StatementAnalyzer;

/// The way we're encoding error/warning locations here is an
/// unmitigated disaster:
///
///   * We don't really have a consistent way of pointing to "a place in
///     a BASIC program". Sometimes we want to point at a line number, which
///     isn't part of the tokenization process, while other times we want to
///     point at a particular place in the tokenized form of the program,
///     while other times a tokenization error has occurred, and we need
///     to point to a particular range in a string.
///
///   * This is further complicated by the fact that we sometimes want to
///     show errors in the parsed version of the code (e.g., so the user
///     can easily see that their "NOTCOOL" variable is actually parsed as
///     "NOT COOL"), while in other contexts we want to be able to point
///     at the original source file (e.g. for use by text editors).
#[derive(Debug)]
pub enum DiagnosticMessage {
    /// The first number is the file line number, then an optional program location,
    /// then the warning message.
    Warning(usize, Option<NumberedProgramLocation>, String),
    /// The first number is the file line number, then the error that occurred.
    Error(usize, TracedInterpreterError),
}

#[derive(Default)]
struct SourceLineRanges {
    line_number_end: usize,
    token_ranges: Option<Vec<Range<usize>>>,
    length: usize,
}

#[derive(Default)]
pub struct SourceFileMap {
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

#[derive(Debug, Copy, Clone, PartialEq)]
pub enum TokenType {
    Symbol,
    String,
    Number,
    Operator,
    Comment,
    Keyword,
    Delimiter,
    Data,
}

impl From<&Token> for TokenType {
    fn from(value: &Token) -> Self {
        match value {
            Token::Dim => TokenType::Keyword,
            Token::Let => TokenType::Keyword,
            Token::Print => TokenType::Keyword,
            Token::Input => TokenType::Keyword,
            Token::Goto => TokenType::Keyword,
            Token::Gosub => TokenType::Keyword,
            Token::Return => TokenType::Keyword,
            Token::Colon => TokenType::Delimiter,
            Token::Semicolon => TokenType::Delimiter,
            Token::Comma => TokenType::Delimiter,
            Token::QuestionMark => TokenType::Keyword,
            Token::LeftParen => TokenType::Delimiter,
            Token::RightParen => TokenType::Delimiter,
            Token::Plus => TokenType::Operator,
            Token::Minus => TokenType::Operator,
            Token::Multiply => TokenType::Operator,
            Token::Divide => TokenType::Operator,
            Token::Caret => TokenType::Operator,
            Token::Equals => TokenType::Operator,
            Token::NotEquals => TokenType::Operator,
            Token::LessThan => TokenType::Operator,
            Token::LessThanOrEqualTo => TokenType::Operator,
            Token::GreaterThan => TokenType::Operator,
            Token::GreaterThanOrEqualTo => TokenType::Operator,
            Token::And => TokenType::Operator,
            Token::Or => TokenType::Operator,
            Token::Not => TokenType::Operator,
            Token::If => TokenType::Keyword,
            Token::Then => TokenType::Keyword,
            Token::Else => TokenType::Keyword,
            Token::End => TokenType::Keyword,
            Token::Stop => TokenType::Keyword,
            Token::For => TokenType::Keyword,
            Token::To => TokenType::Keyword,
            Token::Step => TokenType::Keyword,
            Token::Next => TokenType::Keyword,
            Token::Read => TokenType::Keyword,
            Token::Restore => TokenType::Keyword,
            Token::Def => TokenType::Keyword,
            Token::Remark(_) => TokenType::Comment,
            Token::Symbol(_) => TokenType::Symbol,
            Token::StringLiteral(_) => TokenType::String,
            Token::NumericLiteral(_) => TokenType::Number,
            Token::Data(_) => TokenType::Data,
        }
    }
}

pub enum SymbolAccess {
    Read,
    Write,
}

pub struct SymbolAccessLocation(SymbolAccess, NumberedProgramLocation);

#[derive(Default)]
pub struct SymbolAccessMap(HashMap<Symbol, Vec<SymbolAccessLocation>>);

impl SymbolAccessMap {
    pub fn log_access(
        &mut self,
        symbol: &Symbol,
        location: &ProgramLocation,
        access: SymbolAccess,
    ) {
        let entry = self.0.entry(symbol.clone()).or_default();
        entry.push(SymbolAccessLocation(
            access,
            (*location).try_into().unwrap(),
        ));
    }
}

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

        for message in &self.messages {
            // TODO: We're only doing this so we don't get dead code
            // errors, and so we can verify that this code doesn't
            // panic. But we should actually test it separately.
            self.source_file_map.map_to_source(message);
        }
    }

    pub fn into_interpreter(mut self) -> Interpreter {
        self.program.reset_runtime_state();
        Interpreter::from_program(self.program, self.string_manager)
    }
}
