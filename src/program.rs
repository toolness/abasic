use std::{
    collections::{BTreeSet, HashMap},
    rc::Rc,
};

use crate::{
    interpreter_error::{InterpreterError, OutOfMemoryError, TracedInterpreterError},
    syntax_error::SyntaxError,
    tokenizer::Token,
};

const STACK_LIMIT: usize = 256;

#[derive(Debug, Default, Copy, Clone)]
pub enum ProgramLine {
    #[default]
    Immediate,
    Line(u64),
}

#[derive(Debug, Default, Copy, Clone)]
struct ProgramLocation {
    line: ProgramLine,
    token_index: usize,
}

#[derive(Debug, Default)]
struct LoopInfo {
    location: ProgramLocation,
    symbol: Rc<String>,
    to_value: f64,
}

#[derive(Debug, Default)]
pub struct Program {
    numbered_lines: HashMap<u64, Vec<Token>>,
    immediate_line: Vec<Token>,
    sorted_line_numbers: BTreeSet<u64>,
    location: ProgramLocation,
    stack: Vec<ProgramLocation>,
    loop_stack: Vec<LoopInfo>,
}

impl Program {
    /// Set the content of the "immediate" line (i.e., the line that is being
    /// evaluated by the interpreter and has no line number) and go there.
    ///
    /// Resets the stack.
    pub fn set_and_goto_immediate_line(&mut self, tokens: Vec<Token>) {
        self.stack.clear();
        self.immediate_line = tokens;
        self.location = Default::default();
    }

    pub fn start_loop(&mut self, symbol: Rc<String>, to_value: f64) {
        // TODO: We might want to check the loop stack to see if there's already
        // a loop with the same symbol in-progress, and if so, delete everything
        // from that point to the top of the stack. This might be necessary because
        // loops in basic don't have any kind of "break" statement, and we don't want
        // to memory leak if the inside of a loop uses a goto.
        //
        // Other notes...
        //
        // The following works in Applesoft BASIC (it doesn't in our implementation):
        //
        //   10 for i = 1 to 3
        //   20 for j = 1 to 4
        //   30 print "i" i "j" j
        //   40 next i
        //
        // It will print "i1j1", "i2j1", "i3j1" and then exit.
        //
        // However, adding a "50 next j" at the end of the program *will* fail in
        // Applesoft BASIC, with a "NEXT WITHOUT FOR ERROR IN 50".
        //
        // The following doesn't run out of memory in Applesoft BASIC--it just
        // loops infinitely--while it will leak memory in our implementation:
        //
        //   10 for i = 1 to 3
        //   20 goto 10
        self.loop_stack.push(LoopInfo {
            location: self.location,
            symbol,
            to_value,
        })
    }

    pub fn end_loop(
        &mut self,
        symbol: Rc<String>,
        current_value: f64,
    ) -> Result<f64, TracedInterpreterError> {
        let Some(loop_info) = self.loop_stack.pop() else {
            return Err(InterpreterError::NextWithoutFor.into());
        };

        if loop_info.symbol != symbol {
            return Err(InterpreterError::NextWithoutFor.into());
        }

        let new_value = current_value + 1.0;

        if new_value <= loop_info.to_value {
            self.location = loop_info.location;
            self.loop_stack.push(loop_info);
        }

        Ok(new_value)
    }

    /// Go to the first numbered line. Resets the stack.
    pub fn goto_first_numbered_line(&mut self) {
        if let Some(&first_line) = self.sorted_line_numbers.first() {
            self.stack.clear();
            self.location = ProgramLocation {
                line: ProgramLine::Line(first_line),
                token_index: 0,
            };
        } else {
            // Applesoft basic just does nothing when RUN is executed
            // in an empty program, so we'll do that too.
            self.end();
        };
    }

    pub fn goto_line_number(&mut self, line_number: u64) -> Result<(), TracedInterpreterError> {
        if self.sorted_line_numbers.contains(&line_number) {
            self.location = ProgramLocation {
                line: ProgramLine::Line(line_number),
                token_index: 0,
            };
            Ok(())
        } else {
            Err(InterpreterError::UndefinedStatement.into())
        }
    }

    pub fn gosub_line_number(&mut self, line_number: u64) -> Result<(), TracedInterpreterError> {
        if self.stack.len() == STACK_LIMIT {
            return Err(OutOfMemoryError::StackOverflow.into());
        }
        let return_location = self.location;
        self.goto_line_number(line_number)?;
        self.stack.push(return_location);
        Ok(())
    }

    pub fn return_to_last_gosub(&mut self) -> Result<(), TracedInterpreterError> {
        let Some(return_location) = self.stack.pop() else {
            return Err(InterpreterError::ReturnWithoutGosub.into());
        };
        self.location = return_location;
        Ok(())
    }

    pub fn end(&mut self) {
        self.set_and_goto_immediate_line(vec![]);
    }

    /// Returns the line number currently being evaluated.
    pub fn get_line_number(&self) -> Option<u64> {
        if let ProgramLine::Line(line_number) = self.location.line {
            Some(line_number)
        } else {
            None
        }
    }

    /// Attempt to move to the next line of the program. Returns false
    /// if we're at the end of the program and there's nothing left
    /// to execute.
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
            let line = self
                .numbered_lines
                .get(line_number)
                .unwrap()
                .iter()
                .map(|token| token.to_string())
                .collect::<Vec<String>>()
                .join(" ");

            let line_source = format!("{} {}\n", line_number, line);
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

    pub fn try_next_token<T, F>(&mut self, f: F) -> Option<T>
    where
        F: FnOnce(Token) -> Option<T>,
    {
        if let Some(next_token) = self.peek_next_token() {
            if let Some(t) = f(next_token) {
                self.location.token_index += 1;
                Some(t)
            } else {
                None
            }
        } else {
            None
        }
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
