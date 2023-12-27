use std::{error::Error, fmt::Display};

use crate::{
    syntax_error::SyntaxError,
    tokenizer::{Token, Tokenizer},
};

enum Value {
    String(String),
    Number(f64),
}

#[derive(Debug, PartialEq)]
pub enum InterpreterError {
    SyntaxError(SyntaxError),
}

impl InterpreterError {
    pub fn unexpected_token<T>() -> Result<T, InterpreterError> {
        Err(InterpreterError::SyntaxError(SyntaxError::UnexpectedToken))
    }
}

impl Error for InterpreterError {}

impl Display for InterpreterError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "INTERPRETER ERROR ({:?})", self)
    }
}

pub struct Interpreter {
    output: Vec<String>,
    tokens: Vec<Token>,
    tokens_index: usize,
}

impl Interpreter {
    pub fn new() -> Self {
        Interpreter {
            output: vec![],
            tokens: vec![],
            tokens_index: 0,
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

    fn peek_next_token(&self) -> Option<Token> {
        self.tokens.get(self.tokens_index).cloned()
    }

    fn next_token(&mut self) -> Option<Token> {
        let next = self.peek_next_token();
        if next.is_some() {
            self.tokens_index += 1;
        }
        next
    }

    fn next_unwrapped_token(&mut self) -> Result<Token, InterpreterError> {
        match self.next_token() {
            Some(token) => Ok(token),
            None => Err(InterpreterError::SyntaxError(
                SyntaxError::UnexpectedEndOfInput,
            )),
        }
    }

    fn evaluate_expression(&mut self) -> Result<Value, InterpreterError> {
        match self.next_unwrapped_token()? {
            Token::StringLiteral(string) => Ok(Value::String(string.to_string())),
            Token::NumericLiteral(number) => Ok(Value::Number(number)),
            _ => InterpreterError::unexpected_token(),
        }
    }

    fn evaluate_print_statement(&mut self) -> Result<(), InterpreterError> {
        while self.has_next_token() {
            match self.evaluate_expression()? {
                Value::String(string) => {
                    self.output.push(string);
                }
                Value::Number(number) => {
                    self.output.push(format!("{}", number));
                }
            }
        }
        self.output.push(String::from("\n"));
        Ok(())
    }

    fn evaluate_statement(&mut self) -> Result<(), InterpreterError> {
        match self.next_unwrapped_token()? {
            Token::Print => self.evaluate_print_statement(),
            _ => InterpreterError::unexpected_token(),
        }
    }

    pub fn evaluate<T: AsRef<str>>(&mut self, line: T) -> Result<(), InterpreterError> {
        let tokenizer = Tokenizer::new(line);
        self.tokens.clear();
        self.tokens_index = 0;
        for token_result in tokenizer {
            match token_result {
                Ok(token) => {
                    self.tokens.push(token);
                }
                Err(err) => {
                    return Err(InterpreterError::SyntaxError(err));
                }
            }
        }
        if self.has_next_token() {
            self.evaluate_statement()
        } else {
            Ok(())
        }
    }
}

#[cfg(test)]
mod tests {
    use super::Interpreter;

    fn assert_eval_output(line: &'static str, expected: &'static str) {
        let mut interpreter = Interpreter::new();
        let output = match interpreter.evaluate(line) {
            Ok(_) => interpreter
                .get_and_clear_output_buffer()
                .unwrap_or_default(),
            Err(err) => {
                panic!(
                    "expected '{}' to evaluate successfully but got {:?}",
                    line, err
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
}
