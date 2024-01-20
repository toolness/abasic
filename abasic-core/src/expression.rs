use crate::{
    builtins::Builtin,
    operators::{
        evaluate_exponent, evaluate_logical_and, evaluate_logical_or, AddOrSubtractOp, EqualityOp,
        MultiplyOrDivideOp, UnaryOp,
    },
    program::Program,
    symbol::Symbol,
    value::Value,
    variables::Variables,
    Interpreter, InterpreterError, SyntaxError, Token, TracedInterpreterError,
};

pub struct ExpressionEvaluator<'a> {
    interpreter: &'a mut Interpreter,
}

impl<'a> ExpressionEvaluator<'a> {
    pub fn new(interpreter: &'a mut Interpreter) -> Self {
        ExpressionEvaluator { interpreter }
    }

    pub fn evaluate_expression(&mut self) -> Result<Value, TracedInterpreterError> {
        self.evaluate_logical_or_expression()
    }

    pub fn evaluate_array_index(&mut self) -> Result<Vec<usize>, TracedInterpreterError> {
        let mut indices: Vec<usize> = vec![];
        self.program().expect_next_token(Token::LeftParen)?;
        loop {
            let Value::Number(value) = self.evaluate_expression()? else {
                return Err(InterpreterError::TypeMismatch.into());
            };
            let Ok(index) = usize::try_from(value as i64) else {
                return Err(InterpreterError::IllegalQuantity.into());
            };
            indices.push(index);
            if !self.program().accept_next_token(Token::Comma) {
                break;
            }
        }
        self.program().expect_next_token(Token::RightParen)?;
        Ok(indices)
    }

    fn program(&mut self) -> &mut Program {
        &mut self.interpreter.program
    }

    fn evaluate_unary_number_function_arg(&mut self) -> Result<f64, TracedInterpreterError> {
        self.program().expect_next_token(Token::LeftParen)?;
        let arg: f64 = self.evaluate_expression()?.try_into()?;
        self.program().expect_next_token(Token::RightParen)?;
        Ok(arg)
    }

    fn evaluate_unary_number_function<F: Fn(f64) -> f64>(
        &mut self,
        f: F,
    ) -> Result<Value, TracedInterpreterError> {
        let arg = self.evaluate_unary_number_function_arg()?;
        Ok(f(arg).into())
    }

    fn evaluate_user_defined_function_call(
        &mut self,
        function_name: &Symbol,
    ) -> Result<Option<Value>, TracedInterpreterError> {
        let Some(arg_names) = self
            .program()
            .get_function_argument_names(function_name)
            // Cloning this is a bit of a bummer but we don't expect user-defined
            // function calls to happen very often, and we can always put the Vec
            // behind an Rc to speed things up.
            .cloned()
        else {
            return Ok(None);
        };

        self.program().expect_next_token(Token::LeftParen)?;
        let arity = arg_names.len();
        let mut bindings = Variables::with_capacity(arity);
        for (i, arg) in arg_names.into_iter().enumerate() {
            let value = self.evaluate_expression()?;
            bindings.set(arg, value)?;
            if i < arity - 1 {
                self.program().expect_next_token(Token::Comma)?;
            }
        }
        self.program().expect_next_token(Token::RightParen)?;
        self.program()
            .push_function_call_onto_stack_and_goto_it(function_name, bindings)?;
        let value = self.evaluate_expression()?;
        self.program()
            .pop_function_call_off_stack_and_return_from_it();

        Ok(Some(value))
    }

    fn evaluate_function_call(
        &mut self,
        function_name: &Symbol,
    ) -> Result<Option<Value>, TracedInterpreterError> {
        if let Some(builtin) = Builtin::try_from(function_name) {
            match builtin {
                Builtin::Abs => self.evaluate_unary_number_function(|num| num.abs()),
                Builtin::Int => self.evaluate_unary_number_function(|num| num.floor()),
                Builtin::Rnd => {
                    let number = self.evaluate_unary_number_function_arg()?;
                    Ok(self.interpreter.rng.rnd(number)?.into())
                }
            }
            .map(|value| Some(value))
        } else {
            self.evaluate_user_defined_function_call(function_name)
        }
    }

    fn evaluate_expression_term(&mut self) -> Result<Value, TracedInterpreterError> {
        match self.program().next_unwrapped_token()? {
            Token::StringLiteral(string) => Ok(string.into()),
            Token::NumericLiteral(number) => Ok(number.into()),
            Token::Symbol(symbol) => {
                let is_array_or_function_call =
                    self.program().peek_next_token() == Some(Token::LeftParen);
                if is_array_or_function_call {
                    if let Some(value) = self.evaluate_function_call(&symbol)? {
                        Ok(value)
                    } else {
                        let index = self.evaluate_array_index()?;
                        self.interpreter
                            .maybe_log_warning_about_undeclared_array_use(&symbol);
                        self.interpreter.arrays.get_value_at_index(&symbol, &index)
                    }
                } else if let Some(value) = self.program().find_variable_value_in_stack(&symbol) {
                    Ok(value)
                } else {
                    if self.interpreter.enable_warnings && !self.interpreter.variables.has(&symbol)
                    {
                        self.interpreter
                            .warn(format!("Use of undeclared variable '{}'.", symbol));
                    }
                    Ok(self.interpreter.variables.get(&symbol))
                }
            }
            _ => Err(SyntaxError::UnexpectedToken.into()),
        }
    }

    fn evaluate_parenthesized_expression(&mut self) -> Result<Value, TracedInterpreterError> {
        if self.program().accept_next_token(Token::LeftParen) {
            let value = self.evaluate_expression()?;
            self.program().expect_next_token(Token::RightParen)?;
            Ok(value)
        } else {
            self.evaluate_expression_term()
        }
    }

    fn evaluate_unary_operator(&mut self) -> Result<Value, TracedInterpreterError> {
        let maybe_unary_op = self.program().try_next_token(UnaryOp::from_token);

        let value = self.evaluate_parenthesized_expression()?;

        if let Some(unary_op) = maybe_unary_op {
            Ok(unary_op.evaluate(value)?)
        } else {
            Ok(value)
        }
    }

    fn evaluate_exponent_expression(&mut self) -> Result<Value, TracedInterpreterError> {
        let mut value = self.evaluate_unary_operator()?;

        while self.program().accept_next_token(Token::Caret) {
            let power = self.evaluate_unary_operator()?;
            value = evaluate_exponent(value, power)?;
        }

        Ok(value)
    }

    fn evaluate_multiply_or_divide_expression(&mut self) -> Result<Value, TracedInterpreterError> {
        let mut value = self.evaluate_exponent_expression()?;

        while let Some(op) = self
            .program()
            .try_next_token(MultiplyOrDivideOp::from_token)
        {
            let second_operand = self.evaluate_exponent_expression()?;
            value = op.evaluate(&value, &second_operand)?;
        }

        Ok(value)
    }

    fn evaluate_plus_or_minus_expression(&mut self) -> Result<Value, TracedInterpreterError> {
        let mut value = self.evaluate_multiply_or_divide_expression()?;

        while let Some(plus_or_minus) = self.program().try_next_token(AddOrSubtractOp::from_token) {
            let second_operand = self.evaluate_multiply_or_divide_expression()?;
            value = plus_or_minus.evaluate(&value, &second_operand)?;
        }

        Ok(value)
    }

    fn evaluate_equality_expression(&mut self) -> Result<Value, TracedInterpreterError> {
        let mut value = self.evaluate_plus_or_minus_expression()?;

        while let Some(equality_op) = self.program().try_next_token(EqualityOp::from_token) {
            let second_operand = self.evaluate_plus_or_minus_expression()?;
            value = equality_op.evaluate(&value, &second_operand)?;
        }

        Ok(value)
    }

    fn evaluate_logical_and_expression(&mut self) -> Result<Value, TracedInterpreterError> {
        let mut value = self.evaluate_equality_expression()?;

        while self.program().accept_next_token(Token::And) {
            let second_operand = self.evaluate_equality_expression()?;
            value = evaluate_logical_and(&value, &second_operand)?;
        }

        Ok(value)
    }

    // Logical OR actually has lower precedence than logical AND.  See the Applesoft II BASIC
    // Reference Manual, pg. 36.
    fn evaluate_logical_or_expression(&mut self) -> Result<Value, TracedInterpreterError> {
        let mut value = self.evaluate_logical_and_expression()?;

        while self.program().accept_next_token(Token::Or) {
            let second_operand = self.evaluate_logical_and_expression()?;
            value = evaluate_logical_or(&value, &second_operand)?;
        }

        Ok(value)
    }
}
