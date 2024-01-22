use std::ops::Range;

use abasic_core::{
    DiagnosticMessage, InterpreterError, SourceFileAnalyzer, SourceFileMap, SyntaxError, TokenType,
};

fn analyze(program: &'static str) -> SourceFileAnalyzer {
    let lines = program
        .split("\n")
        .map(|line| line.trim_start())
        .map(|s| s.to_owned());
    SourceFileAnalyzer::analyze(lines.clone().collect::<Vec<_>>().join("\n"))
}

fn assert_program_is_fine(program: &'static str) {
    let analyzer_messages = analyze(program).take_messages();
    if analyzer_messages.len() != 0 {
        panic!("Expected analyzer for program {program} to be empty but got {analyzer_messages:?}");
    }
}

#[derive(PartialEq, Debug)]
enum MessageType {
    Warning,
    Error,
}

#[derive(PartialEq, Debug)]
struct SourceMappedMessage {
    _type: MessageType,
    message: String,
    line: usize,
    source_snippet: String,
}

impl SourceMappedMessage {
    fn from_diagnostic(
        diagnostic: &DiagnosticMessage,
        map: &SourceFileMap,
        lines: &Vec<String>,
    ) -> Self {
        let Some((line, range)) = map.map_to_source(diagnostic) else {
            panic!("Unable to source-map {diagnostic:?}");
        };
        let source_snippet = lines[line][range].to_string();
        match diagnostic {
            DiagnosticMessage::Warning(line, _, message) => SourceMappedMessage {
                _type: MessageType::Warning,
                message: message.clone(),
                line: *line,
                source_snippet,
            },
            DiagnosticMessage::Error(line, err) => SourceMappedMessage {
                _type: MessageType::Error,
                message: err.to_string(),
                line: *line,
                source_snippet,
            },
        }
    }

    fn new(
        _type: MessageType,
        message: &'static str,
        line: usize,
        source_snippet: &'static str,
    ) -> Self {
        SourceMappedMessage {
            _type,
            message: message.to_string(),
            line,
            source_snippet: source_snippet.to_string(),
        }
    }
}

fn assert_program_has_source_mapped_diagnostics(
    program: &'static str,
    expected_messages: Vec<SourceMappedMessage>,
) {
    let mut analyzer = SourceFileAnalyzer::analyze_lines(
        program
            .split('\n')
            .map(|s| s.to_owned())
            .collect::<Vec<_>>(),
    );
    let messages = analyzer
        .take_messages()
        .iter()
        .map(|diagnostic| {
            SourceMappedMessage::from_diagnostic(
                diagnostic,
                analyzer.source_file_map(),
                analyzer.source_file_lines(),
            )
        })
        .collect::<Vec<_>>();
    assert_eq!(messages, expected_messages);
}

fn assert_program_has_error(program: &'static str, error: InterpreterError) {
    let analyzer_messages = analyze(program).take_messages();
    if analyzer_messages.len() != 1 {
        panic!("Expected analyzer for program {program} to have one element but got {analyzer_messages:?}");
    }
    let message = analyzer_messages.get(0).unwrap();
    match message {
        abasic_core::DiagnosticMessage::Error(_line, err) => {
            assert_eq!(err.error, error);
        }
        _ => {
            panic!("Expected analyzer for program {program} to have one error but got {message:?}");
        }
    }
}

fn assert_program_token_types(
    program: &'static str,
    token_types: Vec<Vec<(TokenType, Range<usize>)>>,
) {
    assert_eq!(analyze(program).token_types(), &token_types);
}

#[test]
fn print_works() {
    assert_program_is_fine("10 print \"hi\"");
    assert_program_is_fine("10 a$ = \"hello\":print a$");
}

#[test]
fn builtins_work() {
    assert_program_is_fine("10 print rnd(1)");
}

#[test]
fn for_loops_work() {
    assert_program_is_fine("10 for i = 1 to 3: next i");
    assert_program_has_error(
        "5 i = 0\n10 for i$ = 1 to 3: next i\n20 print i$;i",
        InterpreterError::TypeMismatch,
    );
    assert_program_has_error(
        "5 i$ = \"hi\"\n10 for i = 1 to 3: next i$\n20 print i",
        InterpreterError::TypeMismatch,
    );
}

#[test]
fn goto_and_gosub_work() {
    assert_program_is_fine("10 if 0 then 20\n20 print \"hi\"");
    assert_program_is_fine("10 if 1 then 10 else 20\n20 print \"hi\"");
    assert_program_is_fine("10 goto 20\n20 print \"hi\"");
    assert_program_is_fine("10 gosub 20\n20 print \"hi\"");
}

#[test]
fn conditionals_work() {
    assert_program_is_fine("5 x = 0\n10 if x = 1 then print \"one\" else print \"not one\"");
    assert_program_is_fine("5 x = 0\n10 if x = 0 then print \"zero\" else print \"not zero\"");
    assert_program_has_error(
        "5 x = 0\n10 if x = 1 then a = \"hi\" else a = 1\n20 print a",
        InterpreterError::TypeMismatch,
    );
    assert_program_has_error(
        "5 x = 0\n10 if x = 0 then a = 1 else a = \"hi\"\n20 print a",
        InterpreterError::TypeMismatch,
    );
}

#[test]
fn unexpected_end_of_input_works() {
    assert_program_has_error(
        "10 print 1\n20 goto 10blargblarg",
        SyntaxError::UnexpectedEndOfInput.into(),
    );
}

#[test]
fn undefined_statement_error_works() {
    assert_program_has_error("10 if 0 then goto 20", InterpreterError::UndefinedStatement);
    assert_program_has_error(
        "10 if 0 then gosub 20",
        InterpreterError::UndefinedStatement,
    );
    assert_program_has_error("10 if 0 then 20", InterpreterError::UndefinedStatement);
    assert_program_has_error(
        "10 if 0 then 10 else 20",
        InterpreterError::UndefinedStatement,
    );
}

use MessageType::*;

#[test]
fn line_without_statements_warning_works() {
    assert_program_has_source_mapped_diagnostics(
        "10",
        vec![SourceMappedMessage::new(
            Warning,
            "Line contains no statements and will not be defined.",
            0,
            "10",
        )],
    );
}

#[test]
fn line_without_number_warning_works() {
    assert_program_has_source_mapped_diagnostics(
        "print 5",
        vec![SourceMappedMessage::new(
            Warning,
            "Line has no line number, ignoring it.",
            0,
            "",
        )],
    );
}

#[test]
fn unused_symbol_works() {
    assert_program_has_source_mapped_diagnostics(
        "10 a = 5",
        vec![SourceMappedMessage::new(
            Warning,
            "'A' is never used.",
            0,
            "a",
        )],
    );
}

#[test]
fn undefined_symbol_works() {
    assert_program_has_source_mapped_diagnostics(
        "10 print a",
        vec![SourceMappedMessage::new(
            Warning,
            "'A' is never defined.",
            0,
            "a",
        )],
    );
}

#[test]
fn redefined_line_warning_works() {
    assert_program_has_source_mapped_diagnostics(
        "10 print 5\n10 print 10",
        vec![SourceMappedMessage::new(
            Warning,
            "Redefinition of pre-existing BASIC line.",
            1,
            "10",
        )],
    );
}

#[test]
fn unterminated_string_literal_works() {
    assert_program_has_source_mapped_diagnostics(
        "10 print \"boop",
        vec![SourceMappedMessage::new(
            Error,
            "SYNTAX ERROR (UNTERMINATED STRING)",
            0,
            "\"boop",
        )],
    );
}

#[test]
fn type_mismatch_works() {
    assert_program_has_source_mapped_diagnostics(
        "10 a = \"hi\"\n20 print a",
        vec![SourceMappedMessage::new(
            Error,
            "TYPE MISMATCH IN 10",
            0,
            "\"hi\"",
        )],
    );
}

#[test]
fn token_types_works() {
    use TokenType::*;

    assert_program_token_types(
        "10 pr int  \"hi\"",
        vec![vec![(Number, 0..2), (Keyword, 3..9), (String, 11..15)]],
    );
}
