use crate::{
    program::{Program, ProgramLocation},
    symbol::Symbol,
    InterpreterError, SyntaxError, Token, TracedInterpreterError,
};

use super::{
    expression_analyzer::ExpressionAnalyzer,
    source_file_analyzer::{SymbolAccess, SymbolAccessMap},
    value_type::ValueType,
};

struct LValue {
    symbol_name: Symbol,
    symbol_location: ProgramLocation,
    array_index_arity: Option<usize>,
}

/// This is basically a fork of the statement evaluator, which isn't great.
/// Ideally we'd have some kind of abstraction that allowed the evaluator and
/// analyzer to share the same core parsing logic.
pub struct StatementAnalyzer<'a> {
    program: &'a mut Program,
    symbol_accesses: &'a mut SymbolAccessMap,
}

impl<'a> StatementAnalyzer<'a> {
    pub fn new(program: &'a mut Program, symbol_accesses: &'a mut SymbolAccessMap) -> Self {
        StatementAnalyzer {
            program,
            symbol_accesses,
        }
    }

    pub fn evaluate_statement(&mut self) -> Result<(), TracedInterpreterError> {
        match self.program().next_token() {
            Some(Token::Stop) => Ok(()),
            Some(Token::Dim) => self.evaluate_dim_statement(),
            Some(Token::Print) | Some(Token::QuestionMark) => self.evaluate_print_statement(),
            Some(Token::Input) => self.evaluate_input_statement(),
            Some(Token::If) => self.evaluate_if_statement(),
            Some(Token::Goto | Token::Gosub) => self.evaluate_goto_or_gosub_statement(),
            Some(Token::Return) => Ok(()),
            // Dartmouth BASIC only allowed END at the very end of a program,
            // while Applesoft allowed it anywhere. We'll do the latter.
            Some(Token::End) => Ok(()),
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
        &mut self.program
    }

    fn expression_analyser(&mut self) -> ExpressionAnalyzer {
        ExpressionAnalyzer::new(self.program, self.symbol_accesses)
    }

    fn evaluate_expression(&mut self) -> Result<ValueType, TracedInterpreterError> {
        self.expression_analyser().evaluate_expression()
    }

    fn parse_optional_array_index(&mut self) -> Result<Option<usize>, TracedInterpreterError> {
        if self.program().peek_next_token() != Some(Token::LeftParen) {
            Ok(None)
        } else {
            self.expression_analyser()
                .evaluate_array_index()
                .map(|arity| Some(arity))
        }
    }

    fn evaluate_if_statement(&mut self) -> Result<(), TracedInterpreterError> {
        let _conditional_value = self.evaluate_expression()?;

        // TODO: Dartmouth and Applesoft BASIC both support `IF X GOTO`,
        // whereas we are enforcing the use of `THEN` here.
        self.program().expect_next_token(Token::Then)?;

        // Note that Applesoft BASIC doesn't seem to support ELSE,
        // but it's used in Tim Hartnell's book. We'll support very simple
        // cases; see the test suite for details.

        // Evaluate the "then" clause.
        self.evaluate_statement_or_goto_line_number()?;
        if self.program().accept_next_token(Token::Else) {
            // Evaluate the "else" clause.
            self.evaluate_statement_or_goto_line_number()?;
        }
        Ok(())
    }

    fn assign_value(
        &mut self,
        lvalue: LValue,
        rvalue: ValueType,
    ) -> Result<(), TracedInterpreterError> {
        // TODO: Do something with the arity.
        let _unused_for_now = lvalue.array_index_arity;

        self.log_lvalue_access(&lvalue);
        ValueType::from_variable_name(lvalue.symbol_name).check(rvalue)?;
        Ok(())
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
        let symbol_location = self.program.get_prev_location();
        let lvalue = LValue {
            symbol_name,
            symbol_location,
            array_index_arity: self.parse_optional_array_index()?,
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
        let symbol_location = self.program.get_prev_location();

        let array_index_arity = self.parse_optional_array_index()?;
        Ok(LValue {
            symbol_name,
            symbol_location,
            array_index_arity,
        })
    }

    fn evaluate_read_statement(&mut self) -> Result<(), TracedInterpreterError> {
        loop {
            let lvalue = self.parse_lvalue()?;
            let value = ValueType::from_variable_name(&lvalue.symbol_name);
            self.assign_value(lvalue, value)?;
            if !self.program().accept_next_token(Token::Comma) {
                break;
            }
        }
        Ok(())
    }

    fn log_lvalue_access(&mut self, lvalue: &LValue) {
        self.symbol_accesses.log_access(
            &lvalue.symbol_name,
            &lvalue.symbol_location,
            SymbolAccess::Write,
        );
    }

    fn evaluate_input_statement(&mut self) -> Result<(), TracedInterpreterError> {
        // TODO: Support multiple comma-separated items.
        let lvalue = self.parse_lvalue()?;
        self.log_lvalue_access(&lvalue);
        Ok(())
    }

    /// Note that Darthmouth BASIC actually treated DIM statements similarly to
    /// DATA statements, in that they weren't actually executed at program run-time
    /// and could be placed anywhere in a program. Applesoft BASIC doesn't seem to
    /// treat DIM statements this way, though, perhaps in part because it allows
    /// arrays to be dynamically sized based on user input and such.
    fn evaluate_dim_statement(&mut self) -> Result<(), TracedInterpreterError> {
        let lvalue = self.parse_lvalue()?;
        self.log_lvalue_access(&lvalue);
        // TODO: Do something with the array index arity.
        Ok(())
    }

    fn evaluate_print_statement(&mut self) -> Result<(), TracedInterpreterError> {
        while let Some(token) = self.program().peek_next_token() {
            match token {
                Token::Colon | Token::Else => break,
                Token::Semicolon | Token::Comma => {
                    self.program().next_token().unwrap();
                }
                _ => {
                    self.evaluate_expression()?;
                }
            }
        }
        Ok(())
    }

    fn ensure_valid_line_number(&self, line_number: f64) -> Result<(), TracedInterpreterError> {
        if !self.program.has_line_number(line_number as u64) {
            Err(InterpreterError::UndefinedStatement.into())
        } else {
            Ok(())
        }
    }

    fn evaluate_goto_or_gosub_statement(&mut self) -> Result<(), TracedInterpreterError> {
        let Some(Token::NumericLiteral(line_number)) = self.program().next_token() else {
            return Err(InterpreterError::UndefinedStatement.into());
        };
        self.ensure_valid_line_number(line_number)
    }

    fn evaluate_for_statement(&mut self) -> Result<(), TracedInterpreterError> {
        let Some(Token::Symbol(symbol)) = self.program().next_token() else {
            return Err(SyntaxError::UnexpectedToken.into());
        };
        self.symbol_accesses.log_access(
            &symbol,
            &self.program.get_prev_location(),
            SymbolAccess::Write,
        );
        ValueType::from_variable_name(&symbol).check_number()?;
        self.program().expect_next_token(Token::Equals)?;
        let _from_value = self.evaluate_expression()?.check_number()?;
        self.program().expect_next_token(Token::To)?;
        let _to_value = self.evaluate_expression()?.check_number()?;

        if self.program().accept_next_token(Token::Step) {
            let _step_number = self.evaluate_expression()?.check_number()?;
        }

        Ok(())
    }

    fn evaluate_next_statement(&mut self) -> Result<(), TracedInterpreterError> {
        let Some(Token::Symbol(symbol)) = self.program().next_token() else {
            return Err(SyntaxError::UnexpectedToken.into());
        };
        self.symbol_accesses.log_access(
            &symbol,
            &self.program.get_prev_location(),
            SymbolAccess::Read,
        );
        ValueType::from_variable_name(&symbol).check_number()?;
        Ok(())
    }

    fn evaluate_def_statement(&mut self) -> Result<(), TracedInterpreterError> {
        let Some(Token::Symbol(function_name)) = self.program().next_token() else {
            return Err(SyntaxError::UnexpectedToken.into());
        };
        self.symbol_accesses.log_access(
            &function_name,
            &self.program.get_prev_location(),
            SymbolAccess::Write,
        );
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

        // Evaluate the function body.
        self.evaluate_expression()?;

        Ok(())
    }

    fn evaluate_statement_or_goto_line_number(&mut self) -> Result<(), TracedInterpreterError> {
        if let Some(Token::NumericLiteral(_)) = self.program().peek_next_token() {
            self.evaluate_goto_or_gosub_statement()
        } else {
            self.evaluate_statement()
        }
    }
}
