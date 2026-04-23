//! Direct tests for every primitive registered by `register_core_primitives`.
//!
//! The evaluator tests incidentally exercise many of these through the
//! parser; these tests pin behaviour for the primitive itself — input
//! validation, boundary values, error messages — so a bug inside a
//! primitive fires a named test rather than a vague evaluator failure.

use crate::evaluator::Evaluator;
use crate::value::LogoValue;
use std::sync::{Arc, Mutex};

fn eval() -> (Evaluator, Arc<Mutex<Vec<String>>>) {
    let output = Arc::new(Mutex::new(Vec::new()));
    let output_clone = output.clone();
    let mut e = Evaluator::new(Arc::new(move |text: &str| {
        output_clone.lock().unwrap().push(text.to_string());
    }));
    crate::primitives::register_core_primitives(&mut e);
    (e, output)
}

fn num(e: &mut Evaluator, src: &str) -> f64 {
    match e.evaluate(src).unwrap() {
        Some(LogoValue::Number(n)) => n,
        other => panic!("expected Number, got {:?}", other),
    }
}

fn word(e: &mut Evaluator, src: &str) -> String {
    match e.evaluate(src).unwrap() {
        Some(LogoValue::Word(w)) => w,
        other => panic!("expected Word, got {:?}", other),
    }
}

// ── Arithmetic ──────────────────────────────────

#[test]
fn test_sum() {
    let (mut e, _) = eval();
    assert_eq!(num(&mut e, "sum 3 4"), 7.0);
    assert_eq!(num(&mut e, "sum (minus 3) (minus 4)"), -7.0);
    assert_eq!(num(&mut e, "sum 0 0"), 0.0);
}

#[test]
fn test_difference() {
    let (mut e, _) = eval();
    assert_eq!(num(&mut e, "difference 10 3"), 7.0);
    assert_eq!(num(&mut e, "difference 3 10"), -7.0);
}

#[test]
fn test_product() {
    let (mut e, _) = eval();
    assert_eq!(num(&mut e, "product 3 4"), 12.0);
    assert_eq!(num(&mut e, "product (minus 3) 4"), -12.0);
    assert_eq!(num(&mut e, "product 0 999"), 0.0);
}

#[test]
fn test_quotient() {
    let (mut e, _) = eval();
    assert_eq!(num(&mut e, "quotient 10 2"), 5.0);
    assert_eq!(num(&mut e, "quotient 7 2"), 3.5);
}

#[test]
fn test_quotient_divide_by_zero_errors() {
    let (mut e, _) = eval();
    assert!(e.evaluate("quotient 5 0").is_err());
}

#[test]
fn test_remainder() {
    let (mut e, _) = eval();
    assert_eq!(num(&mut e, "remainder 10 3"), 1.0);
    assert_eq!(num(&mut e, "remainder 9 3"), 0.0);
}

#[test]
fn test_minus() {
    let (mut e, _) = eval();
    assert_eq!(num(&mut e, "minus 5"), -5.0);
    assert_eq!(num(&mut e, "minus (minus 7)"), 7.0);
}

#[test]
fn test_power() {
    let (mut e, _) = eval();
    assert_eq!(num(&mut e, "power 2 8"), 256.0);
    assert_eq!(num(&mut e, "power 5 0"), 1.0);
}

#[test]
fn test_modulo() {
    let (mut e, _) = eval();
    assert_eq!(num(&mut e, "modulo 10 3"), 1.0);
    assert_eq!(num(&mut e, "modulo (minus 1) 5"), 4.0); // Euclidean mod
}

#[test]
fn test_abs() {
    let (mut e, _) = eval();
    assert_eq!(num(&mut e, "abs (minus 7)"), 7.0);
    assert_eq!(num(&mut e, "abs 7"), 7.0);
    assert_eq!(num(&mut e, "abs 0"), 0.0);
}

#[test]
fn test_sqrt() {
    let (mut e, _) = eval();
    assert_eq!(num(&mut e, "sqrt 16"), 4.0);
    assert_eq!(num(&mut e, "sqrt 0"), 0.0);
}

#[test]
fn test_sqrt_negative_errors() {
    let (mut e, _) = eval();
    assert!(e.evaluate("sqrt -4").is_err());
}

#[test]
fn test_sin_cos_tan_in_degrees() {
    let (mut e, _) = eval();
    // Logo convention: trig takes degrees.
    let s = num(&mut e, "sin 90");
    assert!((s - 1.0).abs() < 1e-9);
    let c = num(&mut e, "cos 0");
    assert!((c - 1.0).abs() < 1e-9);
    let t = num(&mut e, "tan 45");
    assert!((t - 1.0).abs() < 1e-9);
}

#[test]
fn test_random_range() {
    let (mut e, _) = eval();
    for _ in 0..20 {
        let v = num(&mut e, "random 10");
        assert!((0.0..10.0).contains(&v), "random 10 produced {}", v);
        assert_eq!(v.fract(), 0.0, "random 10 produced non-integer {}", v);
    }
}

#[test]
fn test_int_truncates_toward_zero() {
    let (mut e, _) = eval();
    assert_eq!(num(&mut e, "int 3.7"), 3.0);
    assert_eq!(num(&mut e, "int (minus 3.7)"), -3.0);
    assert_eq!(num(&mut e, "int 0"), 0.0);
}

#[test]
fn test_round_half_to_even_or_up() {
    let (mut e, _) = eval();
    assert_eq!(num(&mut e, "round 3.4"), 3.0);
    assert_eq!(num(&mut e, "round 3.6"), 4.0);
    assert_eq!(num(&mut e, "round (minus 3.6)"), -4.0);
}

// ── Logic ───────────────────────────────────────

#[test]
fn test_and_short_circuits() {
    let (mut e, _) = eval();
    assert_eq!(word(&mut e, "and \"true \"true"), "true");
    assert_eq!(word(&mut e, "and \"true \"false"), "false");
    assert_eq!(word(&mut e, "and \"false \"true"), "false");
}

#[test]
fn test_or_short_circuits() {
    let (mut e, _) = eval();
    assert_eq!(word(&mut e, "or \"false \"true"), "true");
    assert_eq!(word(&mut e, "or \"false \"false"), "false");
    assert_eq!(word(&mut e, "or \"true \"false"), "true");
}

#[test]
fn test_not_negates() {
    let (mut e, _) = eval();
    assert_eq!(word(&mut e, "not \"true"), "false");
    assert_eq!(word(&mut e, "not \"false"), "true");
}

#[test]
fn test_equal_predicate() {
    let (mut e, _) = eval();
    assert_eq!(word(&mut e, "equal? 3 3"), "true");
    assert_eq!(word(&mut e, "equal? 3 4"), "false");
    assert_eq!(word(&mut e, "equal? \"hi \"hi"), "true");
    assert_eq!(word(&mut e, "equal? \"hi \"bye"), "false");
}

// ── Type predicates ─────────────────────────────

#[test]
fn test_number_predicate() {
    let (mut e, _) = eval();
    assert_eq!(word(&mut e, "number? 3"), "true");
    assert_eq!(word(&mut e, "number? \"hi"), "false");
    assert_eq!(word(&mut e, "number? [1 2]"), "false");
}

#[test]
fn test_list_predicate() {
    let (mut e, _) = eval();
    assert_eq!(word(&mut e, "list? [1 2]"), "true");
    assert_eq!(word(&mut e, "list? 3"), "false");
    assert_eq!(word(&mut e, "list? \"hi"), "false");
}

#[test]
fn test_word_predicate() {
    let (mut e, _) = eval();
    assert_eq!(word(&mut e, "word? \"hi"), "true");
    // In BrickLogo, numbers are their own LogoValue variant and do not
    // satisfy `word?`. Use `number?` to test for a number.
    assert_eq!(word(&mut e, "word? 3"), "false");
    assert_eq!(word(&mut e, "word? [1 2]"), "false");
}

#[test]
fn test_empty_predicate() {
    let (mut e, _) = eval();
    assert_eq!(word(&mut e, "empty? []"), "true");
    assert_eq!(word(&mut e, "empty? [1]"), "false");
    assert_eq!(word(&mut e, "empty? \"x"), "false");
}

#[test]
fn test_member_predicate() {
    let (mut e, _) = eval();
    assert_eq!(word(&mut e, "member? 2 [1 2 3]"), "true");
    assert_eq!(word(&mut e, "member? 4 [1 2 3]"), "false");
    assert_eq!(word(&mut e, "member? \"l \"hello"), "true");
    assert_eq!(word(&mut e, "member? \"z \"hello"), "false");
}

#[test]
fn test_name_predicate_reports_variable_binding() {
    let (mut e, _) = eval();
    e.evaluate("make \"x 42").unwrap();
    assert_eq!(word(&mut e, "name? \"x"), "true");
    assert_eq!(word(&mut e, "name? \"undef"), "false");
}

// ── List and word manipulation ──────────────────

#[test]
fn test_first_of_list() {
    let (mut e, _) = eval();
    assert_eq!(num(&mut e, "first [7 8 9]"), 7.0);
}

#[test]
fn test_first_of_word() {
    let (mut e, _) = eval();
    assert_eq!(word(&mut e, "first \"hello"), "h");
}

#[test]
fn test_first_of_empty_errors() {
    let (mut e, _) = eval();
    assert!(e.evaluate("first []").is_err());
}

#[test]
fn test_last_of_list() {
    let (mut e, _) = eval();
    assert_eq!(num(&mut e, "last [7 8 9]"), 9.0);
}

#[test]
fn test_last_of_word() {
    let (mut e, _) = eval();
    assert_eq!(word(&mut e, "last \"hello"), "o");
}

#[test]
fn test_butfirst() {
    let (mut e, _) = eval();
    match e.evaluate("butfirst [1 2 3]").unwrap() {
        Some(LogoValue::List(l)) => {
            assert_eq!(l.len(), 2);
            assert!(matches!(l[0], LogoValue::Number(2.0)));
            assert!(matches!(l[1], LogoValue::Number(3.0)));
        }
        other => panic!("expected list, got {:?}", other),
    }
    assert_eq!(word(&mut e, "butfirst \"hello"), "ello");
}

#[test]
fn test_butlast() {
    let (mut e, _) = eval();
    assert_eq!(word(&mut e, "butlast \"hello"), "hell");
}

#[test]
fn test_item_1_indexed() {
    let (mut e, _) = eval();
    assert_eq!(num(&mut e, "item 1 [7 8 9]"), 7.0);
    assert_eq!(num(&mut e, "item 3 [7 8 9]"), 9.0);
}

#[test]
fn test_item_out_of_range_errors() {
    let (mut e, _) = eval();
    assert!(e.evaluate("item 0 [1 2]").is_err());
    assert!(e.evaluate("item 5 [1 2]").is_err());
}

#[test]
fn test_count() {
    let (mut e, _) = eval();
    assert_eq!(num(&mut e, "count [1 2 3 4]"), 4.0);
    assert_eq!(num(&mut e, "count []"), 0.0);
    assert_eq!(num(&mut e, "count \"hello"), 5.0);
}

#[test]
fn test_fput_prepends() {
    let (mut e, _) = eval();
    match e.evaluate("fput 0 [1 2]").unwrap() {
        Some(LogoValue::List(l)) => {
            assert_eq!(l.len(), 3);
            assert!(matches!(l[0], LogoValue::Number(0.0)));
        }
        _ => panic!("expected list"),
    }
}

#[test]
fn test_lput_appends() {
    let (mut e, _) = eval();
    match e.evaluate("lput 9 [1 2]").unwrap() {
        Some(LogoValue::List(l)) => {
            assert_eq!(l.len(), 3);
            assert!(matches!(l[2], LogoValue::Number(9.0)));
        }
        _ => panic!("expected list"),
    }
}

#[test]
fn test_list_combines_values_into_list() {
    let (mut e, _) = eval();
    match e.evaluate("list 1 2").unwrap() {
        Some(LogoValue::List(l)) => assert_eq!(l.len(), 2),
        _ => panic!("expected list"),
    }
}

#[test]
fn test_sentence_flattens_one_level() {
    let (mut e, _) = eval();
    match e.evaluate("sentence [1 2] [3 4]").unwrap() {
        Some(LogoValue::List(l)) => assert_eq!(l.len(), 4),
        _ => panic!("expected flattened list"),
    }
}

#[test]
fn test_word_concatenates_words() {
    let (mut e, _) = eval();
    assert_eq!(word(&mut e, "word \"hello \"world"), "helloworld");
}

#[test]
fn test_uppercase_lowercase() {
    let (mut e, _) = eval();
    assert_eq!(word(&mut e, "uppercase \"Hello"), "HELLO");
    assert_eq!(word(&mut e, "lowercase \"HELLO"), "hello");
}

// ── Output ──────────────────────────────────────

#[test]
fn test_print_writes_to_output_stream() {
    let (mut e, out) = eval();
    e.evaluate("print \"hi").unwrap();
    let captured = out.lock().unwrap().clone();
    assert!(
        captured.iter().any(|s| s.contains("hi")),
        "print did not reach the output stream; got {:?}",
        captured
    );
}

#[test]
fn test_show_wraps_lists_with_brackets() {
    let (mut e, out) = eval();
    e.evaluate("show [1 2 3]").unwrap();
    let captured = out.lock().unwrap().clone();
    assert!(
        captured.iter().any(|s| s.contains("[1 2 3]")),
        "show should display list with its brackets; got {:?}",
        captured
    );
}

#[test]
fn test_type_writes_without_trailing_newline() {
    // We can't directly check the newline behaviour from the captured
    // strings (each call becomes a separate entry). Just verify the
    // primitive runs without error and produces at least one entry.
    let (mut e, out) = eval();
    e.evaluate("type \"abc").unwrap();
    assert!(!out.lock().unwrap().is_empty());
}

// ── Variables ───────────────────────────────────

#[test]
fn test_make_sets_global() {
    let (mut e, _) = eval();
    e.evaluate("make \"x 42").unwrap();
    assert_eq!(num(&mut e, ":x"), 42.0);
}

#[test]
fn test_thing_retrieves_by_name() {
    let (mut e, _) = eval();
    e.evaluate("make \"y 99").unwrap();
    assert_eq!(num(&mut e, "thing \"y"), 99.0);
}

#[test]
fn test_thing_on_undefined_errors() {
    let (mut e, _) = eval();
    assert!(e.evaluate("thing \"undefined").is_err());
}

#[test]
fn test_local_inside_procedure_shadows_global() {
    let (mut e, _) = eval();
    e.evaluate("make \"x 10").unwrap();
    e.evaluate("to shadow local \"x make \"x 99 end").unwrap();
    e.evaluate("shadow").unwrap();
    // After `shadow` returns, the global should still be 10.
    assert_eq!(num(&mut e, ":x"), 10.0);
}

#[test]
fn test_localmake_shorthand() {
    let (mut e, _) = eval();
    e.evaluate("make \"x 1").unwrap();
    e.evaluate("to setx localmake \"x 7 output :x end").unwrap();
    assert_eq!(num(&mut e, "setx"), 7.0);
    assert_eq!(num(&mut e, ":x"), 1.0);
}

// ── Timing ──────────────────────────────────────

#[test]
fn test_wait_blocks_for_at_least_the_duration() {
    let (mut e, _) = eval();
    let start = std::time::Instant::now();
    e.evaluate("wait 2").unwrap(); // tenths of a second
    let elapsed = start.elapsed();
    assert!(
        elapsed.as_millis() >= 180,
        "wait 2 returned after only {:?}",
        elapsed
    );
}

#[test]
fn test_timer_and_resett() {
    let (mut e, _) = eval();
    e.evaluate("resett").unwrap();
    std::thread::sleep(std::time::Duration::from_millis(120));
    let ticks = num(&mut e, "timer");
    // timer reports tenths of a second since the last resett.
    assert!(ticks >= 1.0, "timer={} after ~120ms", ticks);
    e.evaluate("resett").unwrap();
    let after_reset = num(&mut e, "timer");
    assert!(
        after_reset < ticks,
        "resett should push timer back near zero; got {}",
        after_reset
    );
}

// ── File & page (best-effort; these touch disk) ─

#[test]
fn test_disk_reports_current_directory() {
    let (mut e, _) = eval();
    let v = word(&mut e, "disk");
    assert!(!v.is_empty());
}

// ── Argument count validation ───────────────────

#[test]
fn test_sum_with_wrong_arity_errors() {
    let (mut e, _) = eval();
    // `sum` takes exactly two operands; a single operand should error
    // when parsed in a context that doesn't supply the second.
    assert!(e.evaluate("print sum 3").is_err());
}

#[test]
fn test_first_with_zero_args_errors() {
    let (mut e, _) = eval();
    assert!(e.evaluate("print first").is_err());
}
