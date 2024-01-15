use crate::InterpreterError;

#[derive(Debug, PartialEq)]
pub enum ValueType {
    String,
    Number,
}

impl ValueType {
    pub fn check(&self, expected: ValueType) -> Result<ValueType, InterpreterError> {
        if self == &expected {
            Ok(expected)
        } else {
            Err(InterpreterError::TypeMismatch)
        }
    }

    pub fn check_number(&self) -> Result<ValueType, InterpreterError> {
        self.check(ValueType::Number)
    }

    pub fn check_variable_name<T: AsRef<str>>(
        &self,
        name: T,
    ) -> Result<ValueType, InterpreterError> {
        self.check(ValueType::from_variable_name(name))
    }

    pub fn from_variable_name<T: AsRef<str>>(name: T) -> Self {
        if name.as_ref().ends_with('$') {
            ValueType::String
        } else {
            ValueType::Number
        }
    }
}
