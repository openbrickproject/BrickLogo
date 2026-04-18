use crate::error::{LogoError, LogoResult};
use crate::token::{Token, TokenType};

const INFIX_OPS: &[char] = &['+', '-', '*', '/', '=', '<', '>'];

fn is_infix(ch: char) -> bool {
    INFIX_OPS.contains(&ch)
}

fn is_delimiter(ch: char) -> bool {
    matches!(ch, ' ' | '\t' | '\n' | '\r' | '[' | ']' | '(' | ')' | ';') || is_infix(ch)
}

fn is_quoted_word_delimiter(ch: char) -> bool {
    matches!(ch, ' ' | '\t' | '\n' | '\r' | '[' | ']' | '(' | ')' | ';')
}

fn is_digit(ch: char) -> bool {
    ch.is_ascii_digit()
}

fn is_word_char(ch: char) -> bool {
    !is_delimiter(ch) && ch != '"' && ch != ':'
}

pub fn tokenize(source: &str) -> LogoResult<Vec<Token>> {
    let mut tokens = Vec::new();
    let chars: Vec<char> = source.chars().collect();
    let mut i = 0;
    let mut line = 1;
    let mut col = 1;

    while i < chars.len() {
        let ch = chars[i];

        // Skip comments
        if ch == ';' {
            while i < chars.len() && chars[i] != '\n' {
                i += 1;
            }
            continue;
        }

        // Whitespace (not newline)
        if ch == ' ' || ch == '\t' || ch == '\r' {
            i += 1;
            col += 1;
            continue;
        }

        // Newline
        if ch == '\n' {
            tokens.push(Token {
                token_type: TokenType::Newline,
                value: "\n".to_string(),
                line,
                col,
            });
            i += 1;
            line += 1;
            col = 1;
            continue;
        }

        // Brackets
        if ch == '[' {
            tokens.push(Token {
                token_type: TokenType::OpenBracket,
                value: "[".to_string(),
                line,
                col,
            });
            i += 1;
            col += 1;
            continue;
        }
        if ch == ']' {
            tokens.push(Token {
                token_type: TokenType::CloseBracket,
                value: "]".to_string(),
                line,
                col,
            });
            i += 1;
            col += 1;
            continue;
        }

        // Parens
        if ch == '(' {
            tokens.push(Token {
                token_type: TokenType::OpenParen,
                value: "(".to_string(),
                line,
                col,
            });
            i += 1;
            col += 1;
            continue;
        }
        if ch == ')' {
            tokens.push(Token {
                token_type: TokenType::CloseParen,
                value: ")".to_string(),
                line,
                col,
            });
            i += 1;
            col += 1;
            continue;
        }

        // Infix operators (but not negative numbers)
        if is_infix(ch) {
            if ch == '-' && i + 1 < chars.len() && is_digit(chars[i + 1]) {
                let prev = tokens.last().map(|t| &t.token_type);
                if prev.is_none()
                    || matches!(
                        prev,
                        Some(
                            TokenType::OpenBracket
                                | TokenType::OpenParen
                                | TokenType::Infix
                                | TokenType::Newline
                        )
                    )
                {
                    // Parse as negative number
                    let start = i;
                    let start_col = col;
                    i += 1;
                    col += 1;
                    while i < chars.len() && (is_digit(chars[i]) || chars[i] == '.') {
                        i += 1;
                        col += 1;
                    }
                    tokens.push(Token {
                        token_type: TokenType::Number,
                        value: chars[start..i].iter().collect(),
                        line,
                        col: start_col,
                    });
                    continue;
                }
            }
            // Two-character operators: >=, <=, <>
            let two_char = if i + 1 < chars.len() {
                match (ch, chars[i + 1]) {
                    ('>', '=') | ('<', '=') | ('<', '>') => {
                        let op: String = chars[i..i + 2].iter().collect();
                        Some(op)
                    }
                    _ => None,
                }
            } else {
                None
            };
            if let Some(op) = two_char {
                tokens.push(Token {
                    token_type: TokenType::Infix,
                    value: op,
                    line,
                    col,
                });
                i += 2;
                col += 2;
            } else {
                tokens.push(Token {
                    token_type: TokenType::Infix,
                    value: ch.to_string(),
                    line,
                    col,
                });
                i += 1;
                col += 1;
            }
            continue;
        }

        // Numbers
        if is_digit(ch) || (ch == '.' && i + 1 < chars.len() && is_digit(chars[i + 1])) {
            let start = i;
            let start_col = col;
            while i < chars.len() && (is_digit(chars[i]) || chars[i] == '.') {
                i += 1;
                col += 1;
            }
            tokens.push(Token {
                token_type: TokenType::Number,
                value: chars[start..i].iter().collect(),
                line,
                col: start_col,
            });
            continue;
        }

        // Quoted word: "hello or "|hello world|
        if ch == '"' {
            let start_col = col;
            i += 1;
            col += 1;
            if i < chars.len() && chars[i] == '|' {
                // "|...|
                i += 1;
                col += 1;
                let start = i;
                while i < chars.len() && chars[i] != '|' {
                    if chars[i] == '\n' {
                        line += 1;
                        col = 0;
                    }
                    i += 1;
                    col += 1;
                }
                if i >= chars.len() {
                    return Err(LogoError::Syntax {
                        message: "Missing closing '|'".to_string(),
                        line,
                        col,
                    });
                }
                tokens.push(Token {
                    token_type: TokenType::QuotedWord,
                    value: chars[start..i].iter().collect(),
                    line,
                    col: start_col,
                });
                i += 1;
                col += 1; // skip closing |
            } else {
                let start = i;
                while i < chars.len() && !is_quoted_word_delimiter(chars[i]) {
                    i += 1;
                    col += 1;
                }
                tokens.push(Token {
                    token_type: TokenType::QuotedWord,
                    value: chars[start..i].iter().collect(),
                    line,
                    col: start_col,
                });
            }
            continue;
        }

        // Variable reference: :name
        if ch == ':' {
            let start_col = col;
            i += 1;
            col += 1;
            let start = i;
            while i < chars.len() && !is_delimiter(chars[i]) {
                i += 1;
                col += 1;
            }
            if i == start {
                return Err(LogoError::Syntax {
                    message: "Expected variable name after ':'".to_string(),
                    line,
                    col,
                });
            }
            let name: String = chars[start..i].iter().collect();
            tokens.push(Token {
                token_type: TokenType::Variable,
                value: name.to_lowercase(),
                line,
                col: start_col,
            });
            continue;
        }

        // Word (bareword)
        if is_word_char(ch) {
            let start = i;
            let start_col = col;
            while i < chars.len() && is_word_char(chars[i]) {
                i += 1;
                col += 1;
            }
            let word: String = chars[start..i].iter().collect();
            tokens.push(Token {
                token_type: TokenType::Word,
                value: word.to_lowercase(),
                line,
                col: start_col,
            });
            continue;
        }

        return Err(LogoError::Syntax {
            message: format!("Unexpected character '{}'", ch),
            line,
            col,
        });
    }

    tokens.push(Token {
        token_type: TokenType::Eof,
        value: String::new(),
        line,
        col,
    });
    Ok(tokens)
}

#[cfg(test)]
#[path = "tests/tokenizer.rs"]
mod tests;
