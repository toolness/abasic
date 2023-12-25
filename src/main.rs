use std::{error::Error, fmt::Display};

#[derive(Debug)]
struct SyntaxError {
}

impl Error for SyntaxError {}

impl Display for SyntaxError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "SYNTAX ERROR")
    }
}

#[derive(Debug)]
enum Token {
    Print,
}

struct Tokenizer<T: AsRef<str>> {
    string: T
}

impl<T: AsRef<str>> Tokenizer<T> {
    pub fn new(string: T) -> Self {
        assert!(string.as_ref().is_ascii());
        Tokenizer { string }
    }
}

impl<T: AsRef<str>> Iterator for Tokenizer<T> {
    type Item = Result<Token, SyntaxError>;

    fn next(&mut self) -> Option<Self::Item> {
        if self.string.as_ref().contains("PRINT") {
            Some(Ok(Token::Print))
        } else {
            None
        }
    }
}

fn main() {
    let mut tok = Tokenizer::new("PRINT \"HELLO WORLD\"");
    println!("First token: {:?}", tok.next());
}
