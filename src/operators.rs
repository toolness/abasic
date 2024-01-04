use crate::{
    interpreter_error::{InterpreterError, TracedInterpreterError},
    tokenizer::Token,
    value::Value,
};

#[derive(Debug, PartialEq)]
pub enum AddOrSubtractOp {
    Add,
    Subtract,
}

impl AddOrSubtractOp {
    // I considered TryFrom here but it required an associated Error type
    // and I just wanted to use Option.
    pub fn from_token(token: Token) -> Option<Self> {
        match &token {
            Token::Plus => Some(AddOrSubtractOp::Add),
            Token::Minus => Some(AddOrSubtractOp::Subtract),
            _ => None,
        }
    }

    pub fn evaluate(
        &self,
        left_side: &Value,
        right_side: &Value,
    ) -> Result<Value, TracedInterpreterError> {
        let result = match (left_side, right_side) {
            (Value::Number(l), Value::Number(r)) => match self {
                AddOrSubtractOp::Add => l + r,
                AddOrSubtractOp::Subtract => l - r,
            },
            _ => return Err(InterpreterError::TypeMismatch.into()),
        };
        Ok(result.into())
    }
}

pub enum UnaryOp {
    Positive,
    Negative,
    Not,
}

impl UnaryOp {
    // I considered TryFrom here but it required an associated Error type
    // and I just wanted to use Option.
    pub fn from_token(token: Token) -> Option<Self> {
        match &token {
            Token::Plus => Some(UnaryOp::Positive),
            Token::Minus => Some(UnaryOp::Negative),
            Token::Not => Some(UnaryOp::Not),
            _ => None,
        }
    }

    pub fn evaluate(&self, value: Value) -> Result<Value, TracedInterpreterError> {
        match self {
            UnaryOp::Positive => Ok(value),
            UnaryOp::Negative => {
                let number: f64 = -value.try_into()?;
                Ok(number.into())
            }
            UnaryOp::Not => Ok(Value::from_bool(!value.to_bool())),
        }
    }
}

pub fn evaluate_exponent(
    left_side: Value,
    right_side: Value,
) -> Result<Value, TracedInterpreterError> {
    let number: f64 = left_side.try_into()?;
    let power: f64 = right_side.try_into()?;

    Ok(number.powf(power).into())
}

pub fn evaluate_logical_or(
    left_side: &Value,
    right_side: &Value,
) -> Result<Value, TracedInterpreterError> {
    Ok(Value::from_bool(
        left_side.to_bool() || right_side.to_bool(),
    ))
}

pub fn evaluate_logical_and(
    left_side: &Value,
    right_side: &Value,
) -> Result<Value, TracedInterpreterError> {
    Ok(Value::from_bool(
        left_side.to_bool() && right_side.to_bool(),
    ))
}

#[derive(Debug)]
pub enum MultiplyOrDivideOp {
    Multiply,
    Divide,
}

impl MultiplyOrDivideOp {
    // I considered TryFrom here but it required an associated Error type
    // and I just wanted to use Option.
    pub fn from_token(token: Token) -> Option<Self> {
        match token {
            Token::Multiply => Some(MultiplyOrDivideOp::Multiply),
            Token::Divide => Some(MultiplyOrDivideOp::Divide),
            _ => None,
        }
    }

    pub fn evaluate(
        &self,
        left_side: &Value,
        right_side: &Value,
    ) -> Result<Value, TracedInterpreterError> {
        let result = match (left_side, right_side) {
            (Value::Number(l), Value::Number(r)) => match self {
                MultiplyOrDivideOp::Multiply => l * r,
                MultiplyOrDivideOp::Divide => {
                    if *r == 0.0 {
                        return Err(InterpreterError::DivisionByZero.into());
                    } else {
                        l / r
                    }
                }
            },
            _ => return Err(InterpreterError::TypeMismatch.into()),
        };
        Ok(result.into())
    }
}

#[derive(Debug)]
pub enum EqualityOp {
    EqualTo,
    LessThan,
    LessThanOrEqualTo,
    GreaterThan,
    GreaterThanOrEqualTo,
    NotEqualTo,
}

impl EqualityOp {
    // I considered TryFrom here but it required an associated Error type
    // and I just wanted to use Option.
    pub fn from_token(token: Token) -> Option<Self> {
        match token {
            Token::Equals => Some(EqualityOp::EqualTo),
            Token::LessThan => Some(EqualityOp::LessThan),
            Token::LessThanOrEqualTo => Some(EqualityOp::LessThanOrEqualTo),
            Token::GreaterThan => Some(EqualityOp::GreaterThan),
            Token::GreaterThanOrEqualTo => Some(EqualityOp::GreaterThanOrEqualTo),
            Token::NotEquals => Some(EqualityOp::NotEqualTo),
            _ => None,
        }
    }

    fn evaluate_partial_ord<T: PartialOrd>(&self, left_side: T, right_side: T) -> bool {
        match self {
            EqualityOp::EqualTo => left_side == right_side,
            EqualityOp::LessThan => left_side < right_side,
            EqualityOp::LessThanOrEqualTo => left_side <= right_side,
            EqualityOp::GreaterThan => left_side > right_side,
            EqualityOp::GreaterThanOrEqualTo => left_side >= right_side,
            EqualityOp::NotEqualTo => left_side != right_side,
        }
    }

    pub fn evaluate(
        &self,
        left_side: &Value,
        right_side: &Value,
    ) -> Result<Value, TracedInterpreterError> {
        let result = match (left_side, right_side) {
            (Value::String(l), Value::String(r)) => self.evaluate_partial_ord(l, r),
            (Value::Number(l), Value::Number(r)) => self.evaluate_partial_ord(l, r),
            _ => return Err(InterpreterError::TypeMismatch.into()),
        };
        // This is how Applesoft BASIC evaluates equality expressions.
        if result {
            Ok(1.0.into())
        } else {
            Ok(0.0.into())
        }
    }
}
