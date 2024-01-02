use crate::{
    interpreter_error::{InterpreterError, TracedInterpreterError},
    value::Value,
};

type ValueResult = Result<Value, TracedInterpreterError>;

pub fn abs(value: Value) -> ValueResult {
    let number: f64 = value.try_into()?;
    Ok(number.abs().into())
}

pub fn int(value: Value) -> ValueResult {
    let number: f64 = value.try_into()?;
    Ok(number.floor().into())
}

pub fn rnd(value: Value) -> ValueResult {
    // Applesoft BASIC would always return the most recent random with the argument '0', and
    // predefined items in the sequence with '-1', but in practice all the code I've seen
    // just calls it with '1', and *any* positive number is supposed to return a random number
    // in the interval [0, 1), so we'll just support that.
    let number: f64 = value.try_into()?;
    if number <= 0.0 {
        Err(InterpreterError::Unimplemented.into())
    } else {
        Ok(fastrand::f64().into())
    }
}
