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
