use std::{collections::HashMap, rc::Rc};

use crate::{
    interpreter_error::{InterpreterError, TracedInterpreterError},
    program::Program,
    syntax_error::SyntaxError,
    tokenizer::{Token, Tokenizer},
};

#[derive(Debug, Clone)]
enum Value {
    String(Rc<String>),
    Number(f64),
}

#[derive(Debug)]
pub struct Interpreter {
    output: Vec<String>,
    program: Program,
    variables: HashMap<Rc<String>, Value>,
}

impl Interpreter {
    pub fn new() -> Self {
        Interpreter {
            output: vec![],
            program: Default::default(),
            variables: HashMap::new(),
        }
    }

    pub fn get_and_clear_output_buffer(&mut self) -> Option<String> {
        if self.output.is_empty() {
            None
        } else {
            let output = self.output.join("");
            self.output.clear();
            Some(output)
        }
    }

    fn evaluate_expression_term(&mut self) -> Result<Value, TracedInterpreterError> {
        match self.program.next_unwrapped_token()? {
            Token::StringLiteral(string) => Ok(Value::String(string.clone())),
            Token::NumericLiteral(number) => Ok(Value::Number(number)),
            Token::Symbol(variable) => {
                if let Some(value) = self.variables.get(&variable) {
                    Ok(value.clone())
                } else {
                    // TODO: It'd be nice to at least log a warning or something here, since
                    //       this can be a notorious source of bugs.
                    // TODO: If the variable ends with `$` we should return an empty string.
                    Ok(Value::Number(0.0))
                }
            }
            _ => Err(SyntaxError::UnexpectedToken.into()),
        }
    }

    fn evaluate_plus_or_minus(&mut self) -> Option<f64> {
        if let Some(next_token) = self.program.peek_next_token() {
            if let Some(unary_plus_or_minus) = parse_plus_or_minus(&next_token) {
                self.program.consume_next_token();
                Some(unary_plus_or_minus)
            } else {
                None
            }
        } else {
            None
        }
    }

    fn evaluate_expression(&mut self) -> Result<Value, TracedInterpreterError> {
        let unary_plus_or_minus = self.evaluate_plus_or_minus();

        let value =
            maybe_apply_unary_plus_or_minus(unary_plus_or_minus, self.evaluate_expression_term()?)?;
        if let Some(binary_plus_or_minus) = self.evaluate_plus_or_minus() {
            let second_operand = self.evaluate_expression()?;
            Ok(Value::Number(
                unwrap_number(value)? + unwrap_number(second_operand)? * binary_plus_or_minus,
            ))
        } else {
            Ok(value)
        }
    }

    fn evaluate_if_statement(&mut self) -> Result<(), TracedInterpreterError> {
        let conditional_value = self.evaluate_expression()?;
        self.program.expect_next_token(Token::Then)?;
        // TODO: It would be nice to support ELSE somehow, even though
        // AppleSoft basic doesn't really seem to. Tim Hartnell's
        // book seems to include ELSE clauses only in the form of line
        // numbers, e.g. `IF X THEN 100 ELSE 200`, which seems like a
        // reasonable compromise.
        if value_to_bool(&conditional_value) {
            self.evaluate_statement()
        } else {
            self.program.discard_remaining_tokens();
            Ok(())
        }
    }

    fn evaluate_assignment_statement(
        &mut self,
        variable: Rc<String>,
    ) -> Result<(), TracedInterpreterError> {
        self.program.expect_next_token(Token::Equals)?;
        let value = self.evaluate_expression()?;
        // TODO: We should only allow assigning numbers to variables that don't end
        // with `$`, and only allow assigning strings to ones that end with `$`.
        self.variables.insert(variable, value);
        Ok(())
    }

    fn evaluate_print_statement(&mut self) -> Result<(), TracedInterpreterError> {
        while let Some(token) = self.program.peek_next_token() {
            match token {
                Token::Colon => break,
                _ => match self.evaluate_expression()? {
                    Value::String(string) => {
                        self.output.push(string.to_string());
                    }
                    Value::Number(number) => {
                        self.output.push(format!("{}", number));
                    }
                },
            }
        }
        self.output.push(String::from("\n"));
        Ok(())
    }

    fn evaluate_goto_statement(&mut self) -> Result<(), TracedInterpreterError> {
        let Some(Token::NumericLiteral(line_number)) = self.program.next_token() else {
            return Err(InterpreterError::UndefinedStatement.into());
        };
        self.program.goto_line_number(line_number as u64)?;
        Ok(())
    }

    fn evaluate_gosub_statement(&mut self) -> Result<(), TracedInterpreterError> {
        let Some(Token::NumericLiteral(line_number)) = self.program.next_token() else {
            return Err(InterpreterError::UndefinedStatement.into());
        };
        self.program.gosub_line_number(line_number as u64)?;
        Ok(())
    }

    fn evaluate_for_statement(&mut self) -> Result<(), TracedInterpreterError> {
        let Some(Token::Symbol(symbol)) = self.program.next_token() else {
            return Err(SyntaxError::UnexpectedToken.into());
        };
        self.program.expect_next_token(Token::Equals)?;
        let from_value = self.evaluate_expression()?;
        let from_number = unwrap_number(from_value)?;
        self.program.expect_next_token(Token::To)?;
        let to_value = self.evaluate_expression()?;
        let to_number = unwrap_number(to_value)?;

        // TODO: Add support for STEP.

        self.program.start_loop(symbol.clone(), to_number);
        self.variables.insert(symbol, Value::Number(from_number));
        Ok(())
    }

    fn evaluate_next_statement(&mut self) -> Result<(), TracedInterpreterError> {
        let Some(Token::Symbol(symbol)) = self.program.next_token() else {
            return Err(SyntaxError::UnexpectedToken.into());
        };
        let Some(current_value) = self.variables.get(&symbol) else {
            return Err(InterpreterError::NextWithoutFor.into());
        };
        let current_number = unwrap_number(current_value.clone())?;
        let new_number = self.program.end_loop(symbol.clone(), current_number)?;
        self.variables.insert(symbol, Value::Number(new_number));
        Ok(())
    }

    fn evaluate_statement(&mut self) -> Result<(), TracedInterpreterError> {
        match self.program.next_token() {
            Some(Token::Print) => self.evaluate_print_statement(),
            Some(Token::If) => self.evaluate_if_statement(),
            Some(Token::Goto) => self.evaluate_goto_statement(),
            Some(Token::Gosub) => self.evaluate_gosub_statement(),
            Some(Token::Return) => self.program.return_to_last_gosub(),
            Some(Token::End) => Ok(self.program.end()),
            Some(Token::For) => self.evaluate_for_statement(),
            Some(Token::Next) => self.evaluate_next_statement(),
            Some(Token::Remark(_)) => Ok(()),
            Some(Token::Colon) => Ok(()),
            Some(Token::Symbol(value)) => self.evaluate_assignment_statement(value),
            Some(_) => Err(SyntaxError::UnexpectedToken.into()),
            None => Ok(()),
        }
    }

    fn run(&mut self) -> Result<(), TracedInterpreterError> {
        // TODO: We can't just run the program indefinitely, or else
        // our output will never be displayed for infinite loops, and
        // if we're run in JS we'll cause the browser to hang.  We should
        // probably just run one statement, then yield control to the caller
        // and rely on it to continue execution at its convenience.
        loop {
            while self.program.has_next_token() {
                self.evaluate_statement()?;
            }
            if !self.program.next_line() {
                break;
            }
        }
        Ok(())
    }

    fn maybe_process_command(
        &mut self,
        maybe_command: &str,
    ) -> Result<bool, TracedInterpreterError> {
        match maybe_command {
            "RUN" => {
                self.program.goto_first_numbered_line();
                self.run()?;
            }
            "LIST" => {
                self.output.extend(self.program.list());
            }
            _ => {
                return Ok(false);
            }
        };
        return Ok(true);
    }

    /// Evaluate the given line of code.
    ///
    /// Note that this is expected to be a *line*, i.e. it shouldn't contain
    /// any newlines (if it does, a syntax error will be raised).
    pub fn evaluate<T: AsRef<str>>(&mut self, line: T) -> Result<(), TracedInterpreterError> {
        let result = self.evaluate_impl(line);
        if let Err(mut err) = result {
            if let Some(line_number) = self.program.get_line_number() {
                err.set_line_number(line_number);
            }
            Err(err)
        } else {
            result
        }
    }

    pub fn evaluate_impl<T: AsRef<str>>(&mut self, line: T) -> Result<(), TracedInterpreterError> {
        let Some(char) = line.as_ref().chars().next() else {
            return Ok(());
        };
        let mut tokenizer = Tokenizer::new(line);
        let mut line_number: Option<u64> = None;
        if char.is_numeric() {
            let Some(num_result) = tokenizer.next() else {
                panic!("Expected numbered line to tokenize");
            };
            let Token::NumericLiteral(number) = num_result? else {
                panic!("Expected numbered line to start with numeric literal");
            };
            line_number = Some(number as u64);
        }

        let tokens = tokenizer.remaining_tokens()?;

        if let Some(line_number) = line_number {
            self.program.set_numbered_line(line_number, tokens);
        } else {
            if let Some(Token::Symbol(maybe_command)) = tokens.first() {
                if self.maybe_process_command(maybe_command.as_str())? {
                    return Ok(());
                }
            }

            self.program.set_and_goto_immediate_line(tokens);
            self.run()?;
        }

        Ok(())
    }
}

fn parse_plus_or_minus(token: &Token) -> Option<f64> {
    match &token {
        Token::Plus => Some(1.0),
        Token::Minus => Some(-1.0),
        _ => None,
    }
}

fn maybe_apply_unary_plus_or_minus(
    unary_sign: Option<f64>,
    value: Value,
) -> Result<Value, TracedInterpreterError> {
    if let Some(unary_sign) = unary_sign {
        Ok(Value::Number(unwrap_number(value)? * unary_sign))
    } else {
        Ok(value)
    }
}

fn unwrap_number(value: Value) -> Result<f64, TracedInterpreterError> {
    if let Value::Number(number) = value {
        Ok(number)
    } else {
        Err(InterpreterError::TypeMismatch.into())
    }
}

fn value_to_bool(value: &Value) -> bool {
    match value {
        Value::String(string) => !string.is_empty(),
        Value::Number(number) => *number != 0.0,
    }
}

#[cfg(test)]
mod tests {
    use crate::interpreter_error::OutOfMemoryError;

    use super::{Interpreter, InterpreterError};

    fn assert_eval_error(line: &'static str, expected: InterpreterError) {
        let mut interpreter = Interpreter::new();
        match interpreter.evaluate(line) {
            Ok(_) => {
                panic!("expected '{}' to error but it didn't", line);
            }
            Err(err) => {
                assert_eq!(err.error, expected, "evaluating '{}'", line);
            }
        }
    }

    fn assert_eval_output(line: &'static str, expected: &'static str) {
        let mut interpreter = Interpreter::new();
        let output = eval_line_and_expect_success(&mut interpreter, line);
        assert_eq!(output, expected, "evaluating '{}'", line);
    }

    fn assert_program_output(program: &'static str, expected: &'static str) {
        let mut interpreter = Interpreter::new();
        let lines = program.split("\n").map(|line| line.trim_start());
        for line in lines {
            eval_line_and_expect_success(&mut interpreter, line);
        }
        let output = eval_line_and_expect_success(&mut interpreter, "run");
        assert_eq!(output, expected, "running program: {}", program);
    }

    fn assert_program_error(program: &'static str, expected: InterpreterError) {
        let mut interpreter = Interpreter::new();
        let lines = program.split("\n").map(|line| line.trim_start());
        for line in lines {
            eval_line_and_expect_success(&mut interpreter, line);
        }
        match interpreter.evaluate("run") {
            Ok(_) => {
                panic!("expected program to error but it didn't: {}", program);
            }
            Err(err) => {
                assert_eq!(err.error, expected, "running program: {}", program);
            }
        }
    }

    fn eval_line_and_expect_success<T: AsRef<str>>(
        interpreter: &mut Interpreter,
        line: T,
    ) -> String {
        match interpreter.evaluate(line.as_ref()) {
            Ok(_) => interpreter
                .get_and_clear_output_buffer()
                .unwrap_or_default(),
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
    fn print_works_with_math() {
        assert_eval_output("print +4", "4\n");
        assert_eval_output("print -4", "-4\n");
        assert_eval_output("print -4 - 4", "-8\n");
        assert_eval_output("print -4 + 4", "0\n");
        assert_eval_output("print 1 + 1", "2\n");
        assert_eval_output("print 1 + 1 - 3", "-1\n");
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
    fn if_statement_processes_multiple_statements() {
        assert_eval_output("if 1 then print \"hi\":print", "hi\n\n");
        assert_eval_output("if 0 then print \"hi\":print:kaboom", "");
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
    }

    #[test]
    fn nested_looping_works() {
        assert_eval_output(
            "for i = 1 to 2: print \"i = \" i:for j = 1 to 2:print \"j = \" j:next j:next i",
            "i = 1\nj = 1\nj = 2\ni = 2\nj = 1\nj = 2\n",
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
    fn type_mismatch_error_works() {
        assert_eval_error("print -\"hi\"", InterpreterError::TypeMismatch);
        assert_eval_error("print \"hi\" - 4", InterpreterError::TypeMismatch);
        assert_eval_error("print 4 + \"hi\"", InterpreterError::TypeMismatch);
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
            60
            "#,
            "sup\ndog\n",
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
            60
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
}
