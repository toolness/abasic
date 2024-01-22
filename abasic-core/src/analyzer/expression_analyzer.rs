use crate::{
    builtins::Builtin,
    operators::{AddOrSubtractOp, EqualityOp, MultiplyOrDivideOp, UnaryOp},
    program::Program,
    symbol::Symbol,
    SyntaxError, Token, TracedInterpreterError,
};

use super::{
    source_file_analyzer::{SymbolAccess, SymbolAccessMap},
    value_type::ValueType,
};

/// This is basically a fork of the expression evaluator, which isn't great.
/// Ideally we'd have some kind of abstraction that allowed the evaluator and
/// analyzer to share the same core parsing logic.
pub struct ExpressionAnalyzer<'a> {
    program: &'a mut Program,
    symbol_accesses: &'a mut SymbolAccessMap,
}

impl<'a> ExpressionAnalyzer<'a> {
    pub fn new(program: &'a mut Program, symbol_accesses: &'a mut SymbolAccessMap) -> Self {
        ExpressionAnalyzer {
            program,
            symbol_accesses,
        }
    }

    pub fn evaluate_expression(&mut self) -> Result<ValueType, TracedInterpreterError> {
        self.evaluate_logical_or_expression()
    }

    pub fn evaluate_array_index(&mut self) -> Result<usize, TracedInterpreterError> {
        let mut arity = 0;
        self.program.expect_next_token(Token::LeftParen)?;
        loop {
            self.evaluate_expression()?.check_number()?;
            arity += 1;
            if !self.program.accept_next_token(Token::Comma) {
                break;
            }
        }
        self.program.expect_next_token(Token::RightParen)?;
        Ok(arity)
    }

    fn evaluate_unary_number_function_arg(&mut self) -> Result<ValueType, TracedInterpreterError> {
        self.program.expect_next_token(Token::LeftParen)?;
        let result = self.evaluate_expression()?.check_number()?;
        self.program.expect_next_token(Token::RightParen)?;
        Ok(result)
    }

    fn evaluate_user_defined_function_call(
        &mut self,
        function_name: &Symbol,
    ) -> Result<Option<ValueType>, TracedInterpreterError> {
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
        for (i, arg) in arg_names.into_iter().enumerate() {
            let value = self.evaluate_expression()?;
            value.check_variable_name(arg)?;
            if i < arity - 1 {
                self.program.expect_next_token(Token::Comma)?;
            }
        }
        self.program.expect_next_token(Token::RightParen)?;

        Ok(Some(ValueType::from_variable_name(function_name)))
    }

    fn evaluate_function_call(
        &mut self,
        function_name: &Symbol,
    ) -> Result<Option<ValueType>, TracedInterpreterError> {
        if let Some(builtin) = Builtin::try_from(function_name) {
            match builtin {
                Builtin::Abs | Builtin::Int | Builtin::Rnd => {
                    self.evaluate_unary_number_function_arg()
                }
            }
            .map(|value| Some(value))
        } else {
            self.evaluate_user_defined_function_call(function_name)
        }
    }

    fn evaluate_expression_term(&mut self) -> Result<ValueType, TracedInterpreterError> {
        match self.program.next_unwrapped_token()? {
            Token::StringLiteral(_string) => Ok(ValueType::String),
            Token::NumericLiteral(_number) => Ok(ValueType::Number),
            Token::Symbol(symbol) => {
                self.symbol_accesses.log_access(
                    &symbol,
                    &self.program.get_prev_location(),
                    SymbolAccess::Read,
                );
                let is_array_or_function_call =
                    self.program.peek_next_token() == Some(Token::LeftParen);
                if is_array_or_function_call {
                    if let Some(value) = self.evaluate_function_call(&symbol)? {
                        Ok(value)
                    } else {
                        self.evaluate_array_index()?;
                        Ok(ValueType::from_variable_name(symbol))
                    }
                } else {
                    Ok(ValueType::from_variable_name(symbol))
                }
            }
            _ => Err(SyntaxError::UnexpectedToken.into()),
        }
    }

    fn evaluate_parenthesized_expression(&mut self) -> Result<ValueType, TracedInterpreterError> {
        if self.program.accept_next_token(Token::LeftParen) {
            let value = self.evaluate_expression()?;
            self.program.expect_next_token(Token::RightParen)?;
            Ok(value)
        } else {
            self.evaluate_expression_term()
        }
    }

    fn evaluate_unary_operator(&mut self) -> Result<ValueType, TracedInterpreterError> {
        let maybe_unary_op = self.program.try_next_token(UnaryOp::from_token);

        let value = self.evaluate_parenthesized_expression()?;

        if let Some(unary_op) = maybe_unary_op {
            match unary_op {
                UnaryOp::Positive | UnaryOp::Negative => Ok(value.check_number()?),
                UnaryOp::Not => Ok(value),
            }
        } else {
            Ok(value)
        }
    }

    fn evaluate_exponent_expression(&mut self) -> Result<ValueType, TracedInterpreterError> {
        let value = self.evaluate_unary_operator()?;

        while self.program.accept_next_token(Token::Caret) {
            let power = self.evaluate_unary_operator()?;
            value.check_number()?;
            power.check_number()?;
        }

        Ok(value)
    }

    fn evaluate_multiply_or_divide_expression(
        &mut self,
    ) -> Result<ValueType, TracedInterpreterError> {
        let value = self.evaluate_exponent_expression()?;

        while let Some(_op) = self.program.try_next_token(MultiplyOrDivideOp::from_token) {
            let second_operand = self.evaluate_exponent_expression()?;
            value.check_number()?;
            second_operand.check_number()?;
        }

        Ok(value)
    }

    fn evaluate_plus_or_minus_expression(&mut self) -> Result<ValueType, TracedInterpreterError> {
        let value = self.evaluate_multiply_or_divide_expression()?;

        while let Some(_plus_or_minus) = self.program.try_next_token(AddOrSubtractOp::from_token) {
            let second_operand = self.evaluate_multiply_or_divide_expression()?;
            value.check_number()?;
            second_operand.check_number()?;
        }

        Ok(value)
    }

    fn evaluate_equality_expression(&mut self) -> Result<ValueType, TracedInterpreterError> {
        let value = self.evaluate_plus_or_minus_expression()?;

        while let Some(_equality_op) = self.program.try_next_token(EqualityOp::from_token) {
            let second_operand = self.evaluate_plus_or_minus_expression()?;
            value.check(second_operand)?;
        }

        Ok(value)
    }

    fn evaluate_logical_and_expression(&mut self) -> Result<ValueType, TracedInterpreterError> {
        let value = self.evaluate_equality_expression()?;

        while self.program.accept_next_token(Token::And) {
            let _second_operand = self.evaluate_equality_expression()?;
        }

        Ok(value)
    }

    // Logical OR actually has lower precedence than logical AND.  See the Applesoft II BASIC
    // Reference Manual, pg. 36.
    fn evaluate_logical_or_expression(&mut self) -> Result<ValueType, TracedInterpreterError> {
        let value = self.evaluate_logical_and_expression()?;

        while self.program.accept_next_token(Token::Or) {
            let _second_operand = self.evaluate_logical_and_expression()?;
        }

        Ok(value)
    }
}
