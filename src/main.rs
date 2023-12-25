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

struct Tokenizer {
    string: String
}

impl Tokenizer {
    pub fn new(string: String) -> Self {
        assert!(string.is_ascii());
        Tokenizer { string }
    }
}

impl Iterator for Tokenizer {
    type Item = Result<Token, SyntaxError>;

    fn next(&mut self) -> Option<Self::Item> {
        if self.string.contains("PRINT") {
            Some(Ok(Token::Print))
        } else {
            None
        }
    }
}

fn main() {
    let mut tok = Tokenizer::new(String::from("PRINT \"HELLO WORLD\""));
    println!("First token: {:?}", tok.next());
}
