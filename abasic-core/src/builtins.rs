use crate::{interpreter_error::TracedInterpreterError, value::Value};

type ValueResult = Result<Value, TracedInterpreterError>;

pub fn abs(value: Value) -> ValueResult {
    let number: f64 = value.try_into()?;
    Ok(number.abs().into())
}

pub fn int(value: Value) -> ValueResult {
    let number: f64 = value.try_into()?;
    Ok(number.floor().into())
}
