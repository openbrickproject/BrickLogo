use super::*;
use crate::evaluator::Evaluator;
use crate::primitives::register_core_primitives;
use std::sync::Arc;

fn create_evaluator() -> Evaluator {
    let mut eval = Evaluator::new(Arc::new(|_: &str| {}));
    register_core_primitives(&mut eval);
    eval
}

#[test]
fn test_simple_procedure() {
    let mut eval = create_evaluator();
    eval.evaluate("to greet :name print :name end").unwrap();
    let procs = eval.get_all_procedures();
    let source = procedure_to_source(procs[0]);
    assert_eq!(source, "to greet :name\n  print :name\nend");
}

#[test]
fn test_no_params() {
    let mut eval = create_evaluator();
    eval.evaluate("to hello print \"hi end").unwrap();
    let procs = eval.get_all_procedures();
    let source = procedure_to_source(procs[0]);
    assert_eq!(source, "to hello\n  print \"hi\nend");
}

#[test]
fn test_repeat() {
    let mut eval = create_evaluator();
    eval.evaluate("to square :n repeat 4 [print :n] end")
        .unwrap();
    let procs = eval.get_all_procedures();
    let source = procedure_to_source(procs[0]);
    assert!(source.contains("repeat 4 [print :n]"));
}

#[test]
fn test_ifelse() {
    let mut eval = create_evaluator();
    eval.evaluate("to check :x ifelse :x > 0 [print \"pos] [print \"neg] end")
        .unwrap();
    let procs = eval.get_all_procedures();
    let source = procedure_to_source(procs[0]);
    assert!(source.contains("ifelse :x > 0 [print \"pos] [print \"neg]"));
}

#[test]
fn test_output_and_stop() {
    let mut eval = create_evaluator();
    eval.evaluate("to double :n output :n * 2 end").unwrap();
    let procs = eval.get_all_procedures();
    let source = procedure_to_source(procs[0]);
    assert!(source.contains("output :n * 2"));
}

#[test]
fn test_round_trip() {
    let mut eval1 = create_evaluator();
    eval1
        .evaluate("to countdown :n if :n = 0 [print \"done stop] print :n countdown :n - 1 end")
        .unwrap();
    let procs1 = eval1.get_all_procedures();
    let source1 = procedure_to_source(procs1[0]);

    let mut eval2 = create_evaluator();
    eval2.evaluate(&source1).unwrap();
    let procs2 = eval2.get_all_procedures();
    let source2 = procedure_to_source(procs2[0]);

    assert_eq!(source1, source2);
}
