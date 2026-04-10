use super::*;
use std::collections::HashMap;

fn arities() -> HashMap<String, usize> {
    let mut m = HashMap::new();
    m.insert("print".to_string(), 1);
    m.insert("repeat".to_string(), 2);
    m.insert("show".to_string(), 1);
    m.insert("make".to_string(), 2);
    m.insert("setpower".to_string(), 1);
    m
}

#[test]
fn test_incomplete_bracket() {
    assert!(matches!(check_input("repeat 4 [", arities()), ParseOutcome::Incomplete));
}

#[test]
fn test_incomplete_nested_brackets() {
    assert!(matches!(check_input("repeat 4 [repeat 3 [", arities()), ParseOutcome::Incomplete));
}

#[test]
fn test_incomplete_procedure_def() {
    assert!(matches!(check_input("to greet\nprint \"hi", arities()), ParseOutcome::Incomplete));
}

#[test]
fn test_incomplete_paren() {
    assert!(matches!(check_input("(print", arities()), ParseOutcome::Incomplete));
}

#[test]
fn test_incomplete_arity() {
    assert!(matches!(check_input("print", arities()), ParseOutcome::Incomplete));
}

#[test]
fn test_error_unexpected_close_bracket() {
    assert!(matches!(check_input("repeat 4 ]", arities()), ParseOutcome::Error(_)));
}

#[test]
fn test_complete_single_line() {
    assert!(matches!(check_input("repeat 4 [print \"hi]", arities()), ParseOutcome::Complete(_)));
}

#[test]
fn test_complete_multi_line() {
    assert!(matches!(
        check_input("repeat 4 [\nprint \"hi\n]", arities()),
        ParseOutcome::Complete(_)
    ));
}

#[test]
fn test_complete_procedure() {
    assert!(matches!(
        check_input("to greet\nprint \"hi\nend", arities()),
        ParseOutcome::Complete(_)
    ));
}

#[test]
fn test_incomplete_forever() {
    assert!(matches!(check_input("forever [", arities()), ParseOutcome::Incomplete));
}

#[test]
fn test_incomplete_if() {
    assert!(matches!(check_input("if \"true [", arities()), ParseOutcome::Incomplete));
}

#[test]
fn test_incomplete_data_list() {
    assert!(matches!(check_input("make \"x [1 2", arities()), ParseOutcome::Incomplete));
}

#[test]
fn test_complete_empty_input() {
    assert!(matches!(check_input("", arities()), ParseOutcome::Complete(_)));
}

#[test]
fn test_error_unknown_token() {
    assert!(matches!(check_input(")", arities()), ParseOutcome::Error(_)));
}
