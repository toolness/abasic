use crate::Token;

#[derive(Debug, Copy, Clone, PartialEq)]
pub enum TokenType {
    Symbol,
    String,
    Number,
    Operator,
    Comment,
    Keyword,
    Delimiter,
    Data,
}

impl From<&Token> for TokenType {
    fn from(value: &Token) -> Self {
        match value {
            Token::Dim => TokenType::Keyword,
            Token::Let => TokenType::Keyword,
            Token::Print => TokenType::Keyword,
            Token::Input => TokenType::Keyword,
            Token::Goto => TokenType::Keyword,
            Token::Gosub => TokenType::Keyword,
            Token::Return => TokenType::Keyword,
            Token::Colon => TokenType::Delimiter,
            Token::Semicolon => TokenType::Delimiter,
            Token::Comma => TokenType::Delimiter,
            Token::QuestionMark => TokenType::Keyword,
            Token::LeftParen => TokenType::Delimiter,
            Token::RightParen => TokenType::Delimiter,
            Token::Plus => TokenType::Operator,
            Token::Minus => TokenType::Operator,
            Token::Multiply => TokenType::Operator,
            Token::Divide => TokenType::Operator,
            Token::Caret => TokenType::Operator,
            Token::Equals => TokenType::Operator,
            Token::NotEquals => TokenType::Operator,
            Token::LessThan => TokenType::Operator,
            Token::LessThanOrEqualTo => TokenType::Operator,
            Token::GreaterThan => TokenType::Operator,
            Token::GreaterThanOrEqualTo => TokenType::Operator,
            Token::And => TokenType::Operator,
            Token::Or => TokenType::Operator,
            Token::Not => TokenType::Operator,
            Token::If => TokenType::Keyword,
            Token::Then => TokenType::Keyword,
            Token::Else => TokenType::Keyword,
            Token::End => TokenType::Keyword,
            Token::Stop => TokenType::Keyword,
            Token::For => TokenType::Keyword,
            Token::To => TokenType::Keyword,
            Token::Step => TokenType::Keyword,
            Token::Next => TokenType::Keyword,
            Token::Read => TokenType::Keyword,
            Token::Restore => TokenType::Keyword,
            Token::Def => TokenType::Keyword,
            Token::Remark(_) => TokenType::Comment,
            Token::Symbol(_) => TokenType::Symbol,
            Token::StringLiteral(_) => TokenType::String,
            Token::NumericLiteral(_) => TokenType::Number,
            Token::Data(_) => TokenType::Data,
        }
    }
}
