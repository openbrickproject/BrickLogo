use std::collections::HashMap;
use crate::ast::AstNode;
use crate::error::LogoError;
use crate::parser::Parser;
use crate::tokenizer::tokenize;

pub enum ParseOutcome {
    Complete(Vec<AstNode>),
    Incomplete,
    Error(LogoError),
}

pub fn check_input(source: &str, arities: HashMap<String, usize>) -> ParseOutcome {
    let tokens = match tokenize(source) {
        Ok(t) => t,
        Err(e) => return ParseOutcome::Error(e),
    };
    let mut parser = Parser::new(arities);
    match parser.parse(tokens) {
        Ok(ast) => ParseOutcome::Complete(ast),
        Err(LogoError::Incomplete { .. }) => ParseOutcome::Incomplete,
        Err(e) => ParseOutcome::Error(e),
    }
}

#[cfg(test)]
#[path = "tests/check.rs"]
mod tests;
