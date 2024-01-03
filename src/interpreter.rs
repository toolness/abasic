use std::{collections::HashMap, rc::Rc};

use crate::{
    builtins,
    data::parse_data_until_colon,
    dim::ValueArray,
    interpreter_error::{InterpreterError, TracedInterpreterError},
    operators::{EqualityOp, MultiplyOrDivideOp, PlusOrMinusOp},
    program::Program,
    syntax_error::SyntaxError,
    tokenizer::{Token, Tokenizer},
    value::Value,
};

struct LValue {
    symbol_name: Rc<String>,
    array_index: Option<Vec<usize>>,
}

#[derive(Debug, PartialEq, Copy, Clone)]
pub enum InterpreterState {
    Idle,
    Running,
    AwaitingInput,
}

#[derive(Debug)]
pub struct Interpreter {
    output: Vec<String>,
    program: Program,
    variables: HashMap<Rc<String>, Value>,
    arrays: HashMap<Rc<String>, ValueArray>,
    state: InterpreterState,
    input: Option<String>,
}

impl Interpreter {
    pub fn new() -> Self {
        Interpreter {
            output: vec![],
            program: Default::default(),
            variables: HashMap::new(),
            arrays: HashMap::new(),
            state: InterpreterState::Idle,
            input: None,
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

    fn evaluate_unary_function<F: Fn(Value) -> Result<Value, TracedInterpreterError>>(
        &mut self,
        f: F,
    ) -> Result<Value, TracedInterpreterError> {
        self.program.expect_next_token(Token::LeftParen)?;
        let arg = self.evaluate_expression()?;
        self.program.expect_next_token(Token::RightParen)?;
        f(arg)
    }

    fn evaluate_function_call(
        &mut self,
        function_name: &str,
    ) -> Result<Option<Value>, TracedInterpreterError> {
        let result = match function_name {
            "ABS" => self.evaluate_unary_function(builtins::abs),
            "INT" => self.evaluate_unary_function(builtins::int),
            "RND" => self.evaluate_unary_function(builtins::rnd),
            _ => {
                return Ok(None);
            }
        };
        result.map(|value| Some(value))
    }

    fn parse_optional_array_index(&mut self) -> Result<Option<Vec<usize>>, TracedInterpreterError> {
        if self.program.peek_next_token() != Some(Token::LeftParen) {
            Ok(None)
        } else {
            self.parse_array_index().map(|index| Some(index))
        }
    }

    fn parse_array_index(&mut self) -> Result<Vec<usize>, TracedInterpreterError> {
        let mut indices: Vec<usize> = vec![];
        self.program.expect_next_token(Token::LeftParen)?;
        loop {
            let Value::Number(value) = self.evaluate_expression()? else {
                return Err(InterpreterError::TypeMismatch.into());
            };
            let Ok(index) = usize::try_from(value as i64) else {
                return Err(InterpreterError::IllegalQuantity.into());
            };
            indices.push(index);
            if !self.program.accept_next_token(Token::Comma) {
                break;
            }
        }
        self.program.expect_next_token(Token::RightParen)?;
        Ok(indices)
    }

    fn maybe_create_default_array(
        &mut self,
        array_name: &Rc<String>,
        dimensions: usize,
    ) -> Result<(), TracedInterpreterError> {
        // It seems we can't use hash_map::Entry here to provide a default value,
        // because we might actually error when creating the default value.
        if !self.arrays.contains_key(array_name) {
            // TODO: It'd be nice to at least log a warning or something here, since
            //       this can be a notorious source of bugs.
            let array = ValueArray::default_for_variable_and_dimensionality(
                &array_name.as_str(),
                dimensions,
            )?;
            self.arrays.insert(array_name.clone(), array);
        }
        Ok(())
    }

    fn set_value_at_array_index(
        &mut self,
        array_name: &Rc<String>,
        index: &Vec<usize>,
        value: Value,
    ) -> Result<(), TracedInterpreterError> {
        self.maybe_create_default_array(&array_name, index.len())?;
        let array = self.arrays.get_mut(array_name).unwrap();
        array.set(index, value)
    }

    fn evaluate_value_at_array_index(
        &mut self,
        array_name: Rc<String>,
    ) -> Result<Value, TracedInterpreterError> {
        let index = self.parse_array_index()?;

        self.maybe_create_default_array(&array_name, index.len())?;
        let array = self.arrays.get(&array_name).unwrap();

        Ok(array.get(&index)?)
    }

    fn evaluate_expression_term(&mut self) -> Result<Value, TracedInterpreterError> {
        match self.program.next_unwrapped_token()? {
            Token::StringLiteral(string) => Ok(string.into()),
            Token::NumericLiteral(number) => Ok(number.into()),
            Token::Symbol(symbol) => {
                let is_array_or_function_call =
                    self.program.peek_next_token() == Some(Token::LeftParen);
                if is_array_or_function_call {
                    if let Some(value) = self.evaluate_function_call(symbol.as_str())? {
                        Ok(value)
                    } else {
                        self.evaluate_value_at_array_index(symbol)
                    }
                } else if let Some(value) = self.variables.get(&symbol) {
                    Ok(value.clone())
                } else {
                    // TODO: It'd be nice to at least log a warning or something here, since
                    //       this can be a notorious source of bugs.
                    Ok(Value::default_for_variable(symbol.as_str()))
                }
            }
            _ => Err(SyntaxError::UnexpectedToken.into()),
        }
    }

    fn evaluate_parenthesized_expression(&mut self) -> Result<Value, TracedInterpreterError> {
        if self.program.accept_next_token(Token::LeftParen) {
            let value = self.evaluate_expression()?;
            self.program.expect_next_token(Token::RightParen)?;
            Ok(value)
        } else {
            self.evaluate_expression_term()
        }
    }

    fn evaluate_unary_plus_or_minus(&mut self) -> Result<Value, TracedInterpreterError> {
        let maybe_plus_or_minus = self.program.try_next_token(PlusOrMinusOp::from_token);

        let value = self.evaluate_parenthesized_expression()?;

        if let Some(plus_or_minus) = maybe_plus_or_minus {
            Ok(plus_or_minus.evaluate_unary(value)?)
        } else {
            Ok(value)
        }
    }

    fn evaluate_multiply_or_divide_expression(&mut self) -> Result<Value, TracedInterpreterError> {
        let mut value = self.evaluate_unary_plus_or_minus()?;

        while let Some(op) = self.program.try_next_token(MultiplyOrDivideOp::from_token) {
            let second_operand = self.evaluate_unary_plus_or_minus()?;
            value = op.evaluate(&value, &second_operand)?;
        }

        Ok(value)
    }

    fn evaluate_plus_or_minus_expression(&mut self) -> Result<Value, TracedInterpreterError> {
        let mut value = self.evaluate_multiply_or_divide_expression()?;

        while let Some(plus_or_minus) = self.program.try_next_token(PlusOrMinusOp::from_token) {
            let second_operand = self.evaluate_multiply_or_divide_expression()?;
            value = plus_or_minus.evaluate_binary(&value, &second_operand)?;
        }

        Ok(value)
    }

    fn evaluate_equality_expression(&mut self) -> Result<Value, TracedInterpreterError> {
        let mut value = self.evaluate_plus_or_minus_expression()?;

        while let Some(equality_op) = self.program.try_next_token(EqualityOp::from_token) {
            let second_operand = self.evaluate_plus_or_minus_expression()?;
            value = equality_op.evaluate(&value, &second_operand)?;
        }

        Ok(value)
    }

    fn evaluate_expression(&mut self) -> Result<Value, TracedInterpreterError> {
        self.evaluate_equality_expression()
    }

    fn evaluate_if_statement(&mut self) -> Result<(), TracedInterpreterError> {
        let conditional_value = self.evaluate_expression()?;
        self.program.expect_next_token(Token::Then)?;
        // Note that Applesoft BASIC doesn't seem to support ELSE,
        // but it's used in Tim Hartnell's book. We'll support very simple
        // cases; see the test suite for details.
        if conditional_value.to_bool() {
            // Evaluate the "then" clause.
            self.evaluate_statement()?;
            if self.program.peek_next_token() == Some(Token::Else) {
                // Skip the else clause, and anything else on this line.
                self.program.discard_remaining_tokens();
            }
            Ok(())
        } else {
            // Skip past the "then" clause. If we encounter a colon, ignore
            // the rest of the line, but if we encounter an "else", evaluate
            // everything after it.
            while let Some(token) = self.program.next_token() {
                match token {
                    Token::Colon => {
                        self.program.discard_remaining_tokens();
                    }
                    Token::Else => {
                        self.evaluate_statement()?;
                        return Ok(());
                    }
                    _ => {}
                }
            }
            Ok(())
        }
    }

    fn assign_value(
        &mut self,
        lvalue: LValue,
        rvalue: Value,
    ) -> Result<(), TracedInterpreterError> {
        rvalue.validate_type_matches_variable_name(lvalue.symbol_name.as_str())?;

        match lvalue.array_index {
            Some(index) => {
                self.set_value_at_array_index(&lvalue.symbol_name, &index, rvalue)?;
            }
            None => {
                self.variables.insert(lvalue.symbol_name, rvalue);
            }
        }

        Ok(())
    }

    fn evaluate_assignment_statement(
        &mut self,
        symbol_name: Rc<String>,
    ) -> Result<(), TracedInterpreterError> {
        let lvalue = LValue {
            symbol_name,
            array_index: self.parse_optional_array_index()?,
        };
        self.program.expect_next_token(Token::Equals)?;
        let value = self.evaluate_expression()?;
        self.assign_value(lvalue, value)?;
        Ok(())
    }

    fn parse_lvalue(&mut self) -> Result<LValue, TracedInterpreterError> {
        let Some(Token::Symbol(symbol_name)) = self.program.next_token() else {
            return Err(SyntaxError::UnexpectedToken.into());
        };
        let array_index = self.parse_optional_array_index()?;
        Ok(LValue {
            symbol_name,
            array_index,
        })
    }

    fn evaluate_read_statement(&mut self) -> Result<(), TracedInterpreterError> {
        // TODO: Support multiple comma-separated items.
        let lvalue = self.parse_lvalue()?;
        let Some(element) = self.program.next_data_element() else {
            return Err(InterpreterError::OutOfData.into());
        };
        let value = Value::coerce_from_data_element(lvalue.symbol_name.as_str(), &element)?;
        self.assign_value(lvalue, value)?;
        Ok(())
    }

    fn rewind_program_and_await_input(&mut self) {
        // We need to rewind to before the INPUT token, so that when we resume
        // execution after input has been retrieved, we will get back to this
        // point in the code. This is a hack, but I want to be able to run this
        // in async contexts without having to explicitly make every single part
        // of this interpreter use async/await.
        self.program.rewind_before_token(Token::Input);
        self.state = InterpreterState::AwaitingInput;
    }

    fn evaluate_input_statement(&mut self) -> Result<(), TracedInterpreterError> {
        if let Some(input) = self.input.take() {
            // TODO: Support multiple comma-separated items.
            let Some(Token::Symbol(symbol)) = self.program.next_token() else {
                return Err(SyntaxError::UnexpectedToken.into());
            };
            let (data, bytes_read) = parse_data_until_colon(input.as_str());
            // We're guaranteed to have at least one item in here, even if the input was an empty string.
            let first_element = &data[0];
            let has_excess_data = data.len() > 1 || bytes_read < input.len();
            match Value::coerce_from_data_element(symbol.as_str(), first_element) {
                Ok(value) => {
                    self.variables.insert(symbol, value);
                    if has_excess_data {
                        self.output.push("EXTRA IGNORED\n".to_string());
                    }
                    Ok(())
                }
                Err(TracedInterpreterError {
                    error: InterpreterError::DataTypeMismatch,
                    ..
                }) => {
                    self.output.push("REENTER\n".to_string());
                    self.rewind_program_and_await_input();
                    Ok(())
                }
                Err(err) => Err(err),
            }
        } else {
            self.rewind_program_and_await_input();
            Ok(())
        }
    }

    fn evaluate_print_statement(&mut self) -> Result<(), TracedInterpreterError> {
        let mut ends_with_semicolon = false;
        while let Some(token) = self.program.peek_next_token() {
            match token {
                Token::Colon | Token::Else => break,
                Token::Semicolon => {
                    // Semicolons in Applesoft BASIC are very weird, they can be interspersed
                    // throughout a PRINT statement and appear to do nothing, unless they're at
                    // the end, in which case there won't be a newline at the end of the output.
                    ends_with_semicolon = true;
                    self.program.next_token().unwrap();
                }
                _ => {
                    ends_with_semicolon = false;
                    match self.evaluate_expression()? {
                        Value::String(string) => {
                            self.output.push(string.to_string());
                        }
                        Value::Number(number) => {
                            self.output.push(format!("{}", number));
                        }
                    }
                }
            }
        }
        if !ends_with_semicolon {
            self.output.push(String::from("\n"));
        }
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
        let from_number: f64 = from_value.try_into()?;
        self.program.expect_next_token(Token::To)?;
        let to_value = self.evaluate_expression()?;
        let to_number: f64 = to_value.try_into()?;

        let step_number = if self.program.accept_next_token(Token::Step) {
            self.evaluate_expression()?.try_into()?
        } else {
            1.0
        };

        self.program
            .start_loop(symbol.clone(), to_number, step_number);
        self.variables.insert(symbol, from_number.into());
        Ok(())
    }

    fn evaluate_next_statement(&mut self) -> Result<(), TracedInterpreterError> {
        let Some(Token::Symbol(symbol)) = self.program.next_token() else {
            return Err(SyntaxError::UnexpectedToken.into());
        };
        let Some(current_value) = self.variables.get(&symbol) else {
            return Err(InterpreterError::NextWithoutFor.into());
        };
        let current_number: f64 = current_value.clone().try_into()?;
        let new_number = self.program.end_loop(symbol.clone(), current_number)?;
        self.variables.insert(symbol, new_number.into());
        Ok(())
    }

    fn evaluate_statement(&mut self) -> Result<(), TracedInterpreterError> {
        match self.program.next_token() {
            Some(Token::Print) => self.evaluate_print_statement(),
            Some(Token::Input) => self.evaluate_input_statement(),
            Some(Token::If) => self.evaluate_if_statement(),
            Some(Token::Goto) => self.evaluate_goto_statement(),
            Some(Token::Gosub) => self.evaluate_gosub_statement(),
            Some(Token::Return) => self.program.return_to_last_gosub(),
            Some(Token::End) => Ok(self.program.end()),
            Some(Token::For) => self.evaluate_for_statement(),
            Some(Token::Next) => self.evaluate_next_statement(),
            Some(Token::Restore) => Ok(self.program.reset_data_cursor()),
            Some(Token::Read) => self.evaluate_read_statement(),
            Some(Token::Remark(_)) => Ok(()),
            Some(Token::Colon) => Ok(()),
            Some(Token::Data(_)) => Ok(()),
            Some(Token::Symbol(symbol)) => self.evaluate_assignment_statement(symbol),
            Some(_) => Err(SyntaxError::UnexpectedToken.into()),
            None => Ok(()),
        }
    }

    fn run(&mut self) -> Result<(), TracedInterpreterError> {
        self.state = InterpreterState::Running;
        if self.program.has_next_token() {
            self.evaluate_statement()?;
        }
        if !self.program.has_next_token() {
            if !self.program.next_line() {
                self.state = InterpreterState::Idle;
            }
        }

        Ok(())
    }

    pub fn get_state(&self) -> InterpreterState {
        self.state
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

    fn postprocess_result<T>(
        &mut self,
        result: Result<T, TracedInterpreterError>,
    ) -> Result<T, TracedInterpreterError> {
        if let Err(mut err) = result {
            let line_number = match err.error {
                InterpreterError::DataTypeMismatch => self.program.get_data_line_number(),
                _ => self.program.get_line_number(),
            };
            if let Some(line_number) = line_number {
                err.set_line_number(line_number);
            }
            self.state = InterpreterState::Idle;
            Err(err)
        } else {
            result
        }
    }

    pub fn provide_input(&mut self, input: String) {
        assert_eq!(self.state, InterpreterState::AwaitingInput);
        self.input = Some(input);
        self.state = InterpreterState::Running;
    }

    pub fn continue_evaluating(&mut self) -> Result<(), TracedInterpreterError> {
        assert_eq!(self.state, InterpreterState::Running);
        let result = self.run();
        self.postprocess_result(result)
    }

    /// Start evaluating the given line of code.
    ///
    /// Note that this is expected to be a *line*, i.e. it shouldn't contain
    /// any newlines (if it does, a syntax error will be raised).
    ///
    /// This only *starts* evaluation. In order to keep running it to completion,
    /// the caller must also call `continue_evaluating` for as long as the
    /// interpreter's state is `InterpreterState::Running`.
    pub fn start_evaluating<T: AsRef<str>>(
        &mut self,
        line: T,
    ) -> Result<(), TracedInterpreterError> {
        let result = self.evaluate_impl(line);
        self.postprocess_result(result)
    }

    /// Stop any evaluation and return the line number we
    /// were evaluating at the time of stopping, if any.
    pub fn stop_evaluating(&mut self) -> Option<u64> {
        let line_number = self.program.get_line_number();
        self.program.set_and_goto_immediate_line(vec![]);
        self.run().unwrap();
        line_number
    }

    fn evaluate_impl<T: AsRef<str>>(&mut self, line: T) -> Result<(), TracedInterpreterError> {
        assert_eq!(self.state, InterpreterState::Idle);
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

#[cfg(test)]
mod tests {
    use crate::{
        interpreter_error::{OutOfMemoryError, TracedInterpreterError},
        syntax_error::SyntaxError,
    };

    use super::{Interpreter, InterpreterError, InterpreterState};

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

    fn assert_eval_error(line: &'static str, expected: InterpreterError) {
        let mut interpreter = Interpreter::new();
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
        let mut interpreter = Interpreter::new();
        let output = eval_line_and_expect_success(&mut interpreter, line);
        assert_eq!(output, expected, "evaluating '{}'", line);
    }

    fn assert_program_actions(program: &'static str, actions: &[Action]) {
        let mut interpreter = Interpreter::new();
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
                    Ok(_) => interpreter
                        .get_and_clear_output_buffer()
                        .unwrap_or_default(),
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
        let mut interpreter = Interpreter::new();
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

    fn eval_line_and_expect_success<T: AsRef<str>>(
        interpreter: &mut Interpreter,
        line: T,
    ) -> String {
        match evaluate_line_while_running(interpreter, line.as_ref()) {
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

        assert_eval_output("print 1 <> 2", "1\n");
        assert_eval_output("print 1 <> 1", "0\n");
    }

    #[test]
    fn print_works_with_chained_numeric_equality_expressions() {
        assert_eval_output("print 5 > 4 > 3", "0\n");
        assert_eval_output("print 5 > 4 = 1", "1\n");
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
    fn rnd_works() {
        fastrand::seed(0);
        assert_eval_output(
            "for i = 1 to 3:print int(rnd(1) * 50):next i",
            "3\n40\n19\n",
        );

        assert_eval_error("print rnd(-1)", InterpreterError::Unimplemented);
        assert_eval_error("print rnd(0)", InterpreterError::Unimplemented);
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
        assert_eval_output("if 0 then print \"hi\":print:kaboom", "");
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

    #[ignore]
    #[test]
    fn input_works_with_arrays() {
        // TODO: Make this pass!
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
}
