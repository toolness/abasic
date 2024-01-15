use core::fmt::Debug;
use std::{fmt::Display, rc::Rc};

/// This is a newtype for a symbol (e.g. an identifier referencing the name
/// of a variable or function).
///
/// Using a newtype allows us to easily change the implementation without
/// needing to change a bunch of dependent code.
#[derive(PartialEq, Clone, Hash, Eq)]
pub struct Symbol(Rc<String>);

impl Symbol {
    pub fn as_str(&self) -> &str {
        self.0.as_str()
    }
}

impl Display for Symbol {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl AsRef<str> for Symbol {
    fn as_ref(&self) -> &str {
        self.0.as_ref()
    }
}

impl Debug for Symbol {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl Into<Symbol> for Rc<String> {
    fn into(self) -> Symbol {
        Symbol(self)
    }
}
