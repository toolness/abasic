use crate::{
    expression::ExpressionEvaluator, program::Program, symbol::Symbol, value::Value, Interpreter,
    InterpreterError, InterpreterOutput, SyntaxError, Token, TracedInterpreterError,
};

struct LValue {
    symbol_name: Symbol,
    array_index: Option<Vec<usize>>,
}

pub struct StatementEvaluator<'a> {
    interpreter: &'a mut Interpreter,
}

impl<'a> StatementEvaluator<'a> {
    pub fn new(interpreter: &'a mut Interpreter) -> Self {
        StatementEvaluator { interpreter }
    }

    pub fn evaluate_statement(&mut self) -> Result<(), TracedInterpreterError> {
        if self.interpreter.enable_tracing {
            if let Some(line_number) = self.program().get_line_number() {
                self.interpreter
                    .output(InterpreterOutput::Trace(line_number));
            }
        }
        match self.program().next_token() {
            Some(Token::Stop) => Ok(self.interpreter.break_at_current_location()),
            Some(Token::Dim) => self.evaluate_dim_statement(),
            Some(Token::Print) | Some(Token::QuestionMark) => self.evaluate_print_statement(),
            Some(Token::Input) => self.evaluate_input_statement(),
            Some(Token::If) => self.evaluate_if_statement(),
            Some(Token::Goto) => self.evaluate_goto_statement(),
            Some(Token::Gosub) => self.evaluate_gosub_statement(),
            Some(Token::Return) => self.program().return_to_last_gosub(),
            // Dartmouth BASIC only allowed END at the very end of a program,
            // while Applesoft allowed it anywhere. We'll do the latter.
            Some(Token::End) => Ok(self.program().end()),
            Some(Token::For) => self.evaluate_for_statement(),
            Some(Token::Next) => self.evaluate_next_statement(),
            Some(Token::Restore) => Ok(self.program().reset_data_cursor()),
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

    fn program(&mut self) -> &mut Program {
        &mut self.interpreter.program
    }

    fn evaluate_expression(&mut self) -> Result<Value, TracedInterpreterError> {
        ExpressionEvaluator::new(self.interpreter).evaluate_expression()
    }

    fn parse_optional_array_index(&mut self) -> Result<Option<Vec<usize>>, TracedInterpreterError> {
        if self.program().peek_next_token() != Some(Token::LeftParen) {
            Ok(None)
        } else {
            ExpressionEvaluator::new(self.interpreter)
                .evaluate_array_index()
                .map(|index| Some(index))
        }
    }

    fn evaluate_if_statement(&mut self) -> Result<(), TracedInterpreterError> {
        let conditional_value = self.evaluate_expression()?;

        // TODO: Dartmouth and Applesoft BASIC both support `IF X GOTO`,
        // whereas we are enforcing the use of `THEN` here.
        self.program().expect_next_token(Token::Then)?;

        // Note that Applesoft BASIC doesn't seem to support ELSE,
        // but it's used in Tim Hartnell's book. We'll support very simple
        // cases; see the test suite for details.
        if conditional_value.to_bool() {
            // Evaluate the "then" clause.
            self.evaluate_statement_or_goto_line_number()?;
            if self.program().peek_next_token() == Some(Token::Else) {
                // Skip the else clause, and anything else on this line.
                self.program().discard_remaining_tokens();
            }
            Ok(())
        } else {
            // Skip past the "then" clause. If we encounter a colon, ignore
            // the rest of the line, but if we encounter an "else", evaluate
            // everything after it.
            while let Some(token) = self.program().next_token() {
                match token {
                    Token::Colon => {
                        self.program().discard_remaining_tokens();
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
        match lvalue.array_index {
            Some(index) => {
                self.interpreter
                    .maybe_log_warning_about_undeclared_array_use(&lvalue.symbol_name);
                self.interpreter
                    .arrays
                    .set_value_at_index(&lvalue.symbol_name, &index, rvalue)
            }
            None => self.interpreter.variables.set(lvalue.symbol_name, rvalue),
        }
    }

    fn evaluate_let_statement(&mut self) -> Result<(), TracedInterpreterError> {
        let Some(Token::Symbol(symbol_name)) = self.program().next_token() else {
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
        self.program().expect_next_token(Token::Equals)?;

        let value = self.evaluate_expression()?;
        self.assign_value(lvalue, value)?;
        Ok(())
    }

    fn parse_lvalue(&mut self) -> Result<LValue, TracedInterpreterError> {
        let Some(Token::Symbol(symbol_name)) = self.program().next_token() else {
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
            let Some(element) = self.program().next_data_element() else {
                return Err(InterpreterError::OutOfData.into());
            };
            let value = Value::coerce_from_data_element(lvalue.symbol_name.as_str(), &element)?;
            self.assign_value(lvalue, value)?;
            if !self.program().accept_next_token(Token::Comma) {
                break;
            }
        }
        Ok(())
    }

    fn evaluate_input_statement(&mut self) -> Result<(), TracedInterpreterError> {
        if let Some((data, has_leftover_input)) = self.interpreter.take_input() {
            // TODO: Support multiple comma-separated items.
            let lvalue = self.parse_lvalue()?;
            // We're guaranteed to have at least one item in here, even if the input was an empty string.
            let first_element = &data[0];
            let has_excess_data = data.len() > 1 || has_leftover_input;
            match Value::coerce_from_data_element(lvalue.symbol_name.as_str(), first_element) {
                Ok(value) => {
                    self.assign_value(lvalue, value)?;
                    if has_excess_data {
                        self.interpreter.output(InterpreterOutput::ExtraIgnored);
                    }
                    Ok(())
                }
                Err(TracedInterpreterError {
                    error: InterpreterError::DataTypeMismatch,
                    ..
                }) => {
                    self.interpreter.output(InterpreterOutput::Reenter);
                    self.interpreter.rewind_program_and_await_input();
                    Ok(())
                }
                Err(err) => Err(err),
            }
        } else {
            self.interpreter.rewind_program_and_await_input();
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
        self.interpreter
            .arrays
            .create(lvalue.symbol_name, max_indices)
    }

    fn evaluate_print_statement(&mut self) -> Result<(), TracedInterpreterError> {
        let mut ends_with_semicolon = false;
        let mut strings: Vec<String> = vec![];
        while let Some(token) = self.program().peek_next_token() {
            match token {
                Token::Colon | Token::Else => break,
                Token::Semicolon => {
                    // Semicolons in Applesoft BASIC are very weird, they can be interspersed
                    // throughout a PRINT statement and appear to do nothing, unless they're at
                    // the end, in which case there won't be a newline at the end of the output.
                    ends_with_semicolon = true;
                    self.program().next_token().unwrap();
                }
                Token::Comma => {
                    ends_with_semicolon = false;
                    strings.push("\t".to_string());
                    self.program().next_token().unwrap();
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
        self.interpreter.print(strings.join(""));
        Ok(())
    }

    fn evaluate_goto_statement(&mut self) -> Result<(), TracedInterpreterError> {
        let Some(Token::NumericLiteral(line_number)) = self.program().next_token() else {
            return Err(InterpreterError::UndefinedStatement.into());
        };
        self.program().goto_line_number(line_number as u64)?;
        Ok(())
    }

    fn evaluate_gosub_statement(&mut self) -> Result<(), TracedInterpreterError> {
        let Some(Token::NumericLiteral(line_number)) = self.program().next_token() else {
            return Err(InterpreterError::UndefinedStatement.into());
        };
        self.program().gosub_line_number(line_number as u64)?;
        Ok(())
    }

    fn evaluate_for_statement(&mut self) -> Result<(), TracedInterpreterError> {
        let Some(Token::Symbol(symbol)) = self.program().next_token() else {
            return Err(SyntaxError::UnexpectedToken.into());
        };
        self.program().expect_next_token(Token::Equals)?;
        let from_value = self.evaluate_expression()?;
        let from_number: f64 = from_value.try_into()?;
        self.program().expect_next_token(Token::To)?;
        let to_value = self.evaluate_expression()?;
        let to_number: f64 = to_value.try_into()?;

        let step_number = if self.program().accept_next_token(Token::Step) {
            self.evaluate_expression()?.try_into()?
        } else {
            1.0
        };

        self.interpreter.program.start_loop(
            &mut self.interpreter.variables,
            symbol.clone(),
            from_number,
            to_number,
            step_number,
        )?;
        Ok(())
    }

    fn evaluate_next_statement(&mut self) -> Result<(), TracedInterpreterError> {
        let Some(Token::Symbol(symbol)) = self.program().next_token() else {
            return Err(SyntaxError::UnexpectedToken.into());
        };
        self.interpreter
            .program
            .end_loop(&mut self.interpreter.variables, symbol)
    }

    fn evaluate_def_statement(&mut self) -> Result<(), TracedInterpreterError> {
        let Some(Token::Symbol(function_name)) = self.program().next_token() else {
            return Err(SyntaxError::UnexpectedToken.into());
        };
        self.program().expect_next_token(Token::LeftParen)?;
        let mut arg_names: Vec<Symbol> = vec![];
        loop {
            // Note that in Applesoft BASIC, all functions must have at least one argument.
            let Some(Token::Symbol(arg_name)) = self.program().next_token() else {
                return Err(SyntaxError::UnexpectedToken.into());
            };
            arg_names.push(arg_name);
            match self.program().next_token() {
                Some(Token::Comma) => {
                    // Keep looping to parse additional arguments.
                }
                Some(Token::RightParen) => break,
                _ => return Err(SyntaxError::UnexpectedToken.into()),
            }
        }
        self.program().expect_next_token(Token::Equals)?;
        self.program().define_function(function_name, arg_names)?;

        // Skip past function body, as we'll evaluate that whenever the function
        // is actually called. but stop if we encounter a colon, since we'll want
        // to evaluate any additional statements immediately.
        while let Some(token) = self.program().next_token() {
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
        if let Some(Token::NumericLiteral(_)) = self.program().peek_next_token() {
            self.evaluate_goto_statement()
        } else {
            self.evaluate_statement()
        }
    }
}
