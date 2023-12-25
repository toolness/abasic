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
    string: T,
    index: usize,
}

impl<T: AsRef<str>> Tokenizer<T> {
    pub fn new(string: T) -> Self {
        assert!(string.as_ref().is_ascii());
        Tokenizer { string, index: 0 }
    }

    fn bytes(&self) -> &[u8] {
        self.string.as_ref().as_bytes()
    }

    fn remaining_bytes(&self) -> &[u8] {
        &self.bytes()[self.index..]
    }

    fn chomp_keyword(&mut self, keyword: &str) -> bool {
        let bytes = self.remaining_bytes();
        let keyword_bytes = keyword.as_bytes();
        let mut i = 0;
        let mut keyword_idx = 0;

        while i < bytes.len() {
            let byte = bytes[i];

            i += 1;
            if byte.to_ascii_uppercase() == keyword_bytes[keyword_idx] {
                keyword_idx += 1;
                if keyword_idx == keyword_bytes.len() {
                    self.index += i;
                    return true
                }
            } else if !byte.is_ascii_whitespace() {
                return false
            }
        }

        false
    }
}

impl<T: AsRef<str>> Iterator for Tokenizer<T> {
    type Item = Result<Token, SyntaxError>;

    fn next(&mut self) -> Option<Self::Item> {
        if self.index == self.bytes().len() {
            return None;
        }

        if self.chomp_keyword("PRINT") {
            Some(Ok(Token::Print))
        } else {
            self.index = self.bytes().len();
            Some(Err(SyntaxError {}))
        }
    }
}

fn main() {
    let tokenizer = Tokenizer::new("PRINT \"HELLO WORLD\"");
    for token in tokenizer {
        println!("Token: {:?}", token);
    }
}
