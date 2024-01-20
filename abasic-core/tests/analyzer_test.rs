use abasic_core::{
    DiagnosticMessage, InterpreterError, SourceFileAnalyzer, SourceFileMap, SyntaxError,
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
enum SourceMappedMessage {
    Warning(String, String),
    Error(String, String),
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
            DiagnosticMessage::Warning(_, message) => {
                SourceMappedMessage::Warning(message.clone(), source_snippet)
            }
            DiagnosticMessage::Error(err, _, _) => {
                SourceMappedMessage::Error(err.to_string(), source_snippet)
            }
        }
    }
}

fn assert_program_has_source_mapped_diagnostics(
    program: &'static str,
    expected_messages: Vec<SourceMappedMessage>,
) {
    let lines = program
        .split('\n')
        .map(|s| s.to_owned())
        .collect::<Vec<_>>();
    let mut analyzer = SourceFileAnalyzer::analyze_lines(lines.clone());
    let messages = analyzer
        .take_messages()
        .iter()
        .map(|diagnostic| {
            SourceMappedMessage::from_diagnostic(diagnostic, analyzer.source_file_map(), &lines)
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
        abasic_core::DiagnosticMessage::Error(err, _, _) => {
            assert_eq!(err.error, error);
        }
        _ => {
            panic!("Expected analyzer for program {program} to have one error but got {message:?}");
        }
    }
}

#[test]
fn print_works() {
    assert_program_is_fine("10 print \"hi\"");
    assert_program_is_fine("10 a$ = \"hello\":print a$");
}

#[test]
fn for_loops_work() {
    assert_program_is_fine("10 for i = 1 to 3: next i");
    assert_program_has_error("10 for i$ = 1 to 3: next i", InterpreterError::TypeMismatch);
    assert_program_has_error("10 for i = 1 to 3: next i$", InterpreterError::TypeMismatch);
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
    assert_program_is_fine("10 if x = 1 then print \"one\" else print \"not one\"");
    assert_program_is_fine("10 if x = 0 then print \"zero\" else print \"not zero\"");
    assert_program_has_error(
        "10 if x = 1 then a = \"hi\" else a = 1",
        InterpreterError::TypeMismatch,
    );
    assert_program_has_error(
        "10 if x = 0 then a = 1 else a = \"hi\"",
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

use SourceMappedMessage::*;

#[test]
fn line_without_statements_warning() {
    assert_program_has_source_mapped_diagnostics(
        "10",
        vec![Warning(
            "Line contains no statements and will not be defined.".to_string(),
            "10".to_string(),
        )],
    );
}

#[test]
fn line_without_number_warning() {
    assert_program_has_source_mapped_diagnostics(
        "print 5",
        vec![Warning(
            "Line has no line number, ignoring it.".to_string(),
            "".to_string(),
        )],
    );
}
