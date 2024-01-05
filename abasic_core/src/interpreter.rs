use std::{collections::HashMap, fmt::Display, rc::Rc};

use crate::{
    builtins,
    data::parse_data_until_colon,
    dim::ValueArray,
    interpreter_error::{InterpreterError, TracedInterpreterError},
    line_number_parser::parse_line_number,
    operators::{
        evaluate_exponent, evaluate_logical_and, evaluate_logical_or, AddOrSubtractOp, EqualityOp,
        MultiplyOrDivideOp, UnaryOp,
    },
    program::Program,
    syntax_error::SyntaxError,
    tokenizer::{Token, Tokenizer},
    value::Value,
};

#[derive(Debug)]
pub enum InterpreterOutput {
    Print(String),
    Break(Option<u64>),
    Warning(String, Option<u64>),
    Trace(u64),
    ExtraIgnored,
    Reenter,
}

impl InterpreterOutput {
    fn get_in_line_number_string(line: &Option<u64>) -> String {
        line.map(|line| format!(" IN {}", line)).unwrap_or_default()
    }
}

impl Display for InterpreterOutput {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            InterpreterOutput::Print(string) => string.fmt(f),
            InterpreterOutput::Warning(message, line) => {
                write!(
                    f,
                    "WARNING{}: {}",
                    InterpreterOutput::get_in_line_number_string(line),
                    message
                )
            }
            InterpreterOutput::Break(line) => {
                write!(
                    f,
                    "BREAK{}",
                    InterpreterOutput::get_in_line_number_string(line)
                )
            }
            InterpreterOutput::ExtraIgnored => write!(f, "EXTRA IGNORED"),
            InterpreterOutput::Reenter => write!(f, "REENTER"),
            InterpreterOutput::Trace(line) => write!(f, "#{}", line),
        }
    }
}

struct LValue {
    symbol_name: Rc<String>,
    array_index: Option<Vec<usize>>,
}

#[derive(Debug, PartialEq, Copy, Clone)]
pub enum InterpreterState {
    Idle,
    Running,
    AwaitingInput,
    NewInterpreterRequested,
}

pub struct Interpreter {
    output: Vec<InterpreterOutput>,
    program: Program,
    pub enable_warnings: bool,
    pub enable_tracing: bool,
    variables: HashMap<Rc<String>, Value>,
    arrays: HashMap<Rc<String>, ValueArray>,
    state: InterpreterState,
    input: Option<String>,
}

impl core::fmt::Debug for Interpreter {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Interpreter")
            .field("output", &self.output)
            .field("program", &self.program)
            .field("enable_warnings", &self.enable_warnings)
            .field("enable_tracing", &self.enable_tracing)
            .field("variables", &self.variables)
            .field("arrays", &self.arrays)
            .field("state", &self.state)
            .field("input", &self.input)
            .finish()
    }
}

impl Interpreter {
    pub fn new() -> Self {
        Interpreter {
            output: vec![],
            program: Default::default(),
            variables: HashMap::new(),
            arrays: HashMap::new(),
            state: InterpreterState::Idle,
            enable_warnings: false,
            enable_tracing: false,
            input: None,
        }
    }

    pub fn take_output(&mut self) -> Vec<InterpreterOutput> {
        std::mem::take(&mut self.output)
    }

    fn evaluate_unary_function<F: Fn(Value) -> Result<Value, TracedInterpreterError>>(
        &mut self,
        f: F,
    ) -> Result<Value, TracedInterpreterError> {
        self.program.expect_next_token(Token::LeftParen)?;
        let arg = self.evaluate_expression()?;
        self.program.expect_next_token(Token::RightParen)?;
        f(arg)
    }

    fn evaluate_user_defined_function_call(
        &mut self,
        function_name: &Rc<String>,
    ) -> Result<Option<Value>, TracedInterpreterError> {
        let Some(arg_names) = self
            .program
            .get_function_argument_names(function_name)
            // Cloning this is a bit of a bummer but we don't expect user-defined
            // function calls to happen very often, and we can always put the Vec
            // behind an Rc to speed things up.
            .cloned()
        else {
            return Ok(None);
        };

        self.program.expect_next_token(Token::LeftParen)?;
        let arity = arg_names.len();
        let mut bindings: HashMap<Rc<String>, Value> = HashMap::with_capacity(arity);
        for (i, arg) in arg_names.into_iter().enumerate() {
            let value = self.evaluate_expression()?;
            value.validate_type_matches_variable_name(arg.as_str())?;
            bindings.insert(arg, value);
            if i < arity - 1 {
                self.program.expect_next_token(Token::Comma)?;
            }
        }
        self.program.expect_next_token(Token::RightParen)?;
        self.program
            .push_function_call_onto_stack_and_goto_it(function_name, bindings)?;
        let value = self.evaluate_expression()?;
        self.program
            .pop_function_call_off_stack_and_return_from_it();

        Ok(Some(value))
    }

    fn evaluate_function_call(
        &mut self,
        function_name: &Rc<String>,
    ) -> Result<Option<Value>, TracedInterpreterError> {
        let result = match function_name.as_str() {
            "ABS" => self.evaluate_unary_function(builtins::abs),
            "INT" => self.evaluate_unary_function(builtins::int),
            "RND" => self.evaluate_unary_function(builtins::rnd),
            _ => {
                return self.evaluate_user_defined_function_call(function_name);
            }
        };
        result.map(|value| Some(value))
    }

    fn warn<T: AsRef<str>>(&mut self, message: T) {
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
            self.parse_array_index().map(|index| Some(index))
        }
    }

    fn parse_array_index(&mut self) -> Result<Vec<usize>, TracedInterpreterError> {
        let mut indices: Vec<usize> = vec![];
        self.program.expect_next_token(Token::LeftParen)?;
        loop {
            let Value::Number(value) = self.evaluate_expression()? else {
                return Err(InterpreterError::TypeMismatch.into());
            };
            let Ok(index) = usize::try_from(value as i64) else {
                return Err(InterpreterError::IllegalQuantity.into());
            };
            indices.push(index);
            if !self.program.accept_next_token(Token::Comma) {
                break;
            }
        }
        self.program.expect_next_token(Token::RightParen)?;
        Ok(indices)
    }

    fn maybe_create_default_array(
        &mut self,
        array_name: &Rc<String>,
        dimensions: usize,
    ) -> Result<(), TracedInterpreterError> {
        // It seems we can't use hash_map::Entry here to provide a default value,
        // because we might actually error when creating the default value.
        if !self.arrays.contains_key(array_name) {
            if self.enable_warnings {
                self.warn(format!("Use of undeclared array '{}'.", array_name));
            }
            let array = ValueArray::default_for_variable_and_dimensionality(
                &array_name.as_str(),
                dimensions,
            )?;
            self.arrays.insert(array_name.clone(), array);
        }
        Ok(())
    }

    fn set_value_at_array_index(
        &mut self,
        array_name: &Rc<String>,
        index: &Vec<usize>,
        value: Value,
    ) -> Result<(), TracedInterpreterError> {
        self.maybe_create_default_array(&array_name, index.len())?;
        let array = self.arrays.get_mut(array_name).unwrap();
        array.set(index, value)
    }

    fn evaluate_value_at_array_index(
        &mut self,
        array_name: Rc<String>,
    ) -> Result<Value, TracedInterpreterError> {
        let index = self.parse_array_index()?;

        self.maybe_create_default_array(&array_name, index.len())?;
        let array = self.arrays.get(&array_name).unwrap();

        Ok(array.get(&index)?)
    }

    fn evaluate_expression_term(&mut self) -> Result<Value, TracedInterpreterError> {
        match self.program.next_unwrapped_token()? {
            Token::StringLiteral(string) => Ok(string.into()),
            Token::NumericLiteral(number) => Ok(number.into()),
            Token::Symbol(symbol) => {
                let is_array_or_function_call =
                    self.program.peek_next_token() == Some(Token::LeftParen);
                if is_array_or_function_call {
                    if let Some(value) = self.evaluate_function_call(&symbol)? {
                        Ok(value)
                    } else {
                        self.evaluate_value_at_array_index(symbol)
                    }
                } else if let Some(value) = self.program.find_variable_value_in_stack(&symbol) {
                    Ok(value)
                } else if let Some(value) = self.variables.get(&symbol) {
                    Ok(value.clone())
                } else {
                    if self.enable_warnings {
                        self.warn(format!("Use of undeclared variable '{}'.", symbol));
                    }
                    Ok(Value::default_for_variable(symbol.as_str()))
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

    fn evaluate_unary_operator(&mut self) -> Result<Value, TracedInterpreterError> {
        let maybe_unary_op = self.program.try_next_token(UnaryOp::from_token);

        let value = self.evaluate_parenthesized_expression()?;

        if let Some(unary_op) = maybe_unary_op {
            Ok(unary_op.evaluate(value)?)
        } else {
            Ok(value)
        }
    }

    fn evaluate_exponent_expression(&mut self) -> Result<Value, TracedInterpreterError> {
        let mut value = self.evaluate_unary_operator()?;

        while self.program.accept_next_token(Token::Caret) {
            let power = self.evaluate_unary_operator()?;
            value = evaluate_exponent(value, power)?;
        }

        Ok(value)
    }

    fn evaluate_multiply_or_divide_expression(&mut self) -> Result<Value, TracedInterpreterError> {
        let mut value = self.evaluate_exponent_expression()?;

        while let Some(op) = self.program.try_next_token(MultiplyOrDivideOp::from_token) {
            let second_operand = self.evaluate_exponent_expression()?;
            value = op.evaluate(&value, &second_operand)?;
        }

        Ok(value)
    }

    fn evaluate_plus_or_minus_expression(&mut self) -> Result<Value, TracedInterpreterError> {
        let mut value = self.evaluate_multiply_or_divide_expression()?;

        while let Some(plus_or_minus) = self.program.try_next_token(AddOrSubtractOp::from_token) {
            let second_operand = self.evaluate_multiply_or_divide_expression()?;
            value = plus_or_minus.evaluate(&value, &second_operand)?;
        }

        Ok(value)
    }

    fn evaluate_equality_expression(&mut self) -> Result<Value, TracedInterpreterError> {
        let mut value = self.evaluate_plus_or_minus_expression()?;

        while let Some(equality_op) = self.program.try_next_token(EqualityOp::from_token) {
            let second_operand = self.evaluate_plus_or_minus_expression()?;
            value = equality_op.evaluate(&value, &second_operand)?;
        }

        Ok(value)
    }

    fn evaluate_logical_and_expression(&mut self) -> Result<Value, TracedInterpreterError> {
        let mut value = self.evaluate_equality_expression()?;

        while self.program.accept_next_token(Token::And) {
            let second_operand = self.evaluate_equality_expression()?;
            value = evaluate_logical_and(&value, &second_operand)?;
        }

        Ok(value)
    }

    // Logical OR actually has lower precedence than logical AND.  See the Applesoft II BASIC
    // Reference Manual, pg. 36.
    fn evaluate_logical_or_expression(&mut self) -> Result<Value, TracedInterpreterError> {
        let mut value = self.evaluate_logical_and_expression()?;

        while self.program.accept_next_token(Token::Or) {
            let second_operand = self.evaluate_logical_and_expression()?;
            value = evaluate_logical_or(&value, &second_operand)?;
        }

        Ok(value)
    }

    fn evaluate_expression(&mut self) -> Result<Value, TracedInterpreterError> {
        self.evaluate_logical_or_expression()
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

    fn assign_value(
        &mut self,
        lvalue: LValue,
        rvalue: Value,
    ) -> Result<(), TracedInterpreterError> {
        rvalue.validate_type_matches_variable_name(lvalue.symbol_name.as_str())?;

        match lvalue.array_index {
            Some(index) => {
                self.set_value_at_array_index(&lvalue.symbol_name, &index, rvalue)?;
            }
            None => {
                self.variables.insert(lvalue.symbol_name, rvalue);
            }
        }

        Ok(())
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
        symbol_name: Rc<String>,
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
            let (data, bytes_read) = parse_data_until_colon(input.as_str());
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
        if self.arrays.contains_key(&lvalue.symbol_name) {
            return Err(InterpreterError::RedimensionedArray.into());
        }
        let array = ValueArray::create(lvalue.symbol_name.as_str(), max_indices)?;
        self.arrays.insert(lvalue.symbol_name, array);
        Ok(())
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
        self.output.push(InterpreterOutput::Print(strings.join("")));
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

        self.program
            .start_loop(symbol.clone(), to_number, step_number)?;
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
        let mut arg_names: Vec<Rc<String>> = vec![];
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
                self.program.goto_first_numbered_line();
                self.run()?;
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
                self.run()?;
            }
            "TRACE" => {
                self.enable_tracing = true;
            }
            "NOTRACE" => {
                self.enable_tracing = false;
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
            let line_number = match err.error {
                InterpreterError::DataTypeMismatch => self.program.get_data_line_number(),
                _ => self.program.get_line_number(),
            };
            if let Some(line_number) = line_number {
                err.set_line_number(line_number);
            }
            self.state = InterpreterState::Idle;
            Err(err)
        } else {
            result
        }
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
        let mut line_ref = line.as_ref();

        if self.maybe_process_command(line_ref.to_uppercase().as_str())? {
            return Ok(());
        }

        let mut maybe_line_number: Option<u64> = None;
        if let Some((line_number, end_index)) = parse_line_number(line_ref) {
            maybe_line_number = Some(line_number);
            line_ref = &line_ref[end_index..];
        }

        let tokens = Tokenizer::new(line_ref).remaining_tokens()?;

        if let Some(line_number) = maybe_line_number {
            self.program.set_numbered_line(line_number, tokens);
        } else {
            self.program.set_and_goto_immediate_line(tokens);
            self.run()?;
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use crate::{
        interpreter_error::{OutOfMemoryError, TracedInterpreterError},
        syntax_error::SyntaxError,
        tokenizer::Token,
        InterpreterOutput,
    };

    use super::{Interpreter, InterpreterError, InterpreterState};

    struct Action {
        expected_output: &'static str,
        then_input: Option<&'static str>,
    }

    impl Action {
        fn expect_output(expected_output: &'static str) -> Self {
            Action {
                expected_output,
                then_input: None,
            }
        }

        fn then_input(mut self, input: &'static str) -> Self {
            self.then_input = Some(input);
            self
        }
    }

    fn create_interpreter() -> Interpreter {
        Interpreter::new()
    }

    fn evaluate_while_running(interpreter: &mut Interpreter) -> Result<(), TracedInterpreterError> {
        while interpreter.get_state() == InterpreterState::Running {
            interpreter.continue_evaluating()?;
        }
        Ok(())
    }

    fn evaluate_line_while_running(
        interpreter: &mut Interpreter,
        line: &str,
    ) -> Result<(), TracedInterpreterError> {
        interpreter.start_evaluating(line)?;
        evaluate_while_running(interpreter)
    }

    fn assert_eval_error(line: &'static str, expected: InterpreterError) {
        let mut interpreter = create_interpreter();
        match evaluate_line_while_running(&mut interpreter, line) {
            Ok(_) => {
                panic!("expected '{}' to error but it didn't", line);
            }
            Err(err) => {
                assert_eq!(err.error, expected, "evaluating '{}'", line);
            }
        }
    }

    fn assert_eval_output(line: &'static str, expected: &'static str) {
        let mut interpreter = create_interpreter();
        let output = eval_line_and_expect_success(&mut interpreter, line);
        assert_eq!(output, expected, "evaluating '{}'", line);
    }

    fn assert_program_actions(program: &'static str, actions: &[Action]) {
        let mut interpreter = create_interpreter();
        let lines = program.split("\n").map(|line| line.trim_start());
        for line in lines {
            eval_line_and_expect_success(&mut interpreter, line);
        }
        let mut output = eval_line_and_expect_success(&mut interpreter, "run");
        for (i, action) in actions.iter().enumerate() {
            assert_eq!(
                output, action.expected_output,
                "running action {} of program: {}",
                i, program
            );
            if let Some(input) = action.then_input {
                interpreter.provide_input(input.to_string());
                output = match evaluate_while_running(&mut interpreter) {
                    Ok(_) => take_output_as_string(&mut interpreter),
                    Err(err) => {
                        panic!(
                            "after inputting '{}', expected successful evaluation but got {}\nIntepreter state is: {:?}",
                            input,
                            err,
                            interpreter
                        )
                    }
                }
            }
        }
    }

    fn assert_program_output(program: &'static str, expected: &'static str) {
        assert_program_actions(program, &[Action::expect_output(expected)]);
    }

    fn assert_program_error(program: &'static str, expected: InterpreterError) {
        let mut interpreter = create_interpreter();
        let lines = program.split("\n").map(|line| line.trim_start());
        for line in lines {
            eval_line_and_expect_success(&mut interpreter, line);
        }
        match evaluate_line_while_running(&mut interpreter, "run") {
            Ok(_) => {
                panic!("expected program to error but it didn't: {}", program);
            }
            Err(err) => {
                assert_eq!(err.error, expected, "running program: {}", program);
            }
        }
    }

    fn take_output_as_string(interpreter: &mut Interpreter) -> String {
        interpreter
            .take_output()
            .into_iter()
            .map(|output| match output {
                InterpreterOutput::Print(message) => message.to_string(),
                _ => format!("{}\n", output.to_string()),
            })
            .collect::<Vec<_>>()
            .join("")
    }

    fn eval_line_and_expect_success<T: AsRef<str>>(
        interpreter: &mut Interpreter,
        line: T,
    ) -> String {
        match evaluate_line_while_running(interpreter, line.as_ref()) {
            Ok(_) => take_output_as_string(interpreter),
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
    fn print_as_question_mark_works() {
        // This is a shortcut that Applesoft BASIC provides.
        assert_eval_output("?", "\n");
        assert_eval_output("? \"\"", "\n");
        assert_eval_output("? \"hello 😊\"", "hello 😊\n");
        assert_eval_output("? \"hello 😊\" 5", "hello 😊5\n");
        assert_eval_output("? \"hello 😊\" 5 \"there\"", "hello 😊5there\n");
    }

    #[test]
    fn print_works_with_comma() {
        assert_eval_output("print ,1", "\t1\n");
    }

    #[test]
    fn print_works_with_semicolon() {
        assert_eval_output("print ;", "");
        assert_eval_output("print ;\"\"", "\n");
        assert_eval_output("print \"hello 😊\";", "hello 😊");
        assert_eval_output("print \"hello\";:print \"there\"", "hellothere\n");
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
        assert_eval_output("print 15 / 5 * 3", "9\n");
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

        assert_eval_output("print 1 >= 1", "1\n");
        assert_eval_output("print 2 >= 1", "1\n");
        assert_eval_output("print 2 >= 3", "0\n");

        assert_eval_output("print 1 <= 1", "1\n");
        assert_eval_output("print 2 <= 1", "0\n");
        assert_eval_output("print 2 <= 3", "1\n");

        assert_eval_output("print 1 <> 2", "1\n");
        assert_eval_output("print 1 <> 1", "0\n");
    }

    #[test]
    fn print_works_with_chained_numeric_equality_expressions() {
        assert_eval_output("print 5 > 4 > 3", "0\n");
        assert_eval_output("print 5 > 4 = 1", "1\n");
    }

    #[test]
    fn exponentiation_works() {
        assert_eval_output("print 5 ^ 2", "25\n");
        assert_eval_output("print 5 ^ 2 + 3 ^ 2", "34\n");
        assert_eval_output("print 5 ^ 2 * 2 ^ 2", "100\n");
        assert_eval_output("print -5 ^ 2", "25\n");
    }

    #[test]
    fn unary_logical_operator_works() {
        assert_eval_output("print not 5", "0\n");
        assert_eval_output("print not 0", "1\n");
        assert_eval_output("print not 5 * 300", "0\n");
        assert_eval_output("print not 0 * 300", "300\n");
        assert_eval_output("print not 0 + 10", "11\n");
        assert_eval_output("print not 530 + 10", "10\n");
    }

    #[test]
    fn binary_logical_operators_work() {
        assert_eval_output("print 5 AND 2", "1\n");
        assert_eval_output("print 5 AND 0", "0\n");
        assert_eval_output("print 0 AND 0", "0\n");

        assert_eval_output("print 5 OR 2", "1\n");
        assert_eval_output("print 5 OR 0", "1\n");
        assert_eval_output("print 0 OR 0", "0\n");
    }

    #[test]
    fn abs_works() {
        assert_eval_output("print abs(5)", "5\n");
        assert_eval_output("print abs(-5)", "5\n");
        assert_eval_output("print abs(-6.0 + 1)", "5\n");
        assert_eval_output("print abs(x)", "0\n");
    }

    #[test]
    fn int_works() {
        assert_eval_output("print int(3)", "3\n");
        assert_eval_output("print int(4.1)", "4\n");
        assert_eval_output("print int(5.9)", "5\n");
    }

    #[test]
    fn rnd_works() {
        fastrand::seed(0);
        assert_eval_output(
            "for i = 1 to 3:print int(rnd(1) * 50):next i",
            "3\n40\n19\n",
        );

        assert_eval_error("print rnd(-1)", InterpreterError::Unimplemented);
        assert_eval_error("print rnd(0)", InterpreterError::Unimplemented);
    }

    #[ignore]
    #[test]
    fn builtin_functions_cannot_be_redefined() {
        todo!("TODO: Add a test to make sure ABS can't be redefined, etc.");
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
    fn if_statement_processes_multiple_statements_in_then_clause() {
        assert_eval_output("if 1 then print \"hi\":print", "hi\n\n");
        assert_eval_output("if 0 then print \"hi\":print:kaboom", "");
        assert_eval_output("if 1 then x=3:print \"hi \" x:print", "hi 3\n\n");
    }

    #[test]
    fn if_statement_processes_multiple_statements_in_else_clause() {
        assert_eval_output("if 1 then print \"hi\" else print \"blah\":print \"this is only executed with the else clause\"", "hi\n");
        assert_eval_output(
            "if 0 then print \"hi\" else print \"blah\":print",
            "blah\n\n",
        );
        assert_eval_output(
            "if 1 then y=4 else x=3:print \"this is only executed with the else clause\"",
            "",
        );
        assert_eval_output("if 0 then y=4 else x=3:print \"hallo \" y", "hallo 0\n");
    }

    #[test]
    fn if_statement_does_not_support_else_when_then_clause_has_multiple_statements() {
        assert_eval_error(
            "if 1 then print:print else print",
            SyntaxError::UnexpectedToken.into(),
        );

        assert_eval_error(
            "if 1 then x = 3:y = 4 else z = 3",
            SyntaxError::UnexpectedToken.into(),
        );
    }

    #[test]
    fn default_array_values_work() {
        assert_eval_output("print a(1)", "0\n");
        assert_eval_output("print a$(1)", "\n");
        assert_eval_output("print a(1,2,3)", "0\n");
        assert_eval_output("print a$(1,2,3)", "\n");
    }

    #[test]
    fn array_assignment_works() {
        assert_eval_output("a(0) = 5:print a(0)", "5\n");
        assert_eval_output("a(1,1) = 5:print a(1,1)", "5\n");

        assert_eval_output("a$(0) = \"blarg\":print a$(0)", "blarg\n");
        assert_eval_output("a$(1,1) = \"blarg\":print a$(1,1)", "blarg\n");

        assert_eval_output("x = 4:a(x+1) = 123:print a(5)", "123\n");
    }

    #[test]
    fn variables_and_arrays_exist_in_separate_universes() {
        // This is not a bug, it's how Applesoft BASIC works. Although it might
        // be a bug in Applesoft BASIC, I'm not sure.
        assert_eval_output("print a:print a(1)", "0\n0\n");
        assert_eval_output("a = 1:print a:print a(1)", "1\n0\n");
        assert_eval_output("print a(1):a = 1:print a:print a(1)", "0\n1\n0\n");
    }

    #[test]
    fn assignment_works_with_let() {
        assert_eval_output("let x=1:print x", "1\n");
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

        assert_eval_output(
            "for i = 3 to 1 step -1: print i:next i:print \"DONE\" i",
            "3\n2\n1\nDONE0\n",
        );

        assert_eval_output("for i = 1 to 3 step 2: print i:next i", "1\n3\n");
    }

    #[test]
    fn nested_looping_works() {
        assert_eval_output(
            "for i = 1 to 2: print \"i = \" i:for j = 1 to 2:print \"j = \" j:next j:next i",
            "i = 1\nj = 1\nj = 2\ni = 2\nj = 1\nj = 2\n",
        );
    }

    #[test]
    fn weird_looping_works() {
        // This is weird but works in Applesoft BASIC.
        assert_eval_output(
            r#"for i = 1 to 2: print "i = " i:for j = 1 to 2:print "j = " j:next i"#,
            "i = 1\nj = 1\ni = 2\nj = 1\n",
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
    fn type_mismatch_error_works_with_array_assignment() {
        assert_eval_error("x(1) = x$", InterpreterError::TypeMismatch);
        assert_eval_error("x(1) = \"hi\"", InterpreterError::TypeMismatch);

        assert_eval_error("x$(1) = x", InterpreterError::TypeMismatch);
        assert_eval_error("x$(1) = 1", InterpreterError::TypeMismatch);
    }

    #[test]
    fn out_of_data_error_works() {
        assert_eval_error("read a", InterpreterError::OutOfData);
        // Applesoft BASIC only recognizes data in actual line numbers in the program, so
        // an immediate data statement is basically a no-op.
        assert_eval_error("data 1,2,3:read a", InterpreterError::OutOfData);
    }

    #[test]
    fn syntax_error_raised_when_no_array_index_is_given() {
        assert_eval_error("print a()", SyntaxError::UnexpectedToken.into());
    }

    #[test]
    fn illegal_quantity_error_works() {
        assert_eval_error("print a(-1)", InterpreterError::IllegalQuantity);
    }

    #[test]
    fn bad_subscript_error_works() {
        assert_eval_error("print a(1):print a(1,1)", InterpreterError::BadSubscript);
        assert_eval_error("print a(1,1):print a(1)", InterpreterError::BadSubscript);
        assert_eval_error("a(1) = 5:print a(1,1)", InterpreterError::BadSubscript);

        // This is weird b/c implicitly-created arrays are sized at 10 dimensions.
        assert_eval_error("print a(11)", InterpreterError::BadSubscript);
    }

    #[test]
    fn type_mismatch_error_works_with_array_indexing() {
        assert_eval_error("print a(\"hi\")", InterpreterError::TypeMismatch);
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
    fn division_by_zero_error_works() {
        assert_eval_error("print 5/0", InterpreterError::DivisionByZero);
    }

    #[test]
    fn dim_works() {
        assert_eval_output("dim a(100):a(57) = 123:print a(56):print a(57)", "0\n123\n");

        // This is weird, but Applesoft is weird.
        assert_eval_output("dim a:dim a:a = 5:print a:dim a:print a", "5\n5\n");
    }

    #[test]
    fn redimensioned_array_error_works() {
        assert_eval_error("dim a(1):dim a(1)", InterpreterError::RedimensionedArray);
    }

    #[test]
    fn data_is_ignored() {
        assert_eval_output("print 1:data a,b,c:print 2", "1\n2\n");
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
    fn line_numbers_can_be_redefined() {
        assert_program_output(
            r#"
            10 print "boop"
            10 print "sup"
            20 print "dog"
            "#,
            "sup\ndog\n",
        );
    }

    #[test]
    fn empty_line_numbers_are_deleted() {
        assert_program_error(
            r#"
            10 goto 20
            20 print "sup"
            20
            "#,
            InterpreterError::UndefinedStatement,
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
            60 end
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
            60 end
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

    #[test]
    fn then_clause_works_with_only_line_number() {
        assert_program_output(
            r#"
            20 if 1 then 40
            30 print "THIS SHOULD NOT PRINT"
            40 print "hi"
            "#,
            "hi\n",
        );
    }

    #[test]
    fn else_clause_works_with_only_line_number() {
        assert_program_output(
            r#"
            20 if 0 then 30 else 40
            30 print "THIS SHOULD NOT PRINT"
            40 print "hi"
            "#,
            "hi\n",
        );
    }

    #[test]
    fn restore_works() {
        assert_program_output(
            r#"
            10 data sup,dog,1
            20 for i = 1 to 3
            30 read a$
            40 print a$
            45 restore
            50 next i
            "#,
            "sup\nsup\nsup\n",
        );
    }

    #[test]
    fn read_works_with_commas() {
        assert_program_output(
            r#"
            10 data sup,dog,1
            20 read A$,b$,c
            30 print a$
            40 print b$
            50 print c
            "#,
            "sup\ndog\n1\n",
        );
    }

    #[test]
    fn data_works_with_arrays() {
        assert_program_output(
            r#"
            10 data sup,dog,1
            20 for i = 1 to 3
            30 read a$(i)
            40 print a$(i)
            50 next i
            "#,
            "sup\ndog\n1\n",
        );
    }

    #[test]
    fn data_at_beginning_works() {
        assert_program_output(
            r#"
            10 data sup,dog,1
            20 for i = 1 to 3
            30 read a$
            40 print a$
            50 next i
            "#,
            "sup\ndog\n1\n",
        );
    }

    #[test]
    fn data_at_end_works() {
        assert_program_output(
            r#"
            20 for i = 1 to 3
            30 read a$
            40 print a$
            50 next i
            60 data sup,dog,1
            "#,
            "sup\ndog\n1\n",
        );
    }

    #[test]
    fn data_in_middle_works() {
        assert_program_output(
            r#"
            20 for i = 1 to 3
            30 read a$
            35 data sup,dog,1
            40 print a$
            50 next i
            "#,
            "sup\ndog\n1\n",
        );
    }

    #[test]
    fn data_type_mismatch_works() {
        assert_program_error(
            r#"
            10 data sup
            20 read a
            "#,
            InterpreterError::DataTypeMismatch,
        );
    }

    #[test]
    fn statements_are_processed_after_function_definitions() {
        assert_program_output(
            r#"
            10 def fna(x) = x + 1:print "hi"
            20 print fna(1)
            "#,
            "hi\n2\n",
        );
    }

    #[test]
    fn functions_work() {
        assert_program_output(
            r#"
            10 def fna(x) = x + 1
            20 print fna(1)
            "#,
            "2\n",
        );
    }

    #[test]
    fn functions_with_multiple_arguments_work() {
        assert_program_output(
            r#"
            10 def fna(x,y,z) = x + y + z + 1
            20 print fna(1,2,3)
            "#,
            "7\n",
        );
    }

    #[test]
    fn nested_functions_work() {
        assert_program_output(
            r#"
            10 def fna(x) = x + 1
            20 def fnb(x) = fna(x) + 1
            30 print fnb(1)
            "#,
            "3\n",
        );
    }

    #[test]
    fn function_calls_without_enough_arguments_fail() {
        assert_program_error(
            r#"
            10 def fna(x,y,z) = x + y + z + 1
            20 print fna(1,2)
            "#,
            SyntaxError::ExpectedToken(Token::Comma).into(),
        );
    }

    #[test]
    fn function_calls_with_too_many_arguments_fail() {
        assert_program_error(
            r#"
            10 def fna(x,y,z) = x + y + z + 1
            20 print fna(1,2,3,4)
            "#,
            SyntaxError::ExpectedToken(Token::RightParen).into(),
        );
    }

    #[test]
    fn infinite_recursion_causes_stack_overflow() {
        assert_program_error(
            r#"
            10 def fna(x) = fna(x) + 1
            20 print fna(1)
            "#,
            OutOfMemoryError::StackOverflow.into(),
        );
    }

    #[test]
    fn input_works() {
        assert_program_actions(
            r#"
            10 input a$
            20 print "hello " a$
        "#,
            &[
                Action::expect_output("").then_input("buddy"),
                Action::expect_output("hello buddy\n"),
            ],
        )
    }

    #[test]
    fn input_works_with_arrays() {
        assert_program_actions(
            r#"
            10 input a$(0)
            20 print "hello " a$(0)
        "#,
            &[
                Action::expect_output("").then_input("buddy"),
                Action::expect_output("hello buddy\n"),
            ],
        )
    }

    #[test]
    fn input_reentry_works() {
        assert_program_actions(
            r#"
            10 input a
            20 print "hello " a
        "#,
            &[
                Action::expect_output("").then_input("this is not a number"),
                Action::expect_output("REENTER\n").then_input("123"),
                Action::expect_output("hello 123\n"),
            ],
        )
    }

    #[test]
    fn input_ignoring_extra_works_with_commas() {
        assert_program_actions(
            r#"
            10 input a$
            20 print "hello " a$
        "#,
            &[
                Action::expect_output("").then_input("sup, dog"),
                Action::expect_output("EXTRA IGNORED\nhello sup\n"),
            ],
        )
    }

    #[test]
    fn input_ignoring_extra_works_with_colons() {
        // This is weird, but it's how Applesoft BASIC works, and it's how
        // this interpreter works because it's the easiest thing to implement.
        assert_program_actions(
            r#"
            10 input a$
            20 print "hello " a$
        "#,
            &[
                Action::expect_output("").then_input("sup:dog"),
                Action::expect_output("EXTRA IGNORED\nhello sup\n"),
            ],
        )
    }
}
