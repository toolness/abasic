use std::{fmt::Display, rc::Rc};

use crate::{
    data::{data_elements_to_string, parse_data_until_colon, DataElement},
    interner::StringInterner,
    line_cruncher::LineCruncher,
    syntax_error::SyntaxError,
};

#[derive(Debug, PartialEq, Clone)]
pub enum Token {
    Dim,
    Let,
    Print,
    Input,
    Goto,
    Gosub,
    Return,
    Colon,
    Semicolon,
    Comma,
    QuestionMark,
    LeftParen,
    RightParen,
    Plus,
    Minus,
    Multiply,
    Divide,
    Caret,
    Equals,
    NotEquals,
    LessThan,
    LessThanOrEqualTo,
    GreaterThan,
    GreaterThanOrEqualTo,
    And,
    Or,
    Not,
    If,
    Then,
    Else,
    End,
    Stop,
    For,
    To,
    Step,
    Next,
    Read,
    Restore,
    Def,
    Remark(Rc<String>),
    Symbol(Rc<String>),
    StringLiteral(Rc<String>),
    NumericLiteral(f64),
    Data(Rc<Vec<DataElement>>),
}

impl Display for Token {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Token::Dim => write!(f, "DIM"),
            Token::Let => write!(f, "LET"),
            Token::Print => write!(f, "PRINT"),
            Token::Input => write!(f, "INPUT"),
            Token::Goto => write!(f, "GOTO"),
            Token::Gosub => write!(f, "GOSUB"),
            Token::Return => write!(f, "RETURN"),
            Token::Colon => write!(f, ":"),
            Token::Semicolon => write!(f, ";"),
            Token::Comma => write!(f, ","),
            Token::QuestionMark => write!(f, "?"),
            Token::LeftParen => write!(f, "("),
            Token::RightParen => write!(f, ")"),
            Token::Plus => write!(f, "+"),
            Token::Minus => write!(f, "-"),
            Token::Multiply => write!(f, "*"),
            Token::Divide => write!(f, "/"),
            Token::Caret => write!(f, "^"),
            Token::Equals => write!(f, "="),
            Token::NotEquals => write!(f, "<>"),
            Token::LessThan => write!(f, "<"),
            Token::LessThanOrEqualTo => write!(f, "<="),
            Token::GreaterThan => write!(f, ">"),
            Token::GreaterThanOrEqualTo => write!(f, ">="),
            Token::And => write!(f, "AND"),
            Token::Or => write!(f, "OR"),
            Token::Not => write!(f, "NOT"),
            Token::If => write!(f, "IF"),
            Token::Then => write!(f, "THEN"),
            Token::Else => write!(f, "ELSE"),
            Token::End => write!(f, "END"),
            Token::Stop => write!(f, "STOP"),
            Token::For => write!(f, "FOR"),
            Token::To => write!(f, "TO"),
            Token::Step => write!(f, "STEP"),
            Token::Next => write!(f, "NEXT"),
            Token::Read => write!(f, "READ"),
            Token::Restore => write!(f, "RESTORE"),
            Token::Def => write!(f, "DEF"),
            Token::Remark(comment) => write!(f, "REM {}", comment),
            Token::Symbol(name) => write!(f, "{}", name),
            Token::StringLiteral(string) => write!(f, "\"{}\"", string),
            Token::NumericLiteral(number) => write!(f, "{}", number),
            Token::Data(elements) => write!(f, "{}", data_elements_to_string(elements)),
        }
    }
}

pub struct Tokenizer<T: AsRef<str>> {
    string: T,
    index: usize,
    errored: bool,
    interner: StringInterner,
}

impl<T: AsRef<str>> Tokenizer<T> {
    pub fn new(string: T) -> Self {
        Tokenizer {
            string,
            index: 0,
            errored: false,
            interner: StringInterner::default(),
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

    fn chomp_one_or_two_characters(&mut self) -> Option<Result<Token, SyntaxError>> {
        if let Some((byte, pos)) = self.crunch_remaining_bytes().next() {
            let token: Token = match byte {
                b':' => Token::Colon,
                b';' => Token::Semicolon,
                b',' => Token::Comma,
                b'?' => Token::QuestionMark,
                b'(' => Token::LeftParen,
                b')' => Token::RightParen,
                b'+' => Token::Plus,
                b'-' => Token::Minus,
                b'*' => Token::Multiply,
                b'/' => Token::Divide,
                b'^' => Token::Caret,
                b'=' => Token::Equals,
                b'<' => Token::LessThan,
                b'>' => Token::GreaterThan,
                _ => return None,
            };
            self.index += pos;

            if token == Token::LessThan {
                if let Some((next_char, pos)) = self.crunch_remaining_bytes().next() {
                    if next_char == b'>' {
                        self.index += pos;
                        return Some(Ok(Token::NotEquals));
                    } else if next_char == b'=' {
                        self.index += pos;
                        return Some(Ok(Token::LessThanOrEqualTo));
                    }
                }
            } else if token == Token::GreaterThan {
                if let Some((next_char, pos)) = self.crunch_remaining_bytes().next() {
                    if next_char == b'=' {
                        self.index += pos;
                        return Some(Ok(Token::GreaterThanOrEqualTo));
                    }
                }
            }

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

            let char_is_dollar_sign = char == b'$';

            let is_valid = if chars.is_empty() {
                char.is_ascii_alphabetic()
            } else {
                char.is_ascii_alphanumeric() || char_is_dollar_sign
            };

            if !is_valid {
                break;
            }

            chars.push(char.to_ascii_uppercase());
            self.index += pos;

            if char_is_dollar_sign {
                break;
            }

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

            Some(Ok(Token::Symbol(self.interner.get(string))))
        }
    }

    fn chomp_number(&mut self) -> Option<Result<Token, SyntaxError>> {
        let mut digits = String::new();
        let mut latest_pos: Option<usize> = None;

        for (byte, pos) in self.crunch_remaining_bytes() {
            // TODO: We don't currently support scientific notation like ".1e10".

            // Note that we're not concerned with whether the decimal is in
            // the right place, we'll deal with that later when we parse the
            // final number.
            if byte.is_ascii_digit() || byte == b'.' {
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

            // TODO: There doesn't seem to be any way to escape a double-quote,
            // and I'm not sure what BASIC conventions for this are, if any. It'd
            // be nice to somehow support this.
            if let Some(end_quote_index) = remaining_str.find('"') {
                let string = self
                    .interner
                    .get(String::from(&remaining_str[..end_quote_index]));
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
            Some(Ok(Token::Remark(self.interner.get(comment))))
        } else {
            None
        }
    }

    fn chomp_any_keyword(&mut self) -> Option<Token> {
        if self.chomp_keyword("DIM") {
            Some(Token::Dim)
        } else if self.chomp_keyword("LET") {
            Some(Token::Let)
        } else if self.chomp_keyword("PRINT") {
            Some(Token::Print)
        } else if self.chomp_keyword("INPUT") {
            Some(Token::Input)
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
        } else if self.chomp_keyword("ELSE") {
            Some(Token::Else)
        } else if self.chomp_keyword("AND") {
            Some(Token::And)
        } else if self.chomp_keyword("OR") {
            Some(Token::Or)
        } else if self.chomp_keyword("NOT") {
            Some(Token::Not)
        } else if self.chomp_keyword("END") {
            Some(Token::End)
        } else if self.chomp_keyword("STOP") {
            Some(Token::Stop)
        } else if self.chomp_keyword("FOR") {
            Some(Token::For)
        } else if self.chomp_keyword("TO") {
            Some(Token::To)
        } else if self.chomp_keyword("NEXT") {
            Some(Token::Next)
        } else if self.chomp_keyword("STEP") {
            Some(Token::Step)
        } else if self.chomp_keyword("READ") {
            Some(Token::Read)
        } else if self.chomp_keyword("RESTORE") {
            Some(Token::Restore)
        } else if self.chomp_keyword("DEF") {
            Some(Token::Def)
        } else {
            None
        }
    }

    fn chomp_data(&mut self) -> Option<Token> {
        if self.chomp_keyword("DATA") {
            // Note that we're not inlining remaining_bytes() here because we
            // also want to get a mutable reference to our interner, and Rust's
            // borrow checker will complain if we use the method directly, because
            // using the method marks the entire struct as borrowed, rather than
            // just one field.
            let remaining_bytes = &self.string.as_ref().as_bytes()[self.index..];

            // We can technically do this using String::from_utf8_unchecked(),
            // but better safe (and slightly inefficient) than sorry for now.
            let remaining = std::str::from_utf8(remaining_bytes).unwrap();

            let (elements, bytes_chomped) =
                parse_data_until_colon(remaining, Some(&mut self.interner));

            self.index += bytes_chomped;

            Some(Token::Data(Rc::new(elements)))
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
        } else if let Some(result) = self.chomp_one_or_two_characters() {
            result
        } else if let Some(result) = self.chomp_string() {
            result
        } else if let Some(result) = self.chomp_number() {
            result
        } else if let Some(result) = self.chomp_remark() {
            result
        } else if let Some(result) = self.chomp_data() {
            Ok(result)
        } else if let Some(result) = self.chomp_symbol() {
            result
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
    fn parsing_decimal_number_works() {
        assert_values_parse_to_tokens(
            &[".1", " .1", " .10 ", "0000.10000"],
            &[Token::NumericLiteral(0.1)],
        );
    }

    #[test]
    fn parsing_invalid_decimal_number_returns_error() {
        assert_values_parse_to_tokens_wrapped(
            &[".1.", " .1..", " ..10 "],
            &[Err(SyntaxError::InvalidNumber)],
        );
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
    fn parsing_equality_operators_works() {
        assert_values_parse_to_tokens(&["<>", " <>", "  <>  ", "< >"], &[Token::NotEquals]);
        assert_values_parse_to_tokens(&["<", " <", "  <  "], &[Token::LessThan]);
        assert_values_parse_to_tokens(&[">", " >", "  >  "], &[Token::GreaterThan]);
        assert_values_parse_to_tokens(&["=", " =", "  =  "], &[Token::Equals]);

        assert_values_parse_to_tokens(
            &["<> =", " <>=", "  <> = ", "< > ="],
            &[Token::NotEquals, Token::Equals],
        );
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
    fn parsing_symbol_with_dollar_sign_works() {
        assert_values_parse_to_tokens(&["x$", " x $", "  x$  "], &[symbol("X$")]);
        assert_values_parse_to_tokens(&["x$u", " x $u", "  x$u  "], &[symbol("X$"), symbol("U")]);
    }

    #[test]
    fn parsing_data_works() {
        use crate::data::test_util::{number, string};

        assert_values_parse_to_tokens(
            &["DATA A, b, C, 4:print", "DATA    A, \"b\", C, 4 : print"],
            &[
                Token::Data(Rc::new(vec![
                    string("A"),
                    string("b"),
                    string("C"),
                    number(4.0),
                ])),
                Token::Colon,
                Token::Print,
            ],
        );
    }

    #[test]
    fn parsing_single_illegal_character_returns_error() {
        assert_values_parse_to_tokens_wrapped(
            &[" %", "😊", "\n", "$"],
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
