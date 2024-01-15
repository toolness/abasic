use std::collections::HashMap;

use crate::{
    data::{DataElement, DataIterator},
    interpreter_error::{InterpreterError, OutOfMemoryError, TracedInterpreterError},
    program_lines::ProgramLines,
    symbol::Symbol,
    syntax_error::SyntaxError,
    tokenizer::Token,
    value::Value,
    variables::Variables,
};

const STACK_LIMIT: usize = 32;

#[derive(Debug, Default, Copy, Clone, PartialEq)]
pub enum ProgramLine {
    #[default]
    Immediate,
    Line(u64),
}

#[derive(Debug)]
struct StackFrame {
    return_location: ProgramLocation,
    variables: Variables,
}

#[derive(Debug, Default, Copy, Clone)]
pub struct NumberedProgramLocation {
    line: u64,
    token_index: usize,
}

impl NumberedProgramLocation {
    pub fn new(line: u64, token_index: usize) -> Self {
        NumberedProgramLocation { line, token_index }
    }
}

impl TryFrom<ProgramLocation> for NumberedProgramLocation {
    type Error = InterpreterError;

    fn try_from(value: ProgramLocation) -> Result<Self, Self::Error> {
        match value.as_numbered() {
            Some(nloc) => Ok(nloc),
            None => Err(InterpreterError::IllegalDirect),
        }
    }
}

#[derive(Debug, Default, Copy, Clone, PartialEq)]
pub struct ProgramLocation {
    pub line: ProgramLine,
    pub token_index: usize,
}

impl ProgramLocation {
    pub fn as_numbered(&self) -> Option<NumberedProgramLocation> {
        match self.line {
            ProgramLine::Immediate => None,
            ProgramLine::Line(line) => Some(NumberedProgramLocation {
                line,
                token_index: self.token_index,
            }),
        }
    }
}

impl From<NumberedProgramLocation> for ProgramLocation {
    fn from(value: NumberedProgramLocation) -> Self {
        ProgramLocation {
            line: ProgramLine::Line(value.line),
            token_index: value.token_index,
        }
    }
}

#[derive(Debug)]
struct LoopInfo {
    location: ProgramLocation,
    symbol: Symbol,
    to_value: f64,
    step_value: f64,
}

#[derive(Debug)]
struct FunctionDefinition {
    arguments: Vec<Symbol>,
    location: NumberedProgramLocation,
}

#[derive(Debug, Default)]
pub struct Program {
    numbered_lines: ProgramLines,
    immediate_line: Vec<Token>,
    location: ProgramLocation,
    breakpoint: Option<NumberedProgramLocation>,
    stack: Vec<StackFrame>,
    loop_stack: Vec<LoopInfo>,
    data_iterator: Option<DataIterator>,
    functions: HashMap<Symbol, FunctionDefinition>,
}

impl Program {
    /// Set the content of the "immediate" line (i.e., the line that is being
    /// evaluated by the interpreter and has no line number) and go there.
    ///
    /// Resets the stack (unless we're in a breakpoint).
    pub fn set_and_goto_immediate_line(&mut self, tokens: Vec<Token>) {
        if self.breakpoint.is_none() {
            self.stack.clear();
        }
        self.immediate_line = tokens;
        self.location = Default::default();
    }

    /// Removes any loop with the given symbol, and any loops in front of it in
    /// the loop stack.
    ///
    /// Returns the loop info for the loop, or None if one wasn't found.
    ///
    /// This is necessary because loops in basic don't have any kind of "break"
    /// statement, and we don't want to memory leak if the inside of a loop uses a
    /// goto or does something else strange with flow control.
    ///
    /// Situations that work in Applesoft BASIC, which this strategy enables:
    ///
    ///   10 for i = 1 to 3
    ///   20 for j = 1 to 4
    ///   30 print "i" i "j" j
    ///   40 next i
    ///
    /// It will print "i1j1", "i2j1", "i3j1" and then exit.
    ///
    /// However, adding a "50 next j" at the end of the program *will* fail in
    /// Applesoft BASIC:
    ///
    ///   10 for i = 1 to 3
    ///   20 for j = 1 to 4
    ///   30 print "i" i "j" j
    ///   40 next i
    ///   50 next j
    ///
    /// Running this raises a "NEXT WITHOUT FOR ERROR IN 50". The reason given
    /// for this in the Applesoft II BASIC reference manual is that "when the NEXT
    /// I is encountered, all knowledge of the J-loop is lost".  This method
    /// effectively implements this "forgetting".
    ///
    /// Note that the following doesn't run out of memory in Applesoft BASIC--it just
    /// loops infinitely--and this method ensures that it will work the same in
    /// our implementation:
    ///
    ///   10 for i = 1 to 3
    ///   20 goto 10
    ///
    /// Finally, note that Dartmouth BASIC actually had a "FOR WITHOUT NEXT" error,
    /// but this doesn't seem to be present in Applesoft BASIC, nor is it present in
    /// our implementation.
    fn remove_loop_with_name(&mut self, symbol: &Symbol) -> Option<LoopInfo> {
        let mut found_index = None;
        for (i, loop_info) in self.loop_stack.iter().enumerate().rev() {
            if &loop_info.symbol == symbol {
                found_index = Some(i);
                break;
            }
        }
        match found_index {
            Some(i) => {
                let mut removed = self.loop_stack.drain(i..);
                Some(removed.next().unwrap())
            }
            None => None,
        }
    }

    pub fn break_at_current_location(&mut self) {
        match self.location.as_numbered() {
            None => {
                self.breakpoint = None;
            }
            Some(nloc) => {
                self.breakpoint = Some(nloc);
            }
        }
        self.set_and_goto_immediate_line(vec![]);
    }

    pub fn continue_from_breakpoint(&mut self) -> Result<(), TracedInterpreterError> {
        self.set_and_goto_immediate_line(vec![]);
        let Some(location) = self.breakpoint else {
            return Err(InterpreterError::CannotContinue.into());
        };
        self.location = location.into();
        self.breakpoint = None;
        Ok(())
    }

    pub fn start_loop(
        &mut self,
        variables: &mut Variables,
        symbol: Symbol,
        from_value: f64,
        to_value: f64,
        step_value: f64,
    ) -> Result<(), TracedInterpreterError> {
        self.remove_loop_with_name(&symbol);
        if self.loop_stack.len() == STACK_LIMIT {
            return Err(OutOfMemoryError::StackOverflow.into());
        }
        self.loop_stack.push(LoopInfo {
            location: self.location,
            symbol: symbol.clone(),
            to_value,
            step_value,
        });
        variables.set(symbol, from_value.into())?;
        Ok(())
    }

    pub fn end_loop(
        &mut self,
        variables: &mut Variables,
        symbol: Symbol,
    ) -> Result<(), TracedInterpreterError> {
        let current_value = variables.get(&symbol);
        let current_number: f64 = current_value.clone().try_into()?;

        let Some(loop_info) = self.remove_loop_with_name(&symbol) else {
            return Err(InterpreterError::NextWithoutFor.into());
        };

        if loop_info.symbol != symbol {
            return Err(InterpreterError::NextWithoutFor.into());
        }

        let new_value = current_number + loop_info.step_value;

        // I obtained this logic through experimentation with
        // Applesoft BASIC, but it's also mentioned in the Dartmouth
        // BASIC manual, fourth edition.
        let continue_loop = if loop_info.step_value >= 0.0 {
            new_value <= loop_info.to_value
        } else {
            new_value >= loop_info.to_value
        };

        if continue_loop {
            self.location = loop_info.location;
            self.loop_stack.push(loop_info);
        }

        variables.set(symbol, new_value.into())?;
        Ok(())
    }

    pub fn has_line_number(&self, line_number: u64) -> bool {
        self.numbered_lines.has(line_number)
    }

    /// Go to the first numbered line. Resets virtually everything in the program
    /// except for the actual code.
    pub fn run_from_first_numbered_line(&mut self) {
        self.breakpoint = None;
        self.reset_data_cursor();
        self.functions.clear();
        self.stack.clear();
        self.loop_stack.clear();
        if let Some(first_line) = self.numbered_lines.first() {
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
        self.breakpoint = None;
        if self.numbered_lines.has(line_number) {
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
        self.stack.push(StackFrame {
            return_location,
            variables: Variables::default(),
        });
        Ok(())
    }

    pub fn return_to_last_gosub(&mut self) -> Result<(), TracedInterpreterError> {
        self.breakpoint = None;
        let Some(stack_frame) = self.stack.pop() else {
            return Err(InterpreterError::ReturnWithoutGosub.into());
        };
        self.location = stack_frame.return_location;
        Ok(())
    }

    pub fn end(&mut self) {
        self.set_and_goto_immediate_line(vec![]);
    }

    pub fn define_function(
        &mut self,
        name: Symbol,
        arguments: Vec<Symbol>,
    ) -> Result<(), TracedInterpreterError> {
        self.functions.insert(
            name,
            FunctionDefinition {
                arguments,
                location: self.location.try_into()?,
            },
        );
        Ok(())
    }

    pub fn get_function_argument_names(&mut self, name: &Symbol) -> Option<&Vec<Symbol>> {
        self.functions.get(name).map(|f| &f.arguments)
    }

    /// Push the function with the given name onto the stack.
    /// Note that a function with this name MUST exist, or else the program will panic.
    /// You can use `get_function_argument_names` to validate this beforehand.
    pub fn push_function_call_onto_stack_and_goto_it(
        &mut self,
        name: &Symbol,
        bindings: Variables,
    ) -> Result<(), TracedInterpreterError> {
        if self.stack.len() == STACK_LIMIT {
            return Err(OutOfMemoryError::StackOverflow.into());
        }
        self.stack.push(StackFrame {
            return_location: self.location,
            variables: bindings,
        });
        self.location = self
            .functions
            .get(name)
            .expect("function must exist")
            .location
            .into();
        Ok(())
    }

    /// Pop the function with the given name off the stack.
    ///
    /// Note that this must be called after a successful `push_function_call_onto_stack`,
    /// and the program will panic if the stack is empty.
    pub fn pop_function_call_off_stack_and_return_from_it(&mut self) {
        let frame = self.stack.pop().expect("stack must not be empty");
        self.location = frame.return_location;
    }

    pub fn find_variable_value_in_stack(&self, variable_name: &Symbol) -> Option<Value> {
        // Yes, it's really weird that we're crawling up the function call stack to look up
        // variables. This is not normal. But it's how Applesoft BASIC seems to work?
        for frame in self.stack.iter().rev() {
            if frame.variables.has(variable_name) {
                return Some(frame.variables.get(variable_name).clone());
            }
        }
        None
    }

    /// Returns the line number currently being evaluated.
    pub fn get_line_number(&self) -> Option<u64> {
        if let ProgramLine::Line(line_number) = self.location.line {
            Some(line_number)
        } else {
            None
        }
    }

    /// Returns the program location currently being evaluated.
    pub fn get_location(&self) -> ProgramLocation {
        self.location
    }

    pub fn get_line_with_pointer_caret(&self, location: ProgramLocation) -> Vec<String> {
        let tokens = self.tokens_for_line(location.line);
        if tokens.is_empty() {
            return vec![];
        }
        let mut string_tokens = vec![];
        let mut spaces_before_caret = 0;
        for (i, token) in tokens.iter().enumerate() {
            let string_token = token.to_string();
            if i < location.token_index {
                spaces_before_caret += string_token.len() + 1;
            }
            string_tokens.push(string_token);
        }
        vec![
            string_tokens.join(" "),
            format!("{}^", " ".repeat(spaces_before_caret)),
        ]
    }

    pub fn get_data_location(&self) -> Option<ProgramLocation> {
        if let Some(data_iterator) = &self.data_iterator {
            if let Some(location) = data_iterator.current_location() {
                Some(location)
            } else {
                None
            }
        } else {
            None
        }
    }

    pub fn reset_data_cursor(&mut self) {
        self.data_iterator = None;
    }

    pub fn next_data_element(&mut self) -> Option<DataElement> {
        let iterator = self
            .data_iterator
            .get_or_insert_with(|| self.numbered_lines.data_iterator());
        iterator.next()
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
                if let Some(next_line) = self.numbered_lines.after(current_line) {
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
        self.numbered_lines.list()
    }

    /// Sets the given numbered line to the given BASIC code.
    ///
    /// This actually ends up resetting a lot of the state of the program,
    /// because so much of it refers to what's in the BASIC program,
    /// which has now been changed in unknown ways.
    pub fn set_numbered_line(&mut self, line_number: u64, tokens: Vec<Token>) {
        self.numbered_lines.set(line_number, tokens);
        self.breakpoint = None;
        self.reset_data_cursor();
        self.functions.clear();
        self.stack.clear();
        self.loop_stack.clear();
        self.end();
    }

    fn tokens_for_line(&self, line: ProgramLine) -> &Vec<Token> {
        match line {
            ProgramLine::Immediate => &self.immediate_line,
            ProgramLine::Line(number) => self.numbered_lines.get(number).unwrap(),
        }
    }

    fn tokens(&self) -> &Vec<Token> {
        self.tokens_for_line(self.location.line)
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

    /// Goes back in the stream just before the occurrence of the given token.
    ///
    /// If the token isn't found, panics.
    ///
    /// Generally this shouldn't be used unless absolutely necessary, e.g. because
    /// we might need to pause evaluation to do some asynchronous activity and then
    /// resume it again once the task is complete.
    pub fn rewind_before_token(&mut self, token: Token) {
        let wrapped_token = Some(token.clone());
        while self.location.token_index > 0 {
            self.location.token_index -= 1;
            if self.peek_next_token() == wrapped_token {
                return;
            }
        }
        panic!("Token {:?} not found!", token);
    }

    /// Return the next token in the stream, advancing our
    /// position in it.  If there are no more tokens, return an error.
    pub fn next_unwrapped_token(&mut self) -> Result<Token, TracedInterpreterError> {
        match self.next_token() {
            Some(token) => Ok(token),
            None => Err(SyntaxError::UnexpectedEndOfInput.into()),
        }
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

    /// Checks to see if the next token in the stream is the given token.
    ///
    /// If it is, then our position in the stream advances and we return `true`.
    ///
    /// Otherwise, we return `false`.
    pub fn accept_next_token(&mut self, token: Token) -> bool {
        if self.peek_next_token() == Some(token) {
            self.location.token_index += 1;
            true
        } else {
            false
        }
    }

    /// Calls the given function with the next token and returns the result.
    ///
    /// If the function returns a `Some` value, our position in the stream advances.
    ///
    /// If we're already at the end of the stream, the function isn't called, and
    /// `None` is returned.
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
