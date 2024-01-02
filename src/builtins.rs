use crate::{interpreter_error::TracedInterpreterError, value::Value};

pub fn abs(value: Value) -> Result<Value, TracedInterpreterError> {
    let number: f64 = value.try_into()?;
    Ok(number.abs().into())
}

pub fn int(value: Value) -> Result<Value, TracedInterpreterError> {
    let number: f64 = value.try_into()?;
    Ok(number.floor().into())
}
