#[derive(Debug, Clone, PartialEq)]
pub enum TokenType {
    Word,
    Number,
    QuotedWord,
    Variable,
    OpenBracket,
    CloseBracket,
    OpenParen,
    CloseParen,
    Infix,
    Newline,
    Eof,
}

#[derive(Debug, Clone)]
pub struct Token {
    pub token_type: TokenType,
    pub value: String,
    pub line: usize,
    pub col: usize,
}
