use std::{collections::HashMap, rc::Rc};

use crate::{
    interpreter_error::{InterpreterError, TracedInterpreterError},
    program::Program,
    syntax_error::SyntaxError,
    tokenizer::{Token, Tokenizer},
};

#[derive(Debug, PartialEq, Copy, Clone)]
pub enum InterpreterState {
    Idle,
    Running,
}

#[derive(Debug, Clone, PartialEq)]
enum Value {
    String(Rc<String>),
    Number(f64),
}

impl Value {
    fn to_bool(&self) -> bool {
        match self {
            Value::String(string) => !string.is_empty(),
            Value::Number(number) => *number != 0.0,
        }
    }
}

impl TryFrom<Value> for f64 {
    type Error = TracedInterpreterError;

    fn try_from(value: Value) -> Result<Self, Self::Error> {
        if let Value::Number(number) = value {
            Ok(number)
        } else {
            Err(InterpreterError::TypeMismatch.into())
        }
    }
}

#[derive(PartialEq)]
enum PlusOrMinusOp {
    Plus,
    Minus,
}

impl PlusOrMinusOp {
    // I considered TryFrom here but it required an associated Error type
    // and I just wanted to use Option.
    fn from_token(token: Token) -> Option<Self> {
        match &token {
            Token::Plus => Some(PlusOrMinusOp::Plus),
            Token::Minus => Some(PlusOrMinusOp::Minus),
            _ => None,
        }
    }

    fn evaluate_unary(&self, value: Value) -> Result<Value, TracedInterpreterError> {
        let mut number: f64 = value.try_into()?;

        if self == &PlusOrMinusOp::Minus {
            number *= -1.0;
        }

        Ok(number.into())
    }

    fn evaluate_binary(
        &self,
        left_side: &Value,
        right_side: &Value,
    ) -> Result<Value, TracedInterpreterError> {
        let result = match (left_side, right_side) {
            (Value::Number(l), Value::Number(r)) => match self {
                PlusOrMinusOp::Plus => l + r,
                PlusOrMinusOp::Minus => l - r,
            },
            _ => return Err(InterpreterError::TypeMismatch.into()),
        };
        Ok(result.into())
    }
}

enum MultiplyOrDivideOp {
    Multiply,
    Divide,
}

impl MultiplyOrDivideOp {
    // I considered TryFrom here but it required an associated Error type
    // and I just wanted to use Option.
    fn from_token(token: Token) -> Option<Self> {
        match token {
            Token::Multiply => Some(MultiplyOrDivideOp::Multiply),
            Token::Divide => Some(MultiplyOrDivideOp::Divide),
            _ => None,
        }
    }

    fn evaluate(
        &self,
        left_side: &Value,
        right_side: &Value,
    ) -> Result<Value, TracedInterpreterError> {
        let result = match (left_side, right_side) {
            (Value::Number(l), Value::Number(r)) => match self {
                MultiplyOrDivideOp::Multiply => l * r,
                MultiplyOrDivideOp::Divide => l / r,
            },
            _ => return Err(InterpreterError::TypeMismatch.into()),
        };
        Ok(result.into())
    }
}

enum EqualityOp {
    EqualTo,
    LessThan,
    GreaterThan,
    NotEqualTo,
}

impl EqualityOp {
    // I considered TryFrom here but it required an associated Error type
    // and I just wanted to use Option.
    fn from_token(token: Token) -> Option<Self> {
        match token {
            Token::Equals => Some(EqualityOp::EqualTo),
            Token::LessThan => Some(EqualityOp::LessThan),
            Token::GreaterThan => Some(EqualityOp::GreaterThan),
            Token::NotEquals => Some(EqualityOp::NotEqualTo),
            _ => None,
        }
    }

    fn evaluate_partial_ord<T: PartialOrd>(&self, left_side: T, right_side: T) -> bool {
        match self {
            EqualityOp::EqualTo => left_side == right_side,
            EqualityOp::LessThan => left_side < right_side,
            EqualityOp::GreaterThan => left_side > right_side,
            EqualityOp::NotEqualTo => left_side != right_side,
        }
    }

    fn evaluate(
        &self,
        left_side: &Value,
        right_side: &Value,
    ) -> Result<Value, TracedInterpreterError> {
        let result = match (left_side, right_side) {
            (Value::String(l), Value::String(r)) => self.evaluate_partial_ord(l, r),
            (Value::Number(l), Value::Number(r)) => self.evaluate_partial_ord(l, r),
            _ => return Err(InterpreterError::TypeMismatch.into()),
        };
        // This is how Applesoft BASIC evaluates equality expressions.
        if result {
            Ok(1.0.into())
        } else {
            Ok(0.0.into())
        }
    }
}

impl Value {
    fn default_for_variable<T: AsRef<str>>(variable_name: T) -> Self {
        if variable_name.as_ref().ends_with('$') {
            String::default().into()
        } else {
            f64::default().into()
        }
    }

    fn validate_type_matches_variable_name<T: AsRef<str>>(
        &self,
        variable_name: T,
    ) -> Result<(), TracedInterpreterError> {
        if variable_name.as_ref().ends_with('$') {
            match self {
                Value::String(_) => Ok(()),
                Value::Number(_) => Err(InterpreterError::TypeMismatch.into()),
            }
        } else {
            match self {
                Value::String(_) => Err(InterpreterError::TypeMismatch.into()),
                Value::Number(_) => Ok(()),
            }
        }
    }
}

impl From<String> for Value {
    fn from(value: String) -> Self {
        Value::String(Rc::new(value))
    }
}

impl From<Rc<String>> for Value {
    fn from(value: Rc<String>) -> Self {
        Value::String(value.clone())
    }
}

impl From<f64> for Value {
    fn from(value: f64) -> Self {
        Value::Number(value)
    }
}

#[derive(Debug)]
pub struct Interpreter {
    output: Vec<String>,
    program: Program,
    variables: HashMap<Rc<String>, Value>,
    state: InterpreterState,
}

impl Interpreter {
    pub fn new() -> Self {
        Interpreter {
            output: vec![],
            program: Default::default(),
            variables: HashMap::new(),
            state: InterpreterState::Idle,
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
            Token::StringLiteral(string) => Ok(string.into()),
            Token::NumericLiteral(number) => Ok(number.into()),
            Token::Symbol(variable) => {
                if let Some(value) = self.variables.get(&variable) {
                    Ok(value.clone())
                } else {
                    // TODO: It'd be nice to at least log a warning or something here, since
                    //       this can be a notorious source of bugs.
                    Ok(Value::default_for_variable(variable.as_str()))
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
        let value = self.evaluate_unary_plus_or_minus()?;

        if let Some(op) = self.program.try_next_token(MultiplyOrDivideOp::from_token) {
            let second_operand = self.evaluate_multiply_or_divide_expression()?;
            return op.evaluate(&value, &second_operand);
        }

        Ok(value)
    }

    fn evaluate_plus_or_minus_expression(&mut self) -> Result<Value, TracedInterpreterError> {
        let value = self.evaluate_multiply_or_divide_expression()?;

        if let Some(plus_or_minus) = self.program.try_next_token(PlusOrMinusOp::from_token) {
            let second_operand = self.evaluate_plus_or_minus_expression()?;
            let result = plus_or_minus.evaluate_binary(&value, &second_operand)?;
            Ok(result.into())
        } else {
            Ok(value)
        }
    }

    fn evaluate_expression(&mut self) -> Result<Value, TracedInterpreterError> {
        let value = self.evaluate_plus_or_minus_expression()?;

        if let Some(equality_op) = self.program.try_next_token(EqualityOp::from_token) {
            let second_operand = self.evaluate_expression()?;
            return equality_op.evaluate(&value, &second_operand);
        }

        Ok(value)
    }

    fn evaluate_if_statement(&mut self) -> Result<(), TracedInterpreterError> {
        let conditional_value = self.evaluate_expression()?;
        self.program.expect_next_token(Token::Then)?;
        // TODO: It would be nice to support ELSE somehow, even though
        // AppleSoft basic doesn't really seem to. Tim Hartnell's
        // book seems to include ELSE clauses only in the form of line
        // numbers, e.g. `IF X THEN 100 ELSE 200`, which seems like a
        // reasonable compromise.
        if conditional_value.to_bool() {
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
        value.validate_type_matches_variable_name(variable.as_str())?;
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
        let from_number: f64 = from_value.try_into()?;
        self.program.expect_next_token(Token::To)?;
        let to_value = self.evaluate_expression()?;
        let to_number: f64 = to_value.try_into()?;

        // TODO: Add support for STEP.

        self.program.start_loop(symbol.clone(), to_number);
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
            if let Some(line_number) = self.program.get_line_number() {
                err.set_line_number(line_number);
            }
            self.state = InterpreterState::Idle;
            Err(err)
        } else {
            result
        }
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
    use crate::interpreter_error::{OutOfMemoryError, TracedInterpreterError};

    use super::{Interpreter, InterpreterError, InterpreterState};

    fn evaluate_until_idle(
        interpreter: &mut Interpreter,
        line: &str,
    ) -> Result<(), TracedInterpreterError> {
        interpreter.start_evaluating(line)?;
        while interpreter.get_state() == InterpreterState::Running {
            interpreter.continue_evaluating()?;
        }
        Ok(())
    }

    fn assert_eval_error(line: &'static str, expected: InterpreterError) {
        let mut interpreter = Interpreter::new();
        match evaluate_until_idle(&mut interpreter, line) {
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
        match evaluate_until_idle(&mut interpreter, "run") {
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
        match evaluate_until_idle(interpreter, line.as_ref()) {
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
}
