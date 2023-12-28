use std::{collections::HashMap, rc::Rc};

use crate::{
    interpreter_error::{InterpreterError, TracedInterpreterError},
    syntax_error::SyntaxError,
    tokenizer::{Token, Tokenizer},
};

#[derive(Debug, Clone)]
enum Value {
    String(String),
    Number(f64),
}

#[derive(Debug)]
pub struct Interpreter {
    output: Vec<String>,
    tokens: Vec<Token>,
    tokens_index: usize,
    variables: HashMap<Rc<String>, Value>,
}

impl Interpreter {
    pub fn new() -> Self {
        Interpreter {
            output: vec![],
            tokens: vec![],
            tokens_index: 0,
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

    fn has_next_token(&self) -> bool {
        self.peek_next_token().is_some()
    }

    /// Return the next token in the stream, if it exists,
    /// but don't advance our position in it.
    fn peek_next_token(&self) -> Option<Token> {
        self.tokens.get(self.tokens_index).cloned()
    }

    /// Return the next token in the stream, if it exists,
    /// and advance our position in it.
    fn next_token(&mut self) -> Option<Token> {
        let next = self.peek_next_token();
        if next.is_some() {
            self.tokens_index += 1;
        }
        next
    }

    /// Return the next token in the stream, advancing our
    /// position in it.  If there are no more tokens, return an error.
    fn next_unwrapped_token(&mut self) -> Result<Token, TracedInterpreterError> {
        unwrap_token(self.next_token())
    }

    fn expect_next_token(&mut self, expected: Token) -> Result<(), TracedInterpreterError> {
        if self.next_unwrapped_token()? == expected {
            Ok(())
        } else {
            Err(SyntaxError::ExpectedToken(expected).into())
        }
    }

    /// Advance to the next token in the stream, panicking if there are
    /// no more tokens. This should only be used after e.g. calling
    /// `peek_next_token` and verifying that the next token actually
    /// exists.
    fn consume_next_token(&mut self) {
        self.tokens.get(self.tokens_index).unwrap();
        self.tokens_index += 1;
    }

    fn evaluate_expression_term(&mut self) -> Result<Value, TracedInterpreterError> {
        match self.next_unwrapped_token()? {
            Token::StringLiteral(string) => Ok(Value::String(string.to_string())),
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
            _ => TracedInterpreterError::unexpected_token(),
        }
    }

    fn evaluate_plus_or_minus(&mut self) -> Option<f64> {
        if let Some(next_token) = self.peek_next_token() {
            if let Some(unary_plus_or_minus) = parse_plus_or_minus(&next_token) {
                self.consume_next_token();
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

    fn evaluate_assignment_statement(
        &mut self,
        variable: Rc<String>,
    ) -> Result<(), TracedInterpreterError> {
        self.expect_next_token(Token::Equals)?;
        let value = self.evaluate_expression()?;
        // TODO: We should only allow assigning numbers to variables that don't end
        // with `$`, and only allow assigning strings to ones that end with `$`.
        self.variables.insert(variable, value);
        Ok(())
    }

    fn evaluate_print_statement(&mut self) -> Result<(), TracedInterpreterError> {
        while let Some(token) = self.peek_next_token() {
            match token {
                Token::Colon => break,
                _ => match self.evaluate_expression()? {
                    Value::String(string) => {
                        self.output.push(string);
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

    fn evaluate_statement(&mut self) -> Result<(), TracedInterpreterError> {
        match self.next_token() {
            Some(Token::Print) => self.evaluate_print_statement(),
            Some(Token::Colon) => Ok(()),
            Some(Token::Symbol(value)) => self.evaluate_assignment_statement(value),
            Some(_) => TracedInterpreterError::unexpected_token(),
            None => Ok(()),
        }
    }

    pub fn evaluate<T: AsRef<str>>(&mut self, line: T) -> Result<(), TracedInterpreterError> {
        self.tokens = Tokenizer::new(line)
            .remaining_tokens()
            .map_err(|err| TracedInterpreterError::from(err))?;
        self.tokens_index = 0;

        while self.has_next_token() {
            self.evaluate_statement()?;
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

fn unwrap_token(token: Option<Token>) -> Result<Token, TracedInterpreterError> {
    match token {
        Some(token) => Ok(token),
        None => Err(SyntaxError::UnexpectedEndOfInput.into()),
    }
}

#[cfg(test)]
mod tests {
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
        let output = match interpreter.evaluate(line) {
            Ok(_) => interpreter
                .get_and_clear_output_buffer()
                .unwrap_or_default(),
            Err(err) => {
                panic!(
                    "expected '{}' to evaluate successfully but got {}\nIntepreter state is: {:?}",
                    line, err, interpreter
                )
            }
        };

        assert_eq!(output, expected, "evaluating '{}'", line);
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
    fn assignment_works() {
        assert_eval_output("x=1:print x", "1\n");
        assert_eval_output("X=1:print x", "1\n");
        assert_eval_output("x5=1:print x5", "1\n");
        assert_eval_output("x=1+1:print x", "2\n");
        assert_eval_output("x=1:print x + 2", "3\n");
        assert_eval_output("x=1:print x:x = x + 1:print x", "1\n2\n");
    }

    #[test]
    fn type_mismatch_error_works() {
        assert_eval_error("print -\"hi\"", InterpreterError::TypeMismatch);
        assert_eval_error("print \"hi\" - 4", InterpreterError::TypeMismatch);
        assert_eval_error("print 4 + \"hi\"", InterpreterError::TypeMismatch);
    }
}
