mod utils;

use abasic_core::{Interpreter, InterpreterOutput, InterpreterState};
use wasm_bindgen::prelude::*;

use crate::utils::set_panic_hook;

#[wasm_bindgen]
pub enum JsInterpreterState {
    Idle,
    Running,
    AwaitingInput,
    Errored,
}

#[wasm_bindgen]
#[derive(Copy, Clone)]
pub enum JsInterpreterOutputType {
    Print,
    Break,
    Warning,
    Trace,
    ExtraIgnored,
    Reenter,
}

#[wasm_bindgen]
/// wasm-bindgen only supports C-Style enums, for tagged unions it looks like
/// I need to use serde, but for now I'm just gonna use this struct instead.
pub struct JsInterpreterOutput {
    pub output_type: JsInterpreterOutputType,
    value: String,
}

#[wasm_bindgen]
impl JsInterpreterOutput {
    pub fn into_string(self) -> String {
        self.value
    }
}

fn convert_interpreter_output_for_js(value: InterpreterOutput) -> JsInterpreterOutput {
    let output_type: JsInterpreterOutputType = match value {
        InterpreterOutput::Print(_) => JsInterpreterOutputType::Print,
        InterpreterOutput::Break(_) => JsInterpreterOutputType::Break,
        InterpreterOutput::Warning(_, _) => JsInterpreterOutputType::Warning,
        InterpreterOutput::Trace(_) => JsInterpreterOutputType::Trace,
        InterpreterOutput::ExtraIgnored => JsInterpreterOutputType::ExtraIgnored,
        InterpreterOutput::Reenter => JsInterpreterOutputType::Reenter,
    };
    JsInterpreterOutput {
        output_type,
        value: value.to_string(),
    }
}

#[wasm_bindgen]
pub struct JsInterpreter {
    interpreter: Interpreter,
    latest_error: Option<String>,
}

#[wasm_bindgen]
impl JsInterpreter {
    pub fn new() -> Self {
        set_panic_hook();
        JsInterpreter {
            interpreter: Interpreter::new(),
            latest_error: None,
        }
    }

    fn maybe_replace_interpreter(&mut self) {
        if self.interpreter.get_state() == InterpreterState::NewInterpreterRequested {
            self.interpreter = Interpreter::new();
        }
    }

    pub fn provide_input(&mut self, input: String) {
        self.interpreter.provide_input(input);
    }

    pub fn take_latest_output(&mut self) -> Vec<JsInterpreterOutput> {
        self.interpreter
            .take_output()
            .into_iter()
            .map(|output| convert_interpreter_output_for_js(output))
            .collect::<Vec<_>>()
    }

    pub fn take_latest_error(&mut self) -> Option<String> {
        self.latest_error.take()
    }

    pub fn start_evaluating(&mut self, line: String) {
        assert!(self.latest_error.is_none());
        if let Err(err) = self.interpreter.start_evaluating(line) {
            self.latest_error = Some(err.to_string());
        } else {
            self.maybe_replace_interpreter();
        }
    }

    pub fn continue_evaluating(&mut self) {
        assert!(self.latest_error.is_none());
        if let Err(err) = self.interpreter.continue_evaluating() {
            self.latest_error = Some(err.to_string());
        } else {
            self.maybe_replace_interpreter();
        }
    }

    pub fn get_state(&self) -> JsInterpreterState {
        if self.latest_error.is_some() {
            return JsInterpreterState::Errored;
        }
        match self.interpreter.get_state() {
            abasic_core::InterpreterState::Idle => JsInterpreterState::Idle,
            abasic_core::InterpreterState::Running => JsInterpreterState::Running,
            abasic_core::InterpreterState::AwaitingInput => JsInterpreterState::AwaitingInput,
            abasic_core::InterpreterState::NewInterpreterRequested => {
                panic!("Underlying interpreter should never be in this state")
            }
        }
    }
}
