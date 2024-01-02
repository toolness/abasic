use std::rc::Rc;

use crate::{
    data::DataElement,
    interpreter_error::{InterpreterError, TracedInterpreterError},
};

#[derive(Debug, Clone, PartialEq)]
pub enum Value {
    String(Rc<String>),
    Number(f64),
}

impl TryFrom<Value> for f64 {
    type Error = TracedInterpreterError;

    fn try_from(value: Value) -> Result<Self, Self::Error> {
        if let Value::Number(number) = value {
            Ok(number)
        } else {
            Err(InterpreterError::TypeMismatch.into())
        }
    }
}

impl Value {
    // TODO: Should we use the `From` trait instead?  Or is this more explicit?
    pub fn to_bool(&self) -> bool {
        match self {
            Value::String(string) => !string.is_empty(),
            Value::Number(number) => *number != 0.0,
        }
    }

    pub fn coerce_from_data_element<T: AsRef<str>>(
        variable_name: T,
        data_element: &DataElement,
    ) -> Result<Value, TracedInterpreterError> {
        if variable_name.as_ref().ends_with('$') {
            match data_element {
                DataElement::String(s) => Ok(Value::String(s.clone())),
                DataElement::Number(n) => Ok(Value::String(Rc::new((*n).to_string()))),
            }
        } else {
            match data_element {
                DataElement::String(_) => Err(InterpreterError::DataTypeMismatch.into()),
                DataElement::Number(n) => Ok(Value::Number(*n)),
            }
        }
    }

    pub fn default_for_variable<T: AsRef<str>>(variable_name: T) -> Self {
        if variable_name.as_ref().ends_with('$') {
            String::default().into()
        } else {
            f64::default().into()
        }
    }

    pub fn validate_type_matches_variable_name<T: AsRef<str>>(
        &self,
        variable_name: T,
    ) -> Result<(), TracedInterpreterError> {
        if variable_name.as_ref().ends_with('$') {
            match self {
                Value::String(_) => Ok(()),
                Value::Number(_) => Err(InterpreterError::TypeMismatch.into()),
            }
        } else {
            match self {
                Value::String(_) => Err(InterpreterError::TypeMismatch.into()),
                Value::Number(_) => Ok(()),
            }
        }
    }
}

impl From<String> for Value {
    fn from(value: String) -> Self {
        Value::String(Rc::new(value))
    }
}

impl From<Rc<String>> for Value {
    fn from(value: Rc<String>) -> Self {
        Value::String(value.clone())
    }
}

impl From<f64> for Value {
    fn from(value: f64) -> Self {
        Value::Number(value)
    }
}
