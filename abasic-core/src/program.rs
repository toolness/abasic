use std::{
    collections::{BTreeSet, HashMap},
    rc::Rc,
};

use crate::{
    data::{DataChunk, DataElement, DataIterator},
    dim::ValueArray,
    interpreter_error::{InterpreterError, OutOfMemoryError, TracedInterpreterError},
    syntax_error::SyntaxError,
    tokenizer::Token,
    value::Value,
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
    variables: HashMap<Rc<String>, Value>,
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
    step_value: f64,
}

#[derive(Debug)]
struct FunctionDefinition {
    arguments: Vec<Rc<String>>,
    location: ProgramLocation,
}

#[derive(Debug, Default)]
pub struct Program {
    numbered_lines: HashMap<u64, Vec<Token>>,
    immediate_line: Vec<Token>,

    /// According to Wikipedia, Applesoft BASIC stored lines as a linked list,
    /// which meant that GOSUB/GOTO took linear time. This was likely due to
    /// memory constraints. We don't have such constraints, so we'll use a
    /// BTreeSet for faster lookup.
    sorted_line_numbers: BTreeSet<u64>,

    location: ProgramLocation,
    breakpoint: Option<ProgramLocation>,
    stack: Vec<StackFrame>,
    loop_stack: Vec<LoopInfo>,
    data_iterator: Option<DataIterator>,
    functions: HashMap<Rc<String>, FunctionDefinition>,
    variables: HashMap<Rc<String>, Value>,
    arrays: HashMap<Rc<String>, ValueArray>,
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
    fn remove_loop_with_name(&mut self, symbol: &Rc<String>) -> Option<LoopInfo> {
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
        match self.location.line {
            ProgramLine::Immediate => {
                self.breakpoint = None;
            }
            ProgramLine::Line(_) => {
                self.breakpoint = Some(self.location);
            }
        }
        self.set_and_goto_immediate_line(vec![]);
    }

    pub fn continue_from_breakpoint(&mut self) -> Result<(), TracedInterpreterError> {
        self.set_and_goto_immediate_line(vec![]);
        let Some(location) = self.breakpoint else {
            return Err(InterpreterError::CannotContinue.into());
        };
        self.location = location;
        self.breakpoint = None;
        Ok(())
    }

    pub fn start_loop(
        &mut self,
        symbol: Rc<String>,
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
        self.variables.insert(symbol, from_value.into());
        Ok(())
    }

    pub fn end_loop(&mut self, symbol: Rc<String>) -> Result<(), TracedInterpreterError> {
        let Some(current_value) = self.variables.get(&symbol) else {
            return Err(InterpreterError::NextWithoutFor.into());
        };
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

        self.variables.insert(symbol, new_value.into());
        Ok(())
    }

    pub fn has_line_number(&self, line_number: u64) -> bool {
        self.numbered_lines.contains_key(&line_number)
    }

    /// Go to the first numbered line. Resets the stack and the data cursor.
    pub fn goto_first_numbered_line(&mut self) {
        self.breakpoint = None;
        self.reset_data_cursor();
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
        self.breakpoint = None;
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
        self.stack.push(StackFrame {
            return_location,
            variables: HashMap::new(),
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
        name: Rc<String>,
        arguments: Vec<Rc<String>>,
    ) -> Result<(), TracedInterpreterError> {
        if self.location.line == ProgramLine::Immediate {
            return Err(InterpreterError::IllegalDirect.into());
        }
        self.functions.insert(
            name,
            FunctionDefinition {
                arguments,
                location: self.location,
            },
        );
        Ok(())
    }

    pub fn get_function_argument_names(&mut self, name: &Rc<String>) -> Option<&Vec<Rc<String>>> {
        self.functions.get(name).map(|f| &f.arguments)
    }

    /// Push the function with the given name onto the stack.
    /// Note that a function with this name MUST exist, or else the program will panic.
    /// You can use `get_function_argument_names` to validate this beforehand.
    pub fn push_function_call_onto_stack_and_goto_it(
        &mut self,
        name: &Rc<String>,
        bindings: HashMap<Rc<String>, Value>,
    ) -> Result<(), TracedInterpreterError> {
        if self.stack.len() == STACK_LIMIT {
            return Err(OutOfMemoryError::StackOverflow.into());
        }
        for (arg_name, arg_value) in bindings.iter() {
            arg_value.validate_type_matches_variable_name(arg_name.as_str())?;
        }
        self.stack.push(StackFrame {
            return_location: self.location,
            variables: bindings,
        });
        self.location = self
            .functions
            .get(name)
            .expect("function must exist")
            .location;
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

    pub fn find_variable_value_in_stack(&self, variable_name: &Rc<String>) -> Option<Value> {
        // Yes, it's really weird that we're crawling up the function call stack to look up
        // variables. This is not normal. But it's how Applesoft BASIC seems to work?
        for frame in self.stack.iter().rev() {
            if let Some(value) = frame.variables.get(variable_name) {
                return Some(value.clone());
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

    pub fn get_data_line_number(&self) -> Option<u64> {
        if let Some(data_iterator) = &self.data_iterator {
            if let Some(ProgramLine::Line(line_number)) = data_iterator.current_location() {
                Some(line_number)
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
        let iterator = self.data_iterator.get_or_insert_with(|| {
            let mut chunks = vec![];
            for line in self.sorted_line_numbers.iter() {
                for token in self.numbered_lines.get(line).unwrap() {
                    if let Token::Data(data) = token {
                        chunks.push(DataChunk::new(ProgramLine::Line(*line), data.clone()));
                    }
                }
            }
            DataIterator::new(chunks)
        });
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
        self.breakpoint = None;
        if tokens.is_empty() {
            self.sorted_line_numbers.remove(&line_number);
            self.numbered_lines.remove(&line_number);
        } else {
            self.sorted_line_numbers.insert(line_number);
            self.numbered_lines.insert(line_number, tokens);
        }
        self.reset_data_cursor();
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

    fn maybe_create_default_array(
        &mut self,
        array_name: &Rc<String>,
        dimensions: usize,
    ) -> Result<(), TracedInterpreterError> {
        // It seems we can't use hash_map::Entry here to provide a default value,
        // because we might actually error when creating the default value.
        if !self.arrays.contains_key(array_name) {
            let array = ValueArray::default_for_variable_and_dimensionality(
                &array_name.as_str(),
                dimensions,
            )?;
            self.arrays.insert(array_name.clone(), array);
        }
        Ok(())
    }

    pub fn create_array(
        &mut self,
        array_name: Rc<String>,
        max_indices: Vec<usize>,
    ) -> Result<(), TracedInterpreterError> {
        if self.has_array(&array_name) {
            return Err(InterpreterError::RedimensionedArray.into());
        }
        let array = ValueArray::create(array_name.as_str(), max_indices)?;
        self.arrays.insert(array_name, array);
        Ok(())
    }

    pub fn get_value_at_array_index(
        &mut self,
        array_name: &Rc<String>,
        index: &Vec<usize>,
    ) -> Result<Value, TracedInterpreterError> {
        self.maybe_create_default_array(array_name, index.len())?;
        let array = self.arrays.get(array_name).unwrap();

        Ok(array.get(index)?)
    }

    pub fn set_value_at_array_index(
        &mut self,
        array_name: &Rc<String>,
        index: &Vec<usize>,
        value: Value,
    ) -> Result<(), TracedInterpreterError> {
        value.validate_type_matches_variable_name(array_name.as_str())?;
        self.maybe_create_default_array(array_name, index.len())?;
        let array = self.arrays.get_mut(array_name).unwrap();
        array.set(index, value)?;
        Ok(())
    }

    pub fn has_array(&self, array_name: &Rc<String>) -> bool {
        self.arrays.contains_key(array_name)
    }

    pub fn get_variable_value(&self, name: &Rc<String>) -> Value {
        match self.variables.get(name) {
            Some(value) => value.clone(),
            None => Value::default_for_variable(name.as_str()),
        }
    }

    pub fn set_variable_value(
        &mut self,
        name: Rc<String>,
        value: Value,
    ) -> Result<(), TracedInterpreterError> {
        value.validate_type_matches_variable_name(name.as_str())?;
        self.variables.insert(name, value);
        Ok(())
    }

    pub fn has_variable(&self, name: &Rc<String>) -> bool {
        self.variables.contains_key(name)
    }
}

fn unwrap_token(token: Option<Token>) -> Result<Token, TracedInterpreterError> {
    match token {
        Some(token) => Ok(token),
        None => Err(SyntaxError::UnexpectedEndOfInput.into()),
    }
}
