use std::rc::Rc;

use crate::syntax_error::SyntaxError;

#[derive(Debug, PartialEq, Clone)]
pub enum Token {
    Print,
    Goto,
    Newline,
    Plus,
    Minus,
    StringLiteral(Rc<String>),
    NumericLiteral(f64),
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
    errored: bool,
}

impl<T: AsRef<str>> Tokenizer<T> {
    pub fn new(string: T) -> Self {
        Tokenizer {
            string,
            index: 0,
            errored: false,
        }
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

    fn chomp_single_character(&mut self) -> Option<Result<Token, SyntaxError>> {
        for (byte, pos) in self.crunch_remaining_bytes() {
            let token: Token = match byte {
                b'\n' => Token::Newline,
                b'+' => Token::Plus,
                b'-' => Token::Minus,
                _ => return None,
            };
            self.index += pos;
            return Some(Ok(token));
        }

        None
    }

    fn chomp_number(&mut self) -> Option<Result<Token, SyntaxError>> {
        let mut digits = String::new();
        let mut latest_pos: Option<usize> = None;

        for (byte, pos) in self.crunch_remaining_bytes() {
            // TODO: We should support decimals too.
            if byte.is_ascii_digit() {
                latest_pos = Some(pos);
                digits.push(byte as char);
            } else {
                break;
            }
        }

        if let Some(pos) = latest_pos {
            if let Ok(number) = digits.parse::<f64>() {
                self.index += pos;
                Some(Ok(Token::NumericLiteral(number)))
            } else {
                Some(Err(SyntaxError::InvalidNumber))
            }
        } else {
            None
        }
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
            // Technically this isn't very efficient because it's re-validating that
            // the rest of the string is valid UTF-8, which it *should* be based on
            // how we've been processing the bytes that came before, but better
            // safe than sorry I guess.
            let remaining_str = std::str::from_utf8(&bytes[1..]).unwrap();

            if let Some(end_quote_index) = remaining_str.find('"') {
                let string = Rc::new(String::from(&remaining_str[..end_quote_index]));
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

    fn chomp_next_token(&mut self) -> Result<Token, SyntaxError> {
        if let Some(token) = self.chomp_any_keyword() {
            Ok(token)
        } else if let Some(result) = self.chomp_single_character() {
            result
        } else if let Some(result) = self.chomp_string() {
            result
        } else if let Some(result) = self.chomp_number() {
            result
        } else {
            Err(SyntaxError::IllegalCharacter)
        }
    }
}

impl<T: AsRef<str>> Iterator for Tokenizer<T> {
    type Item = Result<Token, SyntaxError>;

    fn next(&mut self) -> Option<Self::Item> {
        if self.errored {
            return None;
        }

        self.chomp_leading_whitespace();

        if self.index == self.bytes().len() {
            return None;
        }

        let result = self.chomp_next_token();

        if result.is_err() {
            self.errored = true;
        }

        Some(result)
    }
}

#[cfg(test)]
mod tests {
    use std::rc::Rc;

    use crate::syntax_error::SyntaxError;

    use super::{Token, Tokenizer};

    fn string_literal(value: &'static str) -> Token {
        Token::StringLiteral(Rc::new(String::from(value)))
    }

    fn get_tokens_wrapped(value: &str) -> Vec<Result<Token, SyntaxError>> {
        let tokenizer = Tokenizer::new(value);
        tokenizer.into_iter().collect::<Vec<_>>()
    }

    fn get_tokens(value: &str) -> Vec<Token> {
        let tokenizer = Tokenizer::new(value);
        tokenizer
            .into_iter()
            .map(|t| match t {
                Ok(token) => token,
                Err(err) => {
                    panic!(
                        "expected '{}' to tokenize without error, but got {:?}",
                        value, err
                    )
                }
            })
            .collect::<Vec<_>>()
    }

    fn assert_values_parse_to_tokens(values: &[&str], tokens: &[Token]) {
        for value in values {
            assert_eq!(
                get_tokens(value),
                tokens.to_owned(),
                "parsing '{}' == {:?}",
                value,
                tokens
            );
        }
    }

    fn assert_values_parse_to_tokens_wrapped(
        values: &[&str],
        tokens: &[Result<Token, SyntaxError>],
    ) {
        for value in values {
            assert_eq!(
                get_tokens_wrapped(value),
                tokens.to_owned(),
                "parsing '{}' == {:?}",
                value,
                tokens
            );
        }
    }

    #[test]
    fn parsing_empty_string_works() {
        assert_values_parse_to_tokens(&["", " ", "    "], &[]);
    }

    #[test]
    fn parsing_single_print_statement_works() {
        assert_values_parse_to_tokens(&["PRINT", "print", "p r i N t", "PR INT"], &[Token::Print]);
    }

    #[test]
    fn parsing_multiple_tokens_works() {
        assert_values_parse_to_tokens(
            &["PRINT GOTO", "PRINTGOTO", "  P R I N T G O T O  "],
            &[Token::Print, Token::Goto],
        );
    }

    #[test]
    fn parsing_single_string_literal_works() {
        assert_values_parse_to_tokens(&["\"Hello there\""], &[string_literal("Hello there")]);
    }

    #[test]
    fn parsing_print_with_string_literal_works() {
        assert_values_parse_to_tokens(
            &["print \"Hello there\""],
            &[Token::Print, string_literal("Hello there")],
        );

        assert_values_parse_to_tokens(
            &["\"Hello there 😊\"PRINT"],
            &[string_literal("Hello there 😊"), Token::Print],
        );
    }

    #[test]
    fn parsing_single_numeric_literal_works() {
        assert_values_parse_to_tokens(
            &["1234", "  1234 ", "01234", "1 2 3 4"],
            &[Token::NumericLiteral(1234.0)],
        );
    }

    #[test]
    fn parsing_single_numeric_literal_and_print_works() {
        assert_eq!(
            get_tokens("1234 PRINT"),
            vec![Token::NumericLiteral(1234.0), Token::Print]
        );
    }

    #[test]
    fn parsing_single_newline_works() {
        assert_values_parse_to_tokens(&["\n", " \n", "  \n  "], &[Token::Newline]);
    }

    #[test]
    fn parsing_single_illegal_character_returns_error() {
        assert_values_parse_to_tokens_wrapped(
            &["?", " %", "😊"],
            &[Err(SyntaxError::IllegalCharacter)],
        );
    }

    #[test]
    fn parsing_unterminated_string_literal_returns_error() {
        assert_values_parse_to_tokens_wrapped(
            &["\"", " \"blarg"],
            &[Err(SyntaxError::UnterminatedStringLiteral)],
        );
    }

    #[test]
    fn parsing_borrowed_str_works() {
        let value = String::from("one\ntwo\nthree");
        let first_line = value.split('\n').next().unwrap();
        Tokenizer::new(first_line);
    }
}
