use crate::{
    arrays::Arrays,
    data::parse_data_until_colon,
    expression::ExpressionEvaluator,
    interpreter_error::{InterpreterError, TracedInterpreterError},
    interpreter_output::InterpreterOutput,
    line_number_parser::parse_line_number,
    program::Program,
    random::Rng,
    string_manager::StringManager,
    symbol::Symbol,
    syntax_error::SyntaxError,
    tokenizer::{Token, Tokenizer},
    value::Value,
    variables::Variables,
};

struct LValue {
    symbol_name: Symbol,
    array_index: Option<Vec<usize>>,
}

#[derive(Default, Debug, PartialEq, Copy, Clone)]
pub enum InterpreterState {
    #[default]
    Idle,
    Running,
    AwaitingInput,
    NewInterpreterRequested,
}

#[derive(Default)]
pub struct Interpreter {
    output: Vec<InterpreterOutput>,
    pub(crate) program: Program,
    pub enable_warnings: bool,
    pub enable_tracing: bool,
    state: InterpreterState,
    input: Option<String>,
    string_manager: StringManager,
    pub(crate) rng: Rng,
    pub(crate) variables: Variables,
    pub(crate) arrays: Arrays,
}

impl core::fmt::Debug for Interpreter {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Interpreter")
            .field("output", &self.output)
            .field("program", &self.program)
            .field("enable_warnings", &self.enable_warnings)
            .field("enable_tracing", &self.enable_tracing)
            .field("state", &self.state)
            .field("input", &self.input)
            .field("string_manager", &self.string_manager)
            .finish()
    }
}

impl Interpreter {
    pub fn take_output(&mut self) -> Vec<InterpreterOutput> {
        std::mem::take(&mut self.output)
    }

    pub(crate) fn warn<T: AsRef<str>>(&mut self, message: T) {
        if self.enable_warnings {
            self.output.push(InterpreterOutput::Warning(
                message.as_ref().to_string(),
                self.program.get_line_number(),
            ));
        }
    }

    fn parse_optional_array_index(&mut self) -> Result<Option<Vec<usize>>, TracedInterpreterError> {
        if self.program.peek_next_token() != Some(Token::LeftParen) {
            Ok(None)
        } else {
            ExpressionEvaluator::new(self)
                .evaluate_array_index()
                .map(|index| Some(index))
        }
    }

    pub fn evaluate_expression(&mut self) -> Result<Value, TracedInterpreterError> {
        ExpressionEvaluator::new(self).evaluate_expression()
    }

    fn evaluate_if_statement(&mut self) -> Result<(), TracedInterpreterError> {
        let conditional_value = self.evaluate_expression()?;

        // TODO: Dartmouth and Applesoft BASIC both support `IF X GOTO`,
        // whereas we are enforcing the use of `THEN` here.
        self.program.expect_next_token(Token::Then)?;

        // Note that Applesoft BASIC doesn't seem to support ELSE,
        // but it's used in Tim Hartnell's book. We'll support very simple
        // cases; see the test suite for details.
        if conditional_value.to_bool() {
            // Evaluate the "then" clause.
            self.evaluate_statement_or_goto_line_number()?;
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
                        self.evaluate_statement_or_goto_line_number()?;
                        return Ok(());
                    }
                    _ => {}
                }
            }
            Ok(())
        }
    }

    pub(crate) fn maybe_log_warning_about_undeclared_array_use(&mut self, array_name: &Symbol) {
        if self.enable_warnings && !self.arrays.has(array_name) {
            self.warn(format!("Use of undeclared array '{}'.", array_name));
        }
    }

    fn assign_value(
        &mut self,
        lvalue: LValue,
        rvalue: Value,
    ) -> Result<(), TracedInterpreterError> {
        match lvalue.array_index {
            Some(index) => {
                self.maybe_log_warning_about_undeclared_array_use(&lvalue.symbol_name);
                self.arrays
                    .set_value_at_index(&lvalue.symbol_name, &index, rvalue)
            }
            None => self.variables.set(lvalue.symbol_name, rvalue),
        }
    }

    fn evaluate_let_statement(&mut self) -> Result<(), TracedInterpreterError> {
        let Some(Token::Symbol(symbol_name)) = self.program.next_token() else {
            return Err(SyntaxError::UnexpectedToken.into());
        };
        self.evaluate_assignment_statement(symbol_name)?;
        Ok(())
    }

    fn evaluate_assignment_statement(
        &mut self,
        symbol_name: Symbol,
    ) -> Result<(), TracedInterpreterError> {
        let lvalue = LValue {
            symbol_name,
            array_index: self.parse_optional_array_index()?,
        };

        // Dartmouth BASIC actually supported chained assignment,
        // e.g. "LET A = B = C = 5" would assign A, B, and C to the
        // value 5. Applesoft BASIC doesn't support this, though,
        // as it just treats the remaining equal signs as equality
        // operators. We follow Applesoft's behavior in this case.
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
        loop {
            let lvalue = self.parse_lvalue()?;
            let Some(element) = self.program.next_data_element() else {
                return Err(InterpreterError::OutOfData.into());
            };
            let value = Value::coerce_from_data_element(lvalue.symbol_name.as_str(), &element)?;
            self.assign_value(lvalue, value)?;
            if !self.program.accept_next_token(Token::Comma) {
                break;
            }
        }
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
            let lvalue = self.parse_lvalue()?;
            let (data, bytes_read) =
                parse_data_until_colon(input.as_str(), Some(&mut self.string_manager));
            // We're guaranteed to have at least one item in here, even if the input was an empty string.
            let first_element = &data[0];
            let has_excess_data = data.len() > 1 || bytes_read < input.len();
            match Value::coerce_from_data_element(lvalue.symbol_name.as_str(), first_element) {
                Ok(value) => {
                    self.assign_value(lvalue, value)?;
                    if has_excess_data {
                        self.output.push(InterpreterOutput::ExtraIgnored);
                    }
                    Ok(())
                }
                Err(TracedInterpreterError {
                    error: InterpreterError::DataTypeMismatch,
                    ..
                }) => {
                    self.output.push(InterpreterOutput::Reenter);
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

    /// Note that Darthmouth BASIC actually treated DIM statements similarly to
    /// DATA statements, in that they weren't actually executed at program run-time
    /// and could be placed anywhere in a program. Applesoft BASIC doesn't seem to
    /// treat DIM statements this way, though, perhaps in part because it allows
    /// arrays to be dynamically sized based on user input and such.
    fn evaluate_dim_statement(&mut self) -> Result<(), TracedInterpreterError> {
        let lvalue = self.parse_lvalue()?;
        let Some(max_indices) = lvalue.array_index else {
            // You'd think this would be some kind of syntax error, but Applesoft
            // just no-ops...
            return Ok(());
        };
        self.arrays.create(lvalue.symbol_name, max_indices)
    }

    fn evaluate_print_statement(&mut self) -> Result<(), TracedInterpreterError> {
        let mut ends_with_semicolon = false;
        let mut strings: Vec<String> = vec![];
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
                Token::Comma => {
                    ends_with_semicolon = false;
                    strings.push("\t".to_string());
                    self.program.next_token().unwrap();
                }
                _ => {
                    ends_with_semicolon = false;
                    match self.evaluate_expression()? {
                        Value::String(string) => {
                            strings.push(string.to_string());
                        }
                        Value::Number(number) => {
                            strings.push(format!("{}", number));
                        }
                    }
                }
            }
        }
        if !ends_with_semicolon {
            strings.push(String::from("\n"));
        }
        self.print(strings.join(""));
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

        self.program.start_loop(
            &mut self.variables,
            symbol.clone(),
            from_number,
            to_number,
            step_number,
        )?;
        Ok(())
    }

    fn evaluate_next_statement(&mut self) -> Result<(), TracedInterpreterError> {
        let Some(Token::Symbol(symbol)) = self.program.next_token() else {
            return Err(SyntaxError::UnexpectedToken.into());
        };
        self.program.end_loop(&mut self.variables, symbol)
    }

    pub fn break_at_current_location(&mut self) {
        self.state = InterpreterState::Idle;
        self.output
            .push(InterpreterOutput::Break(self.program.get_line_number()));
        self.program.break_at_current_location();
    }

    fn evaluate_def_statement(&mut self) -> Result<(), TracedInterpreterError> {
        let Some(Token::Symbol(function_name)) = self.program.next_token() else {
            return Err(SyntaxError::UnexpectedToken.into());
        };
        self.program.expect_next_token(Token::LeftParen)?;
        let mut arg_names: Vec<Symbol> = vec![];
        loop {
            // Note that in Applesoft BASIC, all functions must have at least one argument.
            let Some(Token::Symbol(arg_name)) = self.program.next_token() else {
                return Err(SyntaxError::UnexpectedToken.into());
            };
            arg_names.push(arg_name);
            match self.program.next_token() {
                Some(Token::Comma) => {
                    // Keep looping to parse additional arguments.
                }
                Some(Token::RightParen) => break,
                _ => return Err(SyntaxError::UnexpectedToken.into()),
            }
        }
        self.program.expect_next_token(Token::Equals)?;
        self.program.define_function(function_name, arg_names)?;

        // Skip past function body, as we'll evaluate that whenever the function
        // is actually called. but stop if we encounter a colon, since we'll want
        // to evaluate any additional statements immediately.
        while let Some(token) = self.program.next_token() {
            match token {
                Token::Colon => {
                    break;
                }
                _ => {}
            }
        }

        Ok(())
    }

    fn evaluate_statement_or_goto_line_number(&mut self) -> Result<(), TracedInterpreterError> {
        if let Some(Token::NumericLiteral(_)) = self.program.peek_next_token() {
            self.evaluate_goto_statement()
        } else {
            self.evaluate_statement()
        }
    }

    fn evaluate_statement(&mut self) -> Result<(), TracedInterpreterError> {
        if self.enable_tracing {
            if let Some(line_number) = self.program.get_line_number() {
                self.output.push(InterpreterOutput::Trace(line_number));
            }
        }
        match self.program.next_token() {
            Some(Token::Stop) => Ok(self.break_at_current_location()),
            Some(Token::Dim) => self.evaluate_dim_statement(),
            Some(Token::Print) | Some(Token::QuestionMark) => self.evaluate_print_statement(),
            Some(Token::Input) => self.evaluate_input_statement(),
            Some(Token::If) => self.evaluate_if_statement(),
            Some(Token::Goto) => self.evaluate_goto_statement(),
            Some(Token::Gosub) => self.evaluate_gosub_statement(),
            Some(Token::Return) => self.program.return_to_last_gosub(),
            // Dartmouth BASIC only allowed END at the very end of a program,
            // while Applesoft allowed it anywhere. We'll do the latter.
            Some(Token::End) => Ok(self.program.end()),
            Some(Token::For) => self.evaluate_for_statement(),
            Some(Token::Next) => self.evaluate_next_statement(),
            Some(Token::Restore) => Ok(self.program.reset_data_cursor()),
            Some(Token::Def) => self.evaluate_def_statement(),
            Some(Token::Read) => self.evaluate_read_statement(),
            Some(Token::Remark(_)) => Ok(()),
            Some(Token::Colon) => Ok(()),
            Some(Token::Data(_)) => Ok(()),
            Some(Token::Let) => self.evaluate_let_statement(),
            Some(Token::Symbol(symbol)) => self.evaluate_assignment_statement(symbol),
            Some(_) => Err(SyntaxError::UnexpectedToken.into()),
            None => Ok(()),
        }
    }

    fn run_next_statement(&mut self) -> Result<(), TracedInterpreterError> {
        self.state = InterpreterState::Running;
        if self.program.has_next_token() {
            self.evaluate_statement()?;
        }
        if !self.program.has_next_token() {
            if !self.program.next_line() {
                self.return_to_idle_state();
            }
        }

        Ok(())
    }

    pub fn get_state(&self) -> InterpreterState {
        self.state
    }

    fn maybe_process_command(&mut self, line: &str) -> Result<bool, TracedInterpreterError> {
        let Some(first_word) = line.split_ascii_whitespace().next() else {
            return Ok(false);
        };
        // Note that here we're treating the first word of a line specially.
        // In Applesoft BASIC, commands like LIST and RUN are actually full-fledged
        // BASIC tokens and statements that can be executed through numbered lines.
        // That feels like overkill so for now we're just doing this.
        match first_word.to_ascii_uppercase().as_str() {
            "RUN" => {
                self.variables = Variables::default();
                self.arrays = Arrays::default();
                self.program.run_from_first_numbered_line();
                self.run_next_statement()?;
            }
            "LIST" => {
                self.output.extend(
                    self.program
                        .list()
                        .into_iter()
                        .map(|line| InterpreterOutput::Print(line)),
                );
            }
            "NEW" => {
                self.state = InterpreterState::NewInterpreterRequested;
            }
            "CONT" => {
                self.program.continue_from_breakpoint()?;
                self.run_next_statement()?;
            }
            "TRACE" => {
                self.enable_tracing = true;
            }
            "NOTRACE" => {
                self.enable_tracing = false;
            }
            "INTERNALS" => self.print(format!("{:#?}\n", self)),
            "STATS" => self.print(format!(
                "Total string data: {} bytes\n",
                self.string_manager.total_bytes()
            )),
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
            self.return_to_idle_state();
            Err(err)
        } else {
            result
        }
    }

    fn return_to_idle_state(&mut self) {
        self.program.set_and_goto_immediate_line(vec![]);
        self.string_manager.gc();
        self.state = InterpreterState::Idle;
    }

    fn print(&mut self, string: String) {
        self.output.push(InterpreterOutput::Print(string));
    }

    pub fn has_line_number(&self, line_number: u64) -> bool {
        self.program.has_line_number(line_number)
    }

    pub fn provide_input(&mut self, input: String) {
        assert_eq!(self.state, InterpreterState::AwaitingInput);
        self.input = Some(input);
        self.state = InterpreterState::Running;
    }

    pub fn continue_evaluating(&mut self) -> Result<(), TracedInterpreterError> {
        assert_eq!(self.state, InterpreterState::Running);
        let result = self.run_next_statement();
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
        self.run_next_statement().unwrap();
        line_number
    }

    fn evaluate_impl<T: AsRef<str>>(&mut self, line: T) -> Result<(), TracedInterpreterError> {
        assert_eq!(self.state, InterpreterState::Idle);
        let mut line_ref = line.as_ref();

        if self.maybe_process_command(line_ref.to_uppercase().as_str())? {
            return Ok(());
        }

        let mut maybe_line_number: Option<u64> = None;
        if let Some((line_number, end_index)) = parse_line_number(line_ref) {
            maybe_line_number = Some(line_number);
            line_ref = &line_ref[end_index..];
        }

        let tokens = Tokenizer::new(line_ref, &mut self.string_manager).remaining_tokens()?;

        if let Some(line_number) = maybe_line_number {
            let had_existing_line = self.program.has_line_number(line_number);
            self.program.set_numbered_line(line_number, tokens);
            if had_existing_line {
                self.string_manager.gc();
            }
        } else {
            self.string_manager.gc();
            self.program.set_and_goto_immediate_line(tokens);
            self.run_next_statement()?;
        }

        Ok(())
    }

    pub fn randomize(&mut self, seed: u64) {
        self.rng = Rng::new(seed);
    }
}
