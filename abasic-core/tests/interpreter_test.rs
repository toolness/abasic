use abasic_core::{
    DiagnosticMessage, Interpreter, InterpreterError, InterpreterOutput, InterpreterState,
    OutOfMemoryError, SourceFileAnalyzer, SyntaxError, Token, TracedInterpreterError,
};

struct Action {
    expected_output: &'static str,
    then_input: Option<&'static str>,
}

impl Action {
    fn expect_output(expected_output: &'static str) -> Self {
        Action {
            expected_output,
            then_input: None,
        }
    }

    fn then_input(mut self, input: &'static str) -> Self {
        self.then_input = Some(input);
        self
    }
}

fn create_interpreter() -> Interpreter {
    Interpreter::default()
}

fn evaluate_while_running(interpreter: &mut Interpreter) -> Result<(), TracedInterpreterError> {
    while interpreter.get_state() == InterpreterState::Running {
        interpreter.continue_evaluating()?;
    }
    Ok(())
}

fn evaluate_line_while_running(
    interpreter: &mut Interpreter,
    line: &str,
) -> Result<(), TracedInterpreterError> {
    interpreter.start_evaluating(line)?;
    evaluate_while_running(interpreter)
}

fn ensure_no_analyzer_errors<T: AsRef<str>>(program: T) {
    let lines = program
        .as_ref()
        .split("\n")
        .map(|line| line.trim_start())
        .map(|s| s.to_owned());
    let mut analyzer = SourceFileAnalyzer::analyze(lines.clone().collect::<Vec<_>>().join("\n"));
    for message in analyzer.take_messages() {
        if let DiagnosticMessage::Error(_line, e) = message {
            panic!(
                "expected '{}' to raise no analyzer errors but got {:?}",
                program.as_ref(),
                e
            );
        }
    }
}

fn assert_eval_error(line: &'static str, expected: InterpreterError) {
    let mut interpreter = create_interpreter();
    match evaluate_line_while_running(&mut interpreter, line) {
        Ok(_) => {
            panic!("expected '{}' to error but it didn't", line);
        }
        Err(err) => {
            assert_eq!(err.error, expected, "evaluating '{}'", line);
        }
    }
}

fn assert_eval_output(line: &'static str, expected: &'static str) {
    ensure_no_analyzer_errors(format!("10 {line}"));
    let mut interpreter = create_interpreter();
    let output = eval_line_and_expect_success(&mut interpreter, line);
    assert_eq!(output, expected, "evaluating '{}'", line);
}

fn assert_program_actions(program: &'static str, actions: &[Action]) {
    ensure_no_analyzer_errors(program);
    let mut interpreter = create_interpreter();
    let lines = program.split("\n").map(|line| line.trim_start());
    for line in lines {
        eval_line_and_expect_success(&mut interpreter, line);
    }
    let mut output = eval_line_and_expect_success(&mut interpreter, "run");
    for (i, action) in actions.iter().enumerate() {
        assert_eq!(
            output, action.expected_output,
            "running action {} of program: {}",
            i, program
        );
        if let Some(input) = action.then_input {
            interpreter.provide_input(input.to_string());
            output = match evaluate_while_running(&mut interpreter) {
                Ok(_) => take_output_as_string(&mut interpreter),
                Err(err) => {
                    panic!(
                        "after inputting '{}', expected successful evaluation but got {}\nIntepreter state is: {:?}",
                        input,
                        err,
                        interpreter
                    )
                }
            }
        }
    }
}

fn assert_program_output(program: &'static str, expected: &'static str) {
    assert_program_actions(program, &[Action::expect_output(expected)]);
}

fn assert_program_error(program: &'static str, expected: InterpreterError) {
    let mut interpreter = create_interpreter();
    let lines = program.split("\n").map(|line| line.trim_start());
    for line in lines {
        eval_line_and_expect_success(&mut interpreter, line);
    }
    match evaluate_line_while_running(&mut interpreter, "run") {
        Ok(_) => {
            panic!("expected program to error but it didn't: {}", program);
        }
        Err(err) => {
            assert_eq!(err.error, expected, "running program: {}", program);
        }
    }
}

fn take_output_as_string(interpreter: &mut Interpreter) -> String {
    interpreter
        .take_output()
        .into_iter()
        .map(|output| match output {
            InterpreterOutput::Print(message) => message.to_string(),
            _ => format!("{}\n", output.to_string()),
        })
        .collect::<Vec<_>>()
        .join("")
}

fn eval_line_and_expect_success<T: AsRef<str>>(interpreter: &mut Interpreter, line: T) -> String {
    match evaluate_line_while_running(interpreter, line.as_ref()) {
        Ok(_) => take_output_as_string(interpreter),
        Err(err) => {
            panic!(
                "expected '{}' to evaluate successfully but got {}\nIntepreter state is: {:?}",
                line.as_ref(),
                err,
                interpreter
            )
        }
    }
}

#[test]
fn empty_line_works() {
    assert_eval_output("", "");
    assert_eval_output(" ", "");
}

#[test]
fn print_works() {
    assert_eval_output("print", "\n");
    assert_eval_output("print \"\"", "\n");
    assert_eval_output("print \"hello 😊\"", "hello 😊\n");
    assert_eval_output("print \"hello 😊\" 5", "hello 😊5\n");
    assert_eval_output("print \"hello 😊\" 5 \"there\"", "hello 😊5there\n");
}

#[test]
fn print_as_question_mark_works() {
    // This is a shortcut that Applesoft BASIC provides.
    assert_eval_output("?", "\n");
    assert_eval_output("? \"\"", "\n");
    assert_eval_output("? \"hello 😊\"", "hello 😊\n");
    assert_eval_output("? \"hello 😊\" 5", "hello 😊5\n");
    assert_eval_output("? \"hello 😊\" 5 \"there\"", "hello 😊5there\n");
}

#[test]
fn print_works_with_comma() {
    assert_eval_output("print ,1", "\t1\n");
}

#[test]
fn print_works_with_semicolon() {
    assert_eval_output("print ;", "");
    assert_eval_output("print ;\"\"", "\n");
    assert_eval_output("print \"hello 😊\";", "hello 😊");
    assert_eval_output("print \"hello\";:print \"there\"", "hellothere\n");
}

#[test]
fn print_works_with_math() {
    assert_eval_output("print +4", "4\n");
    assert_eval_output("print -4", "-4\n");
    assert_eval_output("print -4 - 4", "-8\n");
    assert_eval_output("print -4 + 4", "0\n");
    assert_eval_output("print 1 + 1", "2\n");
    assert_eval_output("print 1 + 1 - 3", "-1\n");
    assert_eval_output("print 2 * 3", "6\n");
    assert_eval_output("print 2 * 3 + 2", "8\n");
    assert_eval_output("print 2 * 3 + 2 * 4", "14\n");
    assert_eval_output("print 1 / 2", "0.5\n");
    assert_eval_output("print 1 / 2 + 5", "5.5\n");
    assert_eval_output("print 1 / 2 + 5 / 2", "3\n");
    assert_eval_output("print 2 * -3", "-6\n");
    assert_eval_output("print 2 + 5 * 4 - 1", "21\n");
    assert_eval_output("print 5 * 4 * 2", "40\n");
    assert_eval_output("print (5 + 3) * 4", "32\n");
    assert_eval_output("print 15 / 5 * 3", "9\n");
}

#[test]
fn print_works_with_numeric_equality_expressions() {
    assert_eval_output("print 1 = 2", "0\n");
    assert_eval_output("print 1 = 1", "1\n");
    assert_eval_output("print 2 = 2", "1\n");
    assert_eval_output("print 1 + 1 = 3 - 1", "1\n");
    assert_eval_output("print 1 + 1 = 4 - 1", "0\n");
    assert_eval_output("print -1 = 1", "0\n");
    assert_eval_output("print 1 = -1", "0\n");

    assert_eval_output("print 1 < 2", "1\n");
    assert_eval_output("print 1 < 1", "0\n");
    assert_eval_output("print 1 > 2", "0\n");
    assert_eval_output("print 1 > 0", "1\n");
    assert_eval_output("print 1 > 1", "0\n");

    assert_eval_output("print 1 >= 1", "1\n");
    assert_eval_output("print 2 >= 1", "1\n");
    assert_eval_output("print 2 >= 3", "0\n");

    assert_eval_output("print 1 <= 1", "1\n");
    assert_eval_output("print 2 <= 1", "0\n");
    assert_eval_output("print 2 <= 3", "1\n");

    assert_eval_output("print 1 <> 2", "1\n");
    assert_eval_output("print 1 <> 1", "0\n");
}

#[test]
fn print_works_with_chained_numeric_equality_expressions() {
    assert_eval_output("print 5 > 4 > 3", "0\n");
    assert_eval_output("print 5 > 4 = 1", "1\n");
}

#[test]
fn exponentiation_works() {
    assert_eval_output("print 5 ^ 2", "25\n");
    assert_eval_output("print 5 ^ 2 + 3 ^ 2", "34\n");
    assert_eval_output("print 5 ^ 2 * 2 ^ 2", "100\n");
    assert_eval_output("print -5 ^ 2", "25\n");
}

#[test]
fn unary_logical_operator_works() {
    assert_eval_output("print not 5", "0\n");
    assert_eval_output("print not 0", "1\n");
    assert_eval_output("print not 5 * 300", "0\n");
    assert_eval_output("print not 0 * 300", "300\n");
    assert_eval_output("print not 0 + 10", "11\n");
    assert_eval_output("print not 530 + 10", "10\n");
}

#[test]
fn binary_logical_operators_work() {
    assert_eval_output("print 5 AND 2", "1\n");
    assert_eval_output("print 5 AND 0", "0\n");
    assert_eval_output("print 0 AND 0", "0\n");

    assert_eval_output("print 5 OR 2", "1\n");
    assert_eval_output("print 5 OR 0", "1\n");
    assert_eval_output("print 0 OR 0", "0\n");
}

#[test]
fn abs_works() {
    assert_eval_output("print abs(5)", "5\n");
    assert_eval_output("print abs(-5)", "5\n");
    assert_eval_output("print abs(-6.0 + 1)", "5\n");
    assert_eval_output("print abs(x)", "0\n");
}

#[test]
fn int_works() {
    assert_eval_output("print int(3)", "3\n");
    assert_eval_output("print int(4.1)", "4\n");
    assert_eval_output("print int(5.9)", "5\n");
}

#[test]
fn rnd_with_positive_number_works() {
    assert_eval_output(
        "for i = 1 to 3:print int(rnd(1) * 50):next i",
        "5\n31\n20\n",
    );

    assert_eval_output(
        "for i = 1 to 3:print int(rnd(i) * 50):next i",
        "5\n31\n20\n",
    );
}

#[test]
fn rnd_with_zero_works() {
    assert_eval_output(
        "print int(rnd(1) * 50):for i = 1 to 2:print int(rnd(0) * 50):next i",
        "5\n5\n5\n",
    );
}

#[test]
fn rnd_with_negative_number_is_unimplemented() {
    assert_eval_error("print rnd(-1)", InterpreterError::Unimplemented);
}

#[ignore]
#[test]
fn builtin_functions_cannot_be_redefined() {
    todo!("TODO: Add a test to make sure ABS can't be redefined, etc.");
}

#[test]
fn print_works_with_string_equality_expressions() {
    assert_eval_output("print \"hi\" = \"hi\"", "1\n");
    assert_eval_output("print \"hi\" = \"there\"", "0\n");

    assert_eval_output("print \"hi\" < x$", "0\n");
    assert_eval_output("print \"hi\" > x$", "1\n");

    assert_eval_output("print \"hi\" <> x$", "1\n");
    assert_eval_output("print x$ > x$", "0\n");
}

#[test]
fn colon_works() {
    assert_eval_output(":::", "");
    assert_eval_output("print 4:print \"hi\"", "4\nhi\n");
}

#[test]
fn if_statement_works_with_strings() {
    assert_eval_output("if \"\" then print \"THIS SHOULD NOT APPEAR\"", "");
    assert_eval_output("if \"hi\" then print \"YO\"", "YO\n");
}

#[test]
fn if_statement_works_with_numbers() {
    assert_eval_output("if 0 then print \"THIS SHOULD NOT APPEAR\"", "");
    assert_eval_output("x = 0:if x then print \"THIS SHOULD NOT APPEAR\"", "");
    assert_eval_output("if 1 then print \"YO\"", "YO\n");
    assert_eval_output("if 0+0 then print \"THIS SHOULD NOT APPEAR\"", "");
}

#[test]
fn if_statement_processes_multiple_statements_in_then_clause() {
    assert_eval_output("if 1 then print \"hi\":print", "hi\n\n");
    assert_eval_output(
        "if 0 then print \"hi\":print:print \"this should not print\"",
        "",
    );
    assert_eval_output("if 1 then x=3:print \"hi \" x:print", "hi 3\n\n");
}

#[test]
fn if_statement_processes_multiple_statements_in_else_clause() {
    assert_eval_output("if 1 then print \"hi\" else print \"blah\":print \"this is only executed with the else clause\"", "hi\n");
    assert_eval_output(
        "if 0 then print \"hi\" else print \"blah\":print",
        "blah\n\n",
    );
    assert_eval_output(
        "if 1 then y=4 else x=3:print \"this is only executed with the else clause\"",
        "",
    );
    assert_eval_output("if 0 then y=4 else x=3:print \"hallo \" y", "hallo 0\n");
}

#[test]
fn if_statement_does_not_support_else_when_then_clause_has_multiple_statements() {
    assert_eval_error(
        "if 1 then print:print else print",
        SyntaxError::UnexpectedToken.into(),
    );

    assert_eval_error(
        "if 1 then x = 3:y = 4 else z = 3",
        SyntaxError::UnexpectedToken.into(),
    );
}

#[test]
fn default_array_values_work() {
    assert_eval_output("print a(1)", "0\n");
    assert_eval_output("print a$(1)", "\n");
    assert_eval_output("print a(1,2,3)", "0\n");
    assert_eval_output("print a$(1,2,3)", "\n");
}

#[test]
fn array_assignment_works() {
    assert_eval_output("a(0) = 5:print a(0)", "5\n");
    assert_eval_output("a(1,1) = 5:print a(1,1)", "5\n");

    assert_eval_output("a$(0) = \"blarg\":print a$(0)", "blarg\n");
    assert_eval_output("a$(1,1) = \"blarg\":print a$(1,1)", "blarg\n");

    assert_eval_output("x = 4:a(x+1) = 123:print a(5)", "123\n");
}

#[test]
fn variables_and_arrays_exist_in_separate_universes() {
    // This is not a bug, it's how Applesoft BASIC works. Although it might
    // be a bug in Applesoft BASIC, I'm not sure.
    assert_eval_output("print a:print a(1)", "0\n0\n");
    assert_eval_output("a = 1:print a:print a(1)", "1\n0\n");
    assert_eval_output("print a(1):a = 1:print a:print a(1)", "0\n1\n0\n");
}

#[test]
fn assignment_works_with_let() {
    assert_eval_output("let x=1:print x", "1\n");
}

#[test]
fn assignment_works() {
    assert_eval_output("x=1:print x", "1\n");
    assert_eval_output("X=1:print x", "1\n");
    assert_eval_output("x5=1:print x5", "1\n");
    assert_eval_output("x=1+1:print x", "2\n");
    assert_eval_output("x=1:print x + 2", "3\n");
    assert_eval_output("x=1:print x:x = x + 1:print x", "1\n2\n");
}

#[test]
fn default_number_values_work() {
    assert_eval_output("print x", "0\n");
    assert_eval_output("x = x + 1:print x", "1\n");
}

#[test]
fn default_string_values_work() {
    assert_eval_output("print x$", "\n");
}

#[test]
fn remark_works() {
    assert_eval_output("REM hi", "");
    assert_eval_output("rem hi 😊", "");
    assert_eval_output("REM:PRINT \"THIS SHOULD NOT APPEAR\"", "");
    assert_eval_output("PRINT \"hi\":REM:PRINT \"THIS SHOULD NOT APPEAR\"", "hi\n");
}

#[test]
fn looping_works() {
    assert_eval_output(
        "for i = 1 to 3: print i:next i:print \"DONE\" i",
        "1\n2\n3\nDONE4\n",
    );

    assert_eval_output("for i = 4 to 6: print i:next i", "4\n5\n6\n");

    assert_eval_output(
        "for i = 3 to 1 step -1: print i:next i:print \"DONE\" i",
        "3\n2\n1\nDONE0\n",
    );

    assert_eval_output("for i = 1 to 3 step 2: print i:next i", "1\n3\n");
}

#[test]
fn nested_looping_works() {
    assert_eval_output(
        "for i = 1 to 2: print \"i = \" i:for j = 1 to 2:print \"j = \" j:next j:next i",
        "i = 1\nj = 1\nj = 2\ni = 2\nj = 1\nj = 2\n",
    );
}

#[test]
fn weird_looping_works() {
    // This is weird but works in Applesoft BASIC.
    assert_eval_output(
        r#"for i = 1 to 2: print "i = " i:for j = 1 to 2:print "j = " j:next i"#,
        "i = 1\nj = 1\ni = 2\nj = 1\n",
    );
}

#[test]
fn next_without_for_error_works() {
    assert_eval_error("next i", InterpreterError::NextWithoutFor);
    assert_eval_error("for i = 1 to 3:next j", InterpreterError::NextWithoutFor);
    assert_eval_error(
        "for j = 1 to 3:for i = 1 to 3:next j:next i",
        InterpreterError::NextWithoutFor,
    );
}

#[test]
fn type_mismatch_error_works_with_arithmetic_expressions() {
    assert_eval_error("print -\"hi\"", InterpreterError::TypeMismatch);
    assert_eval_error("print \"hi\" - 4", InterpreterError::TypeMismatch);
    assert_eval_error("print 4 + \"hi\"", InterpreterError::TypeMismatch);
}

#[test]
fn type_mismatch_error_works_with_equality_expressions() {
    assert_eval_error("print x = x$", InterpreterError::TypeMismatch);
    assert_eval_error("print x$ = x", InterpreterError::TypeMismatch);
    assert_eval_error("print x < x$", InterpreterError::TypeMismatch);
    assert_eval_error("print x$ < x", InterpreterError::TypeMismatch);
    assert_eval_error("print x > x$", InterpreterError::TypeMismatch);
    assert_eval_error("print x$ > x", InterpreterError::TypeMismatch);
}

#[test]
fn type_mismatch_error_works_with_variable_assignment() {
    assert_eval_error("x = x$", InterpreterError::TypeMismatch);
    assert_eval_error("x = \"hi\"", InterpreterError::TypeMismatch);

    assert_eval_error("x$ = x", InterpreterError::TypeMismatch);
    assert_eval_error("x$ = 1", InterpreterError::TypeMismatch);
}

#[test]
fn type_mismatch_error_works_with_array_assignment() {
    assert_eval_error("x(1) = x$", InterpreterError::TypeMismatch);
    assert_eval_error("x(1) = \"hi\"", InterpreterError::TypeMismatch);

    assert_eval_error("x$(1) = x", InterpreterError::TypeMismatch);
    assert_eval_error("x$(1) = 1", InterpreterError::TypeMismatch);
}

#[test]
fn out_of_data_error_works() {
    assert_eval_error("read a", InterpreterError::OutOfData);
    // Applesoft BASIC only recognizes data in actual line numbers in the program, so
    // an immediate data statement is basically a no-op.
    assert_eval_error("data 1,2,3:read a", InterpreterError::OutOfData);
}

#[test]
fn syntax_error_raised_when_no_array_index_is_given() {
    assert_eval_error("print a()", SyntaxError::UnexpectedToken.into());
}

#[test]
fn illegal_quantity_error_works() {
    assert_eval_error("print a(-1)", InterpreterError::IllegalQuantity);
}

#[test]
fn bad_subscript_error_works() {
    assert_eval_error("print a(1):print a(1,1)", InterpreterError::BadSubscript);
    assert_eval_error("print a(1,1):print a(1)", InterpreterError::BadSubscript);
    assert_eval_error("a(1) = 5:print a(1,1)", InterpreterError::BadSubscript);

    // This is weird b/c implicitly-created arrays are sized at 10 dimensions.
    assert_eval_error("print a(11)", InterpreterError::BadSubscript);
}

#[test]
fn type_mismatch_error_works_with_array_indexing() {
    assert_eval_error("print a(\"hi\")", InterpreterError::TypeMismatch);
}

#[test]
fn undefined_statement_error_works() {
    assert_eval_error("goto 30", InterpreterError::UndefinedStatement);
    assert_eval_error("goto x", InterpreterError::UndefinedStatement);
    assert_eval_error("gosub 30", InterpreterError::UndefinedStatement);
    assert_eval_error("gosub x", InterpreterError::UndefinedStatement);
}

#[test]
fn return_without_gosub_error_works() {
    assert_eval_error("return", InterpreterError::ReturnWithoutGosub);
}

#[test]
fn division_by_zero_error_works() {
    assert_eval_error("print 5/0", InterpreterError::DivisionByZero);
}

#[test]
fn dim_works() {
    assert_eval_output("dim a(100):a(57) = 123:print a(56):print a(57)", "0\n123\n");

    // This is weird, but Applesoft is weird.
    assert_eval_output("dim a:dim a:a = 5:print a:dim a:print a", "5\n5\n");
}

#[test]
fn redimensioned_array_error_works() {
    assert_eval_error("dim a(1):dim a(1)", InterpreterError::RedimensionedArray);
}

#[test]
fn data_is_ignored() {
    assert_eval_output("print 1:data a,b,c:print 2", "1\n2\n");
}

#[test]
fn line_numbers_work() {
    assert_program_output(
        r#"
        10 print "sup"
        20 print "dog"
        "#,
        "sup\ndog\n",
    );
}

#[test]
fn line_numbers_can_be_redefined() {
    assert_program_output(
        r#"
        10 print "boop"
        10 print "sup"
        20 print "dog"
        "#,
        "sup\ndog\n",
    );
}

#[test]
fn empty_line_numbers_are_deleted() {
    assert_program_error(
        r#"
        10 goto 20
        20 print "sup"
        20
        "#,
        InterpreterError::UndefinedStatement,
    );
}

#[test]
fn out_of_order_line_numbers_work() {
    assert_program_output(
        r#"
        20 print "dog"
        10 print "sup"
        "#,
        "sup\ndog\n",
    );
}

#[test]
fn goto_works() {
    assert_program_output(
        r#"
        10 print "sup"
        20 goto 40
        30 print "THIS SHOULD NOT PRINT"
        40 print "dog"
        "#,
        "sup\ndog\n",
    );
}

#[test]
fn end_works() {
    assert_program_output(
        r#"
        10 print "sup"
        20 print "dog"
        30 end
        40 print "THIS SHOULD NOT PRINT"
        "#,
        "sup\ndog\n",
    );
}

#[test]
fn gosub_works() {
    assert_program_output(
        r#"
        10 gosub 40
        20 print "dog"
        30 goto 60
        40 print "sup"
        50 return
        60 end
        "#,
        "sup\ndog\n",
    );
}

#[test]
fn loop_with_goto_after_next_works() {
    assert_program_output(
        r#"
        10 for i = 1 to 3
        20 if i = 2 then goto 60
        30 print i
        40 next i
        50 end
        60 print "TWO":goto 40
        "#,
        "1\nTWO\n3\n",
    );
}

#[test]
fn loop_with_goto_before_for_works() {
    assert_program_output(
        r#"
        10 goto 30
        20 next i
        30 for i = 1 to 3
        40 print i
        50 if i = 3 then end
        60 goto 20
        "#,
        "1\n2\n3\n",
    );
}

#[test]
fn gosub_works_in_line_with_colons() {
    assert_program_output(
        r#"
        10 print "calling":gosub 40:print "returned"
        20 print "dog"
        30 goto 60
        40 print "sup"
        50 return
        60 end
        "#,
        "calling\nsup\nreturned\ndog\n",
    );
}

#[test]
fn stack_overflow_works() {
    assert_program_error(
        r#"
        10 print "hi"
        20 gosub 10
        "#,
        InterpreterError::OutOfMemory(OutOfMemoryError::StackOverflow),
    );
}

#[test]
fn conditional_goto_works() {
    assert_program_output(
        r#"
        10 print "sup"
        15 x = 1
        20 if x then goto 40
        30 print "THIS SHOULD NOT PRINT"
        40 print "dog"
        "#,
        "sup\ndog\n",
    );
}

#[test]
fn then_clause_works_with_only_line_number() {
    assert_program_output(
        r#"
        20 if 1 then 40
        30 print "THIS SHOULD NOT PRINT"
        40 print "hi"
        "#,
        "hi\n",
    );
}

#[test]
fn else_clause_works_with_only_line_number() {
    assert_program_output(
        r#"
        20 if 0 then 30 else 40
        30 print "THIS SHOULD NOT PRINT"
        40 print "hi"
        "#,
        "hi\n",
    );
}

#[test]
fn restore_works() {
    assert_program_output(
        r#"
        10 data sup,dog,1
        20 for i = 1 to 3
        30 read a$
        40 print a$
        45 restore
        50 next i
        "#,
        "sup\nsup\nsup\n",
    );
}

#[test]
fn read_works_with_commas() {
    assert_program_output(
        r#"
        10 data sup,dog,1
        20 read A$,b$,c
        30 print a$
        40 print b$
        50 print c
        "#,
        "sup\ndog\n1\n",
    );
}

#[test]
fn data_works_with_arrays() {
    assert_program_output(
        r#"
        10 data sup,dog,1
        20 for i = 1 to 3
        30 read a$(i)
        40 print a$(i)
        50 next i
        "#,
        "sup\ndog\n1\n",
    );
}

#[test]
fn data_at_beginning_works() {
    assert_program_output(
        r#"
        10 data sup,dog,1
        20 for i = 1 to 3
        30 read a$
        40 print a$
        50 next i
        "#,
        "sup\ndog\n1\n",
    );
}

#[test]
fn data_at_end_works() {
    assert_program_output(
        r#"
        20 for i = 1 to 3
        30 read a$
        40 print a$
        50 next i
        60 data sup,dog,1
        "#,
        "sup\ndog\n1\n",
    );
}

#[test]
fn data_in_middle_works() {
    assert_program_output(
        r#"
        20 for i = 1 to 3
        30 read a$
        35 data sup,dog,1
        40 print a$
        50 next i
        "#,
        "sup\ndog\n1\n",
    );
}

#[test]
fn data_type_mismatch_works() {
    assert_program_error(
        r#"
        10 data sup
        20 read a
        "#,
        InterpreterError::DataTypeMismatch,
    );
}

#[test]
fn statements_are_processed_after_function_definitions() {
    assert_program_output(
        r#"
        10 def fna(x) = x + 1:print "hi"
        20 print fna(1)
        "#,
        "hi\n2\n",
    );
}

#[test]
fn immediate_functions_do_not_work() {
    assert_eval_error("def fna(x) = x + 1", InterpreterError::IllegalDirect);
}

#[test]
fn functions_work() {
    assert_program_output(
        r#"
        10 def fna(x) = x + 1
        20 print fna(1)
        "#,
        "2\n",
    );
}

#[test]
fn functions_with_multiple_arguments_work() {
    assert_program_output(
        r#"
        10 def fna(x,y,z) = x + y + z + 1
        20 print fna(1,2,3)
        "#,
        "7\n",
    );
}

#[test]
fn nested_functions_work() {
    assert_program_output(
        r#"
        10 def fna(x) = x + 1
        20 def fnb(x) = fna(x) + 1
        30 print fnb(1)
        "#,
        "3\n",
    );
}

#[test]
fn nested_functions_weirdly_look_at_the_stack_of_their_callers() {
    // THIS IS EXTREMELY WEIRD but it's what Applesoft BASIC does. Not
    // sure whether it's a feature or a bug.
    assert_program_output(
        r#"
        1 y = 0
        10 def fna(x) = x + y + 1
        20 def fnb(y) = fna(y)
        30 print fnb(1)
        "#,
        "3\n",
    );
}

#[test]
fn function_calls_with_badly_typed_arguments_fail() {
    assert_program_error(
        r#"
        10 def fna(x) = x
        20 print fna("boop")
        "#,
        InterpreterError::TypeMismatch.into(),
    );
}

#[test]
fn function_calls_without_enough_arguments_fail() {
    assert_program_error(
        r#"
        10 def fna(x,y,z) = x + y + z + 1
        20 print fna(1,2)
        "#,
        SyntaxError::ExpectedToken(Token::Comma).into(),
    );
}

#[test]
fn function_calls_with_too_many_arguments_fail() {
    assert_program_error(
        r#"
        10 def fna(x,y,z) = x + y + z + 1
        20 print fna(1,2,3,4)
        "#,
        SyntaxError::ExpectedToken(Token::RightParen).into(),
    );
}

#[test]
fn infinite_recursion_causes_stack_overflow() {
    assert_program_error(
        r#"
        10 def fna(x) = fna(x) + 1
        20 print fna(1)
        "#,
        OutOfMemoryError::StackOverflow.into(),
    );
}

#[test]
fn input_works() {
    assert_program_actions(
        r#"
        10 input a$
        20 print "hello " a$
    "#,
        &[
            Action::expect_output("").then_input("buddy"),
            Action::expect_output("hello buddy\n"),
        ],
    )
}

#[test]
fn input_works_with_arrays() {
    assert_program_actions(
        r#"
        10 input a$(0)
        20 print "hello " a$(0)
    "#,
        &[
            Action::expect_output("").then_input("buddy"),
            Action::expect_output("hello buddy\n"),
        ],
    )
}

#[test]
fn input_reentry_works() {
    assert_program_actions(
        r#"
        10 input a
        20 print "hello " a
    "#,
        &[
            Action::expect_output("").then_input("this is not a number"),
            Action::expect_output("REENTER\n").then_input("123"),
            Action::expect_output("hello 123\n"),
        ],
    )
}

#[test]
fn input_ignoring_extra_works_with_commas() {
    assert_program_actions(
        r#"
        10 input a$
        20 print "hello " a$
    "#,
        &[
            Action::expect_output("").then_input("sup, dog"),
            Action::expect_output("EXTRA IGNORED\nhello sup\n"),
        ],
    )
}

#[test]
fn input_ignoring_extra_works_with_colons() {
    // This is weird, but it's how Applesoft BASIC works, and it's how
    // this interpreter works because it's the easiest thing to implement.
    assert_program_actions(
        r#"
        10 input a$
        20 print "hello " a$
    "#,
        &[
            Action::expect_output("").then_input("sup:dog"),
            Action::expect_output("EXTRA IGNORED\nhello sup\n"),
        ],
    )
}
