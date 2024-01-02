use crate::{
    interpreter_error::{InterpreterError, TracedInterpreterError},
    tokenizer::Token,
    value::Value,
};

#[derive(PartialEq)]
pub enum PlusOrMinusOp {
    Plus,
    Minus,
}

impl PlusOrMinusOp {
    // I considered TryFrom here but it required an associated Error type
    // and I just wanted to use Option.
    pub fn from_token(token: Token) -> Option<Self> {
        match &token {
            Token::Plus => Some(PlusOrMinusOp::Plus),
            Token::Minus => Some(PlusOrMinusOp::Minus),
            _ => None,
        }
    }

    pub fn evaluate_unary(&self, value: Value) -> Result<Value, TracedInterpreterError> {
        let mut number: f64 = value.try_into()?;

        if self == &PlusOrMinusOp::Minus {
            number *= -1.0;
        }

        Ok(number.into())
    }

    pub fn evaluate_binary(
        &self,
        left_side: &Value,
        right_side: &Value,
    ) -> Result<Value, TracedInterpreterError> {
        let result = match (left_side, right_side) {
            (Value::Number(l), Value::Number(r)) => match self {
                PlusOrMinusOp::Plus => l + r,
                PlusOrMinusOp::Minus => l - r,
            },
            _ => return Err(InterpreterError::TypeMismatch.into()),
        };
        Ok(result.into())
    }
}

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
                MultiplyOrDivideOp::Divide => l / r,
            },
            _ => return Err(InterpreterError::TypeMismatch.into()),
        };
        Ok(result.into())
    }
}

pub enum EqualityOp {
    EqualTo,
    LessThan,
    GreaterThan,
    NotEqualTo,
}

impl EqualityOp {
    // I considered TryFrom here but it required an associated Error type
    // and I just wanted to use Option.
    pub fn from_token(token: Token) -> Option<Self> {
        match token {
            Token::Equals => Some(EqualityOp::EqualTo),
            Token::LessThan => Some(EqualityOp::LessThan),
            Token::GreaterThan => Some(EqualityOp::GreaterThan),
            Token::NotEquals => Some(EqualityOp::NotEqualTo),
            _ => None,
        }
    }

    fn evaluate_partial_ord<T: PartialOrd>(&self, left_side: T, right_side: T) -> bool {
        match self {
            EqualityOp::EqualTo => left_side == right_side,
            EqualityOp::LessThan => left_side < right_side,
            EqualityOp::GreaterThan => left_side > right_side,
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
