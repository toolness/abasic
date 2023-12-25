use crate::syntax_error::SyntaxError;

#[derive(Debug, PartialEq)]
pub enum Token {
    Print,
    Goto,
    Newline,
    StringLiteral(String),
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

    pub fn is_basic_whitespace(byte: u8) -> bool {
        byte.is_ascii_whitespace() && byte != b'\n'
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
            if !LineCruncher::is_basic_whitespace(byte) {
                return Some((byte, self.index));
            }
        }

        None
    }
}

pub struct Tokenizer<T: AsRef<str>> {
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

    fn chomp_leading_whitespace(&mut self) {
        let mut cruncher = LineCruncher::new(self.remaining_bytes());
        if cruncher.next().is_some() {
            self.index += cruncher.pos() - 1;
        } else {
            self.index += cruncher.pos();
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

    fn chomp_string(&mut self) -> Option<Result<Token, SyntaxError>> {
        let bytes = self.remaining_bytes();

        assert_ne!(bytes.len(), 0, "we must have remaining bytes to read");

        let first_byte = bytes[0];

        assert!(
            !LineCruncher::is_basic_whitespace(first_byte),
            "first byte must not be BASIC whitespace"
        );

        if first_byte == b'"' {
            let remaining_str = std::str::from_utf8(&bytes[1..]).unwrap();
            if let Some(end_quote_index) = remaining_str.find('"') {
                let string = String::from(&remaining_str[..end_quote_index]);
                self.index += 1 + end_quote_index + 1;
                Some(Ok(Token::StringLiteral(string)))
            } else {
                Some(Err(SyntaxError::UnterminatedStringLiteral))
            }
        } else {
            None
        }
    }

    fn chomp_any_keyword(&mut self) -> Option<Token> {
        if self.chomp_keyword("PRINT") {
            Some(Token::Print)
        } else if self.chomp_keyword("GOTO") {
            Some(Token::Goto)
        } else {
            None
        }
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
        self.chomp_leading_whitespace();

        if self.index == self.bytes().len() {
            return None;
        }

        println!("NEXT {}", self.index);

        if let Some(token) = self.chomp_any_keyword() {
            Some(Ok(token))
        } else if self.chomp_newline() {
            Some(Ok(Token::Newline))
        } else if let Some(result) = self.chomp_string() {
            Some(result)
        } else {
            println!("ILLEGAL CHARACTER AT {}", self.index);
            self.index = self.bytes().len();
            Some(Err(SyntaxError::IllegalCharacter))
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::syntax_error::SyntaxError;

    use super::{Token, Tokenizer};

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
    fn parsing_multiple_tokens_works() {
        for value in ["PRINT GOTO", "PRINTGOTO", "  P R I N T G O T O  "] {
            assert_eq!(get_tokens(value), vec![Token::Print, Token::Goto,]);
        }
    }

    #[test]
    fn parsing_single_string_literal_works() {
        assert_eq!(
            get_tokens("\"Hello there\""),
            vec![Token::StringLiteral(String::from("Hello there"))]
        );
    }

    #[test]
    fn parsing_print_with_string_literal_works() {
        assert_eq!(
            get_tokens("print \"Hello there\""),
            vec![
                Token::Print,
                Token::StringLiteral(String::from("Hello there"))
            ]
        );

        assert_eq!(
            get_tokens("\"Hello there 😊\"PRINT"),
            vec![
                Token::StringLiteral(String::from("Hello there 😊")),
                Token::Print
            ]
        );
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

    #[test]
    fn parsing_borrowed_str_works() {
        let value = String::from("one\ntwo\nthree");
        let first_line = value.split('\n').next().unwrap();
        Tokenizer::new(first_line);
    }
}
