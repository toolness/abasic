mod utils;

use abasic_core::{Interpreter, TracedInterpreterError};
use wasm_bindgen::prelude::*;

#[wasm_bindgen]
pub fn evaluate_basic(value: String) -> String {
    match evaluate_basic_impl(value) {
        Ok(string) => string,
        Err(err) => err.to_string(),
    }
}

fn evaluate_basic_impl(value: String) -> Result<String, TracedInterpreterError> {
    let mut interpreter = Interpreter::new();
    interpreter.start_evaluating(value)?;
    let mut i = 0;
    loop {
        i += 1;
        if i > 3000 {
            return Ok(String::from("TOO MANY ITERATIONS"));
        }
        match interpreter.get_state() {
            abasic_core::InterpreterState::Idle => break,
            abasic_core::InterpreterState::Running => interpreter.continue_evaluating()?,
            abasic_core::InterpreterState::AwaitingInput => {
                return Ok(String::from("TODO: IMPLEMENT INPUT"))
            }
            abasic_core::InterpreterState::NewInterpreterRequested => {
                return Ok(String::from("TODO: IMPLEMENT NEW INTERPRETER REQUEST"))
            }
        }
    }
    let output = interpreter
        .take_output()
        .into_iter()
        .map(|output| output.to_string())
        .collect::<Vec<_>>()
        .join("");
    Ok(output)
}
