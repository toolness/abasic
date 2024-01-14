use core::fmt::Debug;
use std::collections::HashMap;

use crate::{symbol::Symbol, value::Value, TracedInterpreterError};

#[derive(Default)]
pub struct Variables(HashMap<Symbol, Value>);

impl Debug for Variables {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{:?}", self.0)
    }
}

impl Variables {
    pub fn with_capacity(capacity: usize) -> Self {
        Variables(HashMap::with_capacity(capacity))
    }

    pub fn get(&self, name: &Symbol) -> Value {
        match self.0.get(name) {
            Some(value) => value.clone(),
            None => Value::default_for_variable(name.as_str()),
        }
    }

    pub fn set(&mut self, name: Symbol, value: Value) -> Result<(), TracedInterpreterError> {
        value.validate_type_matches_variable_name(name.as_str())?;
        self.0.insert(name, value);
        Ok(())
    }

    pub fn has(&self, name: &Symbol) -> bool {
        self.0.contains_key(name)
    }
}
