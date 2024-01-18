use crate::{
    arrays::Arrays,
    data::{parse_data_until_colon, DataElement},
    expression::ExpressionEvaluator,
    interpreter_error::TracedInterpreterError,
    interpreter_output::InterpreterOutput,
    line_number_parser::parse_line_number,
    program::Program,
    random::Rng,
    statement::StatementEvaluator,
    string_manager::StringManager,
    symbol::Symbol,
    tokenizer::{Token, Tokenizer},
    value::Value,
    variables::Variables,
};

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
    input: Option<String>,
    output: Vec<InterpreterOutput>,
    state: InterpreterState,
    string_manager: StringManager,
    pub(crate) program: Program,
    pub(crate) rng: Rng,
    pub(crate) variables: Variables,
    pub(crate) arrays: Arrays,
    pub enable_warnings: bool,
    pub enable_tracing: bool,
}

impl core::fmt::Debug for Interpreter {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Interpreter")
            .field("input", &self.input)
            .field("output", &self.output)
            .field("state", &self.state)
            .field("string_manager", &self.string_manager)
            .field("program", &self.program)
            .field("rng", &self.rng)
            .field("variables", &self.variables)
            .field("arrays", &self.arrays)
            .field("enable_warnings", &self.enable_warnings)
            .field("enable_tracing", &self.enable_tracing)
            .finish()
    }
}

impl Interpreter {
    pub fn take_output(&mut self) -> Vec<InterpreterOutput> {
        std::mem::take(&mut self.output)
    }

    pub(crate) fn from_program(program: Program, string_manager: StringManager) -> Self {
        Interpreter {
            program,
            string_manager,
            ..Default::default()
        }
    }

    pub(crate) fn output(&mut self, output: InterpreterOutput) {
        self.output.push(output);
    }

    pub(crate) fn take_input(&mut self) -> Option<(Vec<DataElement>, bool)> {
        if let Some(input) = self.input.take() {
            let (elements, bytes_read) =
                parse_data_until_colon(input.as_str(), Some(&mut self.string_manager));
            let has_leftover_input = bytes_read < input.len();
            Some((elements, has_leftover_input))
        } else {
            None
        }
    }

    pub(crate) fn warn<T: AsRef<str>>(&mut self, message: T) {
        if self.enable_warnings {
            self.output.push(InterpreterOutput::Warning(
                message.as_ref().to_string(),
                self.program.get_line_number(),
            ));
        }
    }

    pub fn evaluate_expression(&mut self) -> Result<Value, TracedInterpreterError> {
        ExpressionEvaluator::new(self).evaluate_expression()
    }

    pub(crate) fn maybe_log_warning_about_undeclared_array_use(&mut self, array_name: &Symbol) {
        if self.enable_warnings && !self.arrays.has(array_name) {
            self.warn(format!("Use of undeclared array '{}'.", array_name));
        }
    }

    pub(crate) fn rewind_program_and_await_input(&mut self) {
        // We need to rewind to before the INPUT token, so that when we resume
        // execution after input has been retrieved, we will get back to this
        // point in the code. This is a hack, but I want to be able to run this
        // in async contexts without having to explicitly make every single part
        // of this interpreter use async/await.
        self.program.rewind_before_token(Token::Input);
        self.state = InterpreterState::AwaitingInput;
    }

    pub fn break_at_current_location(&mut self) {
        self.state = InterpreterState::Idle;
        self.output
            .push(InterpreterOutput::Break(self.program.get_line_number()));
        self.program.break_at_current_location();
    }

    fn run_next_statement(&mut self) -> Result<(), TracedInterpreterError> {
        self.state = InterpreterState::Running;
        if self.program.has_next_token() {
            StatementEvaluator::new(self).evaluate_statement()?;
        }
        if !self.program.has_next_token() {
            if !self.program.next_line() {
                self.program.set_and_goto_immediate_line(vec![]);
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
            self.program.populate_error_location(&mut err);
            self.return_to_idle_state();
            Err(err)
        } else {
            result
        }
    }

    fn return_to_idle_state(&mut self) {
        self.string_manager.gc();
        self.state = InterpreterState::Idle;
    }

    pub(crate) fn print(&mut self, string: String) {
        self.output.push(InterpreterOutput::Print(string));
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
        self.program.set_and_goto_immediate_line(vec![]);

        if self.maybe_process_command(line.as_ref().to_uppercase().as_str())? {
            return Ok(());
        }

        let mut maybe_line_number: Option<u64> = None;
        let mut skip_bytes = 0;
        if let Some((line_number, end_index)) = parse_line_number(line.as_ref()) {
            maybe_line_number = Some(line_number);
            skip_bytes = end_index;
        }

        let tokens = Tokenizer::new(line, &mut self.string_manager)
            .skip_bytes(skip_bytes)
            .remaining_tokens()?;

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
