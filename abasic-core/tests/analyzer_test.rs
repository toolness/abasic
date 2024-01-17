use abasic_core::{InterpreterError, SourceFileAnalyzer, SyntaxError};

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

fn assert_program_has_error(program: &'static str, error: InterpreterError) {
    let analyzer_messages = analyze(program).take_messages();
    if analyzer_messages.len() != 1 {
        panic!("Expected analyzer for program {program} to have one element but got {analyzer_messages:?}");
    }
    let message = analyzer_messages.get(0).unwrap();
    match message {
        abasic_core::DiagnosticMessage::Error(err, _) => {
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
