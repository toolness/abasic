use std::{error::Error, fmt::Display};

use crate::{
    syntax_error::SyntaxError,
    tokenizer::{Token, Tokenizer},
};

#[derive(Debug, PartialEq)]
pub enum InterpreterError {
    SyntaxError(SyntaxError),
}

impl Error for InterpreterError {}

impl Display for InterpreterError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "INTERPRETER ERROR ({:?})", self)
    }
}

pub struct Interpreter {
    output: Vec<String>,
}

impl Interpreter {
    pub fn new() -> Self {
        Interpreter { output: vec![] }
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

    fn evaluate_tokens(&mut self, tokens: Vec<Token>) -> Result<(), InterpreterError> {
        for token in tokens {
            // TODO: Actually evaluate!
            self.output.push(format!("Token: {:?}\n", token));
        }
        Ok(())
    }

    pub fn evaluate<T: AsRef<str>>(&mut self, line: T) -> Result<(), InterpreterError> {
        let tokenizer = Tokenizer::new(line);
        let mut tokens: Vec<Token> = vec![];
        for token_result in tokenizer {
            match token_result {
                Ok(token) => {
                    tokens.push(token);
                }
                Err(err) => {
                    return Err(InterpreterError::SyntaxError(err));
                }
            }
        }
        self.evaluate_tokens(tokens)
    }
}
