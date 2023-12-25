use std::{error::Error, fmt::Display};

#[derive(Debug, PartialEq)]
enum SyntaxError {
    IllegalCharacter,
}

impl Error for SyntaxError {}

impl Display for SyntaxError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "SYNTAX ERROR ({:?})", self)
    }
}

#[derive(Debug, PartialEq)]
enum Token {
    Print,
    Newline,
}

/// First-generation BASIC dialects completely ignored spaces
/// and tabs. This is part of what made it possible to write
/// either `GO TO` or `GOTO`, for instance.
///
/// This struct allows clients to iterate through the bytes
/// of an array, skipping all such whitespace.
struct LineCruncher<'a> {
    bytes: &'a [u8],
    index: usize,
}

impl<'a> LineCruncher<'a> {
    pub fn new(bytes: &'a [u8]) -> Self {
        LineCruncher { bytes, index: 0 }
    }

    /// Returns the total number of bytes that have been consumed
    /// so far, including whitespace.
    pub fn pos(&self) -> usize {
        self.index
    }
}

impl<'a> Iterator for LineCruncher<'a> {
    /// A tuple of the byte and the total number of bytes consumed
    /// so far, including the given byte and any prior whitespace.
    type Item = (u8, usize);

    fn next(&mut self) -> Option<Self::Item> {
        while self.index < self.bytes.len() {
            let byte = self.bytes[self.index];
            self.index += 1;
            if !(byte.is_ascii_whitespace() && byte != b'\n') {
                return Some((byte, self.index));
            }
        }

        None
    }
}

struct Tokenizer<T: AsRef<str>> {
    string: T,
    index: usize,
}

impl<T: AsRef<str>> Tokenizer<T> {
    pub fn new(string: T) -> Self {
        Tokenizer { string, index: 0 }
    }

    fn bytes(&self) -> &[u8] {
        self.string.as_ref().as_bytes()
    }

    fn remaining_bytes(&self) -> &[u8] {
        &self.bytes()[self.index..]
    }

    fn crunch_remaining_bytes(&self) -> LineCruncher {
        LineCruncher::new(self.remaining_bytes())
    }

    fn chomp_remaining_whitespace(&mut self) -> bool {
        let mut cruncher = LineCruncher::new(self.remaining_bytes());
        if cruncher.next().is_none() {
            self.index += cruncher.pos();
            true
        } else {
            false
        }
    }

    fn chomp_newline(&mut self) -> bool {
        for (byte, pos) in self.crunch_remaining_bytes() {
            if byte == b'\n' {
                self.index += pos;
                return true;
            } else {
                return false;
            }
        }

        false
    }

    fn chomp_keyword(&mut self, keyword: &str) -> bool {
        let keyword_bytes = keyword.as_bytes();
        let mut keyword_idx = 0;

        assert_ne!(keyword_bytes.len(), 0, "keyword must be non-empty");

        for (byte, pos) in self.crunch_remaining_bytes() {
            if byte.to_ascii_uppercase() == keyword_bytes[keyword_idx] {
                keyword_idx += 1;
                if keyword_idx == keyword_bytes.len() {
                    self.index += pos;
                    return true;
                }
            } else {
                return false;
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
        } else if self.chomp_remaining_whitespace() {
            None
        } else if self.chomp_newline() {
            Some(Ok(Token::Newline))
        } else {
            self.index = self.bytes().len();
            Some(Err(SyntaxError::IllegalCharacter))
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::{SyntaxError, Token, Tokenizer};

    fn get_tokens(value: &str) -> Vec<Token> {
        let tokenizer = Tokenizer::new(value);
        tokenizer
            .into_iter()
            .map(|t| t.unwrap())
            .collect::<Vec<_>>()
    }

    #[test]
    fn parsing_empty_string_works() {
        for value in ["", " ", "    "] {
            assert_eq!(get_tokens(value), vec![]);
        }
    }

    #[test]
    fn parsing_single_print_statement_works() {
        for value in ["PRINT", "print", "p r i N t", "PR INT"] {
            assert_eq!(get_tokens(value), vec![Token::Print]);
        }
    }

    #[test]
    fn parsing_single_newline_works() {
        for value in ["\n", " \n", "  \n  "] {
            assert_eq!(get_tokens(value), vec![Token::Newline]);
        }
    }

    #[test]
    fn parsing_single_illegal_character_works() {
        for value in ["?", " %", "😊"] {
            let mut tokenizer = Tokenizer::new(value);
            assert_eq!(tokenizer.next(), Some(Err(SyntaxError::IllegalCharacter)));
            assert_eq!(tokenizer.next(), None);
        }
    }
}

fn main() {
    let tokenizer = Tokenizer::new("PRINT \"HELLO WORLD\"");
    for token in tokenizer {
        println!("Token: {:?}", token);
    }
}
