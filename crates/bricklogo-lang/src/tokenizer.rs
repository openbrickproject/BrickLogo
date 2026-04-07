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
            tokens.push(Token {
                token_type: TokenType::Infix,
                value: ch.to_string(),
                line,
                col,
            });
            i += 1;
            col += 1;
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
mod tests {
    use super::*;

    #[test]
    fn test_numbers() {
        let tokens = tokenize("42 3.14").unwrap();
        assert_eq!(tokens[0].token_type, TokenType::Number);
        assert_eq!(tokens[0].value, "42");
        assert_eq!(tokens[1].token_type, TokenType::Number);
        assert_eq!(tokens[1].value, "3.14");
    }

    #[test]
    fn test_negative_number() {
        let tokens = tokenize("-7").unwrap();
        assert_eq!(tokens[0].token_type, TokenType::Number);
        assert_eq!(tokens[0].value, "-7");
    }

    #[test]
    fn test_minus_as_infix() {
        let tokens = tokenize("5 - 3").unwrap();
        assert_eq!(tokens[0].token_type, TokenType::Number);
        assert_eq!(tokens[1].token_type, TokenType::Infix);
        assert_eq!(tokens[2].token_type, TokenType::Number);
    }

    #[test]
    fn test_quoted_words() {
        let tokens = tokenize("\"hello").unwrap();
        assert_eq!(tokens[0].token_type, TokenType::QuotedWord);
        assert_eq!(tokens[0].value, "hello");
    }

    #[test]
    fn test_quoted_word_with_spaces() {
        let tokens = tokenize("\"|hello world|").unwrap();
        assert_eq!(tokens[0].token_type, TokenType::QuotedWord);
        assert_eq!(tokens[0].value, "hello world");
    }

    #[test]
    fn test_variable() {
        let tokens = tokenize(":myvar").unwrap();
        assert_eq!(tokens[0].token_type, TokenType::Variable);
        assert_eq!(tokens[0].value, "myvar");
    }

    #[test]
    fn test_lowercase_words() {
        let tokens = tokenize("PRINT").unwrap();
        assert_eq!(tokens[0].token_type, TokenType::Word);
        assert_eq!(tokens[0].value, "print");
    }

    #[test]
    fn test_lowercase_variables() {
        let tokens = tokenize(":MyVar").unwrap();
        assert_eq!(tokens[0].value, "myvar");
    }

    #[test]
    fn test_preserves_quoted_case() {
        let tokens = tokenize("\"Hello").unwrap();
        assert_eq!(tokens[0].value, "Hello");
    }

    #[test]
    fn test_brackets() {
        let tokens = tokenize("[a b]").unwrap();
        assert_eq!(tokens[0].token_type, TokenType::OpenBracket);
        assert_eq!(tokens[1].token_type, TokenType::Word);
        assert_eq!(tokens[2].token_type, TokenType::Word);
        assert_eq!(tokens[3].token_type, TokenType::CloseBracket);
    }

    #[test]
    fn test_parens() {
        let tokens = tokenize("(sum 1 2)").unwrap();
        assert_eq!(tokens[0].token_type, TokenType::OpenParen);
        assert_eq!(tokens[1].token_type, TokenType::Word);
        assert_eq!(tokens[4].token_type, TokenType::CloseParen);
    }

    #[test]
    fn test_infix_operators() {
        let tokens = tokenize("3 + 4 * 2 = 11").unwrap();
        assert_eq!(tokens[1].token_type, TokenType::Infix);
        assert_eq!(tokens[1].value, "+");
        assert_eq!(tokens[3].token_type, TokenType::Infix);
        assert_eq!(tokens[3].value, "*");
        assert_eq!(tokens[5].token_type, TokenType::Infix);
        assert_eq!(tokens[5].value, "=");
    }

    #[test]
    fn test_comments() {
        let tokens = tokenize("print 5 ; this is a comment\nprint 6").unwrap();
        let words: Vec<_> = tokens
            .iter()
            .filter(|t| t.token_type == TokenType::Word)
            .collect();
        assert_eq!(words.len(), 2);
    }

    #[test]
    fn test_empty_input() {
        let tokens = tokenize("").unwrap();
        assert_eq!(tokens.len(), 1);
        assert_eq!(tokens[0].token_type, TokenType::Eof);
    }

    #[test]
    fn test_newlines() {
        let tokens = tokenize("a\nb").unwrap();
        assert_eq!(tokens[1].token_type, TokenType::Newline);
    }

    #[test]
    fn test_empty_variable_name() {
        assert!(tokenize(": ").is_err());
    }

    #[test]
    fn test_quoted_word_with_path() {
        let tokens = tokenize("\"./examples").unwrap();
        assert_eq!(tokens[0].token_type, TokenType::QuotedWord);
        assert_eq!(tokens[0].value, "./examples");
    }

    #[test]
    fn test_dot_in_quoted_word() {
        let tokens = tokenize("\"hub.a").unwrap();
        assert_eq!(tokens[0].token_type, TokenType::QuotedWord);
        assert_eq!(tokens[0].value, "hub.a");
    }
}
