use std::collections::{BTreeSet, HashMap};

use crate::{
    interpreter_error::TracedInterpreterError, syntax_error::SyntaxError, tokenizer::Token,
};

#[derive(Debug, Default)]
enum ProgramLine {
    #[default]
    Immediate,
    Line(u64),
}

#[derive(Debug, Default)]
struct ProgramLocation {
    line: ProgramLine,
    token_index: usize,
}

#[derive(Debug, Default)]
pub struct Program {
    numbered_lines: HashMap<u64, Vec<Token>>,
    immediate_line: Vec<Token>,
    sorted_line_numbers: BTreeSet<u64>,
    location: ProgramLocation,
}

impl Program {
    pub fn set_and_goto_immediate_line(&mut self, tokens: Vec<Token>) {
        self.immediate_line = tokens;
        self.location = Default::default();
    }

    pub fn goto_first_numbered_line(&mut self) {
        if let Some(&first_line) = self.sorted_line_numbers.first() {
            self.location = ProgramLocation {
                line: ProgramLine::Line(first_line),
                token_index: 0,
            };
        } else {
            self.set_and_goto_immediate_line(vec![]);
        };
    }

    pub fn next_line(&mut self) -> bool {
        match self.location.line {
            ProgramLine::Immediate => {
                // There is nowhere to go, don't do anything.
                false
            }
            ProgramLine::Line(current_line) => {
                if let Some(&next_line) = self.sorted_line_numbers.range(current_line + 1..).next()
                {
                    self.location = ProgramLocation {
                        line: ProgramLine::Line(next_line),
                        token_index: 0,
                    };
                    true
                } else {
                    false
                }
            }
        }
    }

    pub fn list(&self) -> Vec<String> {
        let mut lines: Vec<String> = Vec::with_capacity(self.numbered_lines.len());

        for line_number in &self.sorted_line_numbers {
            let line = self.numbered_lines.get(line_number).unwrap();
            // TODO: Actually show the source code, not a debug list of tokens!
            let line_source = format!("{} {:?}\n", line_number, line);
            lines.push(line_source);
        }

        lines
    }

    pub fn set_numbered_line(&mut self, line_number: u64, tokens: Vec<Token>) {
        self.sorted_line_numbers.insert(line_number);
        self.numbered_lines.insert(line_number, tokens);
    }

    fn tokens(&self) -> &Vec<Token> {
        match self.location.line {
            ProgramLine::Immediate => &self.immediate_line,
            ProgramLine::Line(number) => self.numbered_lines.get(&number).unwrap(),
        }
    }

    /// Returns whether we have any more tokens in the stream.
    pub fn has_next_token(&self) -> bool {
        self.peek_next_token().is_some()
    }

    /// Return the next token in the stream, if it exists,
    /// but don't advance our position in it.
    pub fn peek_next_token(&self) -> Option<Token> {
        self.tokens().get(self.location.token_index).cloned()
    }

    /// Return the next token in the stream, if it exists,
    /// and advance our position in it.
    pub fn next_token(&mut self) -> Option<Token> {
        let next = self.peek_next_token();
        if next.is_some() {
            self.location.token_index += 1;
        }
        next
    }

    /// Return the next token in the stream, advancing our
    /// position in it.  If there are no more tokens, return an error.
    pub fn next_unwrapped_token(&mut self) -> Result<Token, TracedInterpreterError> {
        unwrap_token(self.next_token())
    }

    /// Expect the next token to be the given token, and advance our position
    /// in the stream. If the next token is not what we expect it to be,
    /// return an error.
    pub fn expect_next_token(&mut self, expected: Token) -> Result<(), TracedInterpreterError> {
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
    pub fn consume_next_token(&mut self) {
        self.tokens().get(self.location.token_index).unwrap();
        self.location.token_index += 1;
    }

    /// Throw away any remaining tokens.
    pub fn discard_remaining_tokens(&mut self) {
        self.location.token_index = self.tokens().len();
    }
}

fn unwrap_token(token: Option<Token>) -> Result<Token, TracedInterpreterError> {
    match token {
        Some(token) => Ok(token),
        None => Err(SyntaxError::UnexpectedEndOfInput.into()),
    }
}
