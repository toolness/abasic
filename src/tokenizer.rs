use std::{fmt::Display, rc::Rc};

use crate::{line_cruncher::LineCruncher, syntax_error::SyntaxError};

#[derive(Debug, PartialEq, Clone)]
pub enum DataElement {
    String(Rc<String>),
    Number(f64),
}

#[derive(Debug, PartialEq, Clone)]
pub enum Token {
    Print,
    Goto,
    Gosub,
    Return,
    Colon,
    Plus,
    Minus,
    Equals,
    If,
    Then,
    End,
    For,
    To,
    Next,
    Remark(Rc<String>),
    Symbol(Rc<String>),
    StringLiteral(Rc<String>),
    NumericLiteral(f64),
    Data(Rc<Vec<DataElement>>),
}

impl Display for Token {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Token::Print => write!(f, "PRINT"),
            Token::Goto => write!(f, "GOTO"),
            Token::Gosub => write!(f, "GOSUB"),
            Token::Return => write!(f, "RETURN"),
            Token::Colon => write!(f, ":"),
            Token::Plus => write!(f, "+"),
            Token::Minus => write!(f, "-"),
            Token::Equals => write!(f, "="),
            Token::If => write!(f, "IF"),
            Token::Then => write!(f, "THEN"),
            Token::End => write!(f, "END"),
            Token::For => write!(f, "FOR"),
            Token::To => write!(f, "TO"),
            Token::Next => write!(f, "NEXT"),
            Token::Remark(comment) => write!(f, "REM {}", comment),
            Token::Symbol(name) => write!(f, "{}", name),
            Token::StringLiteral(string) => write!(f, "\"{}\"", string),
            Token::NumericLiteral(number) => write!(f, "{}", number),
            Token::Data(elements) => write!(f, "{}", data_to_string(elements)),
        }
    }
}

fn data_to_string(elements: &Vec<DataElement>) -> String {
    elements
        .iter()
        .map(|element| match element {
            DataElement::String(string) => format!("\"{}\"", string),
            DataElement::Number(number) => number.to_string(),
        })
        .collect::<Vec<_>>()
        .join(", ")
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
                b':' => Token::Colon,
                b'+' => Token::Plus,
                b'-' => Token::Minus,
                b'=' => Token::Equals,
                _ => return None,
            };
            self.index += pos;
            return Some(Ok(token));
        }

        None
    }

    fn chomp_symbol(&mut self) -> Option<Result<Token, SyntaxError>> {
        let mut chars: Vec<u8> = vec![];

        loop {
            let mut remaining = self.crunch_remaining_bytes();
            let Some((char, pos)) = remaining.next() else {
                break;
            };

            // TODO: Symbols can end with `$`, we should support it.
            let is_valid = if chars.is_empty() {
                char.is_ascii_alphabetic()
            } else {
                char.is_ascii_alphanumeric()
            };

            if !is_valid {
                break;
            }

            chars.push(char.to_ascii_uppercase());
            self.index += pos;

            // Because of line crunching, it's possible that we have a
            // keyword immediately following a symbol. If this happens,
            // we need to give precedence to the keyword, rather than
            // extend the name of the symbol, e.g. `if x then y` should
            // never be parsed as `if` followed by a `xtheny` symbol.
            let prev_index = self.index;
            if self.chomp_any_keyword().is_some() {
                self.index = prev_index;
                break;
            }
        }

        if chars.is_empty() {
            None
        } else {
            // We can technically do this using String::from_utf8_unchecked(),
            // but better safe (and slightly inefficient) than sorry for now.
            let string = String::from_utf8(chars).unwrap();

            Some(Ok(Token::Symbol(Rc::new(string))))
        }
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

    fn chomp_remark(&mut self) -> Option<Result<Token, SyntaxError>> {
        if self.chomp_keyword("REM") {
            let bytes = self.remaining_bytes();

            // We can technically do this using from_utf8_unchecked(),
            // but better safe (and slightly inefficient) than sorry for now.
            let comment = std::str::from_utf8(bytes).unwrap().to_string();

            self.index += comment.len();
            Some(Ok(Token::Remark(Rc::new(comment))))
        } else {
            None
        }
    }

    fn chomp_any_keyword(&mut self) -> Option<Token> {
        if self.chomp_keyword("PRINT") {
            Some(Token::Print)
        } else if self.chomp_keyword("GOTO") {
            Some(Token::Goto)
        } else if self.chomp_keyword("GOSUB") {
            Some(Token::Gosub)
        } else if self.chomp_keyword("RETURN") {
            Some(Token::Return)
        } else if self.chomp_keyword("IF") {
            Some(Token::If)
        } else if self.chomp_keyword("THEN") {
            Some(Token::Then)
        } else if self.chomp_keyword("END") {
            Some(Token::End)
        } else if self.chomp_keyword("FOR") {
            Some(Token::For)
        } else if self.chomp_keyword("TO") {
            Some(Token::To)
        } else if self.chomp_keyword("NEXT") {
            Some(Token::Next)
        } else {
            None
        }
    }

    fn chomp_data(&mut self) -> Option<Token> {
        if self.chomp_keyword("DATA") {
            // TODO: Finish this
            Some(Token::Data(Rc::new(vec![])))
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
        } else if let Some(result) = self.chomp_remark() {
            result
        } else if let Some(result) = self.chomp_symbol() {
            result
        } else if let Some(result) = self.chomp_data() {
            Ok(result)
        } else {
            Err(SyntaxError::IllegalCharacter)
        }
    }

    pub fn remaining_tokens(self) -> Result<Vec<Token>, SyntaxError> {
        let mut tokens: Vec<Token> = vec![];
        for token in self {
            tokens.push(token?);
        }
        Ok(tokens)
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

    fn symbol(value: &'static str) -> Token {
        Token::Symbol(Rc::new(String::from(value)))
    }

    fn remark(value: &'static str) -> Token {
        Token::Remark(Rc::new(String::from(value)))
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
    fn parsing_single_remark_works() {
        assert_values_parse_to_tokens(&["REM hi"], &[remark(" hi")]);
        assert_values_parse_to_tokens(&["REM hi:print"], &[remark(" hi:print")]);
        assert_values_parse_to_tokens(&["REM hi 😊"], &[remark(" hi 😊")]);
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
    fn parsing_if_statement_works() {
        assert_values_parse_to_tokens(
            &["if x then print"],
            &[Token::If, symbol("X"), Token::Then, Token::Print],
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
    fn parsing_single_colon_works() {
        assert_values_parse_to_tokens(&[":", " :", "  :  "], &[Token::Colon]);
    }

    #[test]
    fn parsing_symbol_works() {
        assert_values_parse_to_tokens(&["x", " x", "  x  "], &[symbol("X")]);
        assert_values_parse_to_tokens(&["x 1", " x1", "  X1  "], &[symbol("X1")]);
        assert_values_parse_to_tokens(
            &["1 x 1", " 1x1", "  1X1  "],
            &[Token::NumericLiteral(1.0), symbol("X1")],
        );
    }

    #[test]
    fn parsing_single_illegal_character_returns_error() {
        assert_values_parse_to_tokens_wrapped(
            &["?", " %", "😊", "\n"],
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
