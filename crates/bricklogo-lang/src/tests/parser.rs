use super::*;
use crate::tokenizer::tokenize;

fn parse_str(input: &str) -> Vec<AstNode> {
    let tokens = tokenize(input).unwrap();
    let mut parser = Parser::new(HashMap::new());
    parser.set_arity("print", 1);
    parser.set_arity("sum", 2);
    parser.set_arity("make", 2);
    parser.set_arity("repeat", 1);
    parser.parse(tokens).unwrap()
}

#[test]
fn test_number_literal() {
    let ast = parse_str("42");
    assert!(matches!(&ast[0], AstNode::Number(n) if *n == 42.0));
}

#[test]
fn test_quoted_word() {
    let ast = parse_str("\"hello");
    assert!(matches!(&ast[0], AstNode::Word(s) if s == "hello"));
}

#[test]
fn test_variable() {
    let ast = parse_str(":x");
    assert!(matches!(&ast[0], AstNode::Variable(s) if s == "x"));
}

#[test]
fn test_infix() {
    let ast = parse_str("3 + 4");
    assert!(matches!(&ast[0], AstNode::Infix { operator, .. } if operator == "+"));
}

#[test]
fn test_procedure_call() {
    let ast = parse_str("print \"hello");
    assert!(
        matches!(&ast[0], AstNode::Call { name, args, .. } if name == "print" && args.len() == 1)
    );
}

#[test]
fn test_data_list() {
    let ast = parse_str("[a b c]");
    if let AstNode::List(elements) = &ast[0] {
        assert_eq!(elements.len(), 3);
        assert!(matches!(&elements[0], AstNode::Word(s) if s == "a"));
    } else {
        panic!("Expected list");
    }
}

#[test]
fn test_procedure_def() {
    let ast = parse_str("to greet :name print :name end");
    assert!(
        matches!(&ast[0], AstNode::ProcDef { name, params, body } if name == "greet" && params.len() == 1 && body.len() == 1)
    );
}

#[test]
fn test_repeat() {
    let tokens = tokenize("repeat 4 [print \"hi]").unwrap();
    let mut parser = Parser::new(HashMap::new());
    parser.set_arity("print", 1);
    let ast = parser.parse(tokens).unwrap();
    assert!(matches!(&ast[0], AstNode::Repeat { body, .. } if body.len() == 1));
}

#[test]
fn test_ifelse() {
    let tokens = tokenize("ifelse 1 = 1 [\"yes] [\"no]").unwrap();
    let mut parser = Parser::new(HashMap::new());
    let ast = parser.parse(tokens).unwrap();
    assert!(matches!(&ast[0], AstNode::IfElse { .. }));
}

#[test]
fn test_carefully() {
    let tokens = tokenize("carefully [print \"ok] [print \"err]").unwrap();
    let mut parser = Parser::new(HashMap::new());
    parser.set_arity("print", 1);
    let ast = parser.parse(tokens).unwrap();
    assert!(matches!(&ast[0], AstNode::Carefully { .. }));
}

#[test]
fn test_missing_end() {
    let tokens = tokenize("to test print \"hi").unwrap();
    let mut parser = Parser::new(HashMap::new());
    parser.set_arity("print", 1);
    assert!(parser.parse(tokens).is_err());
}

#[test]
fn test_recursive_procedure() {
    let tokens =
        tokenize("to countdown :n if :n = 0 [stop] print :n countdown :n - 1 end").unwrap();
    let mut parser = Parser::new(HashMap::new());
    parser.set_arity("print", 1);
    let ast = parser.parse(tokens).unwrap();
    assert!(
        matches!(&ast[0], AstNode::ProcDef { name, params, .. } if name == "countdown" && params.len() == 1)
    );
}
