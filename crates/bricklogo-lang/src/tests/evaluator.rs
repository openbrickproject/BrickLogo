use super::*;
use std::sync::{Mutex, mpsc};
use std::time::{SystemTime, UNIX_EPOCH};

fn create_evaluator() -> (Evaluator, Arc<Mutex<Vec<String>>>) {
    let output = Arc::new(Mutex::new(Vec::new()));
    let output_clone = output.clone();
    let mut eval = Evaluator::new(Arc::new(move |text: &str| {
        output_clone.lock().unwrap().push(text.to_string());
    }));
    crate::primitives::register_core_primitives(&mut eval);
    (eval, output)
}

#[test]
fn test_number() {
    let (mut eval, _) = create_evaluator();
    assert_eq!(
        eval.evaluate("sum 3 4").unwrap(),
        Some(LogoValue::Number(7.0))
    );
}

#[test]
fn test_infix() {
    let (mut eval, _) = create_evaluator();
    assert_eq!(
        eval.evaluate("3 + 4").unwrap(),
        Some(LogoValue::Number(7.0))
    );
    assert_eq!(
        eval.evaluate("10 - 3").unwrap(),
        Some(LogoValue::Number(7.0))
    );
    assert_eq!(
        eval.evaluate("3 * 4").unwrap(),
        Some(LogoValue::Number(12.0))
    );
    assert_eq!(
        eval.evaluate("10 / 2").unwrap(),
        Some(LogoValue::Number(5.0))
    );
}

#[test]
fn test_comparison() {
    let (mut eval, _) = create_evaluator();
    assert_eq!(
        eval.evaluate("3 = 3").unwrap(),
        Some(LogoValue::Word("true".to_string()))
    );
    assert_eq!(
        eval.evaluate("3 = 4").unwrap(),
        Some(LogoValue::Word("false".to_string()))
    );
    assert_eq!(
        eval.evaluate("3 < 4").unwrap(),
        Some(LogoValue::Word("true".to_string()))
    );
    assert_eq!(
        eval.evaluate("4 > 3").unwrap(),
        Some(LogoValue::Word("true".to_string()))
    );
}

#[test]
fn test_print() {
    let (mut eval, output) = create_evaluator();
    eval.evaluate("print \"hello").unwrap();
    assert_eq!(output.lock().unwrap().as_slice(), &["hello"]);
}

#[test]
fn test_variables() {
    let (mut eval, output) = create_evaluator();
    eval.evaluate("make \"x 42").unwrap();
    eval.evaluate("print :x").unwrap();
    assert_eq!(output.lock().unwrap().as_slice(), &["42"]);
}

#[test]
fn test_make_does_not_broadcast_unchanged_global() {
    let (mut eval, _) = create_evaluator();
    let (tx, rx) = mpsc::channel();
    eval.set_var_broadcast(tx);

    eval.evaluate("make \"x 42").unwrap();
    assert_eq!(
        rx.recv_timeout(std::time::Duration::from_millis(100)).unwrap(),
        ("x".to_string(), LogoValue::Number(42.0))
    );

    eval.evaluate("make \"x 42").unwrap();
    assert!(rx.recv_timeout(std::time::Duration::from_millis(100)).is_err());
}

#[test]
fn test_undefined_variable() {
    let (mut eval, _) = create_evaluator();
    assert!(eval.evaluate("print :nope").is_err());
}

#[test]
fn test_logic() {
    let (mut eval, _) = create_evaluator();
    assert_eq!(
        eval.evaluate("and \"true \"true").unwrap(),
        Some(LogoValue::Word("true".to_string()))
    );
    assert_eq!(
        eval.evaluate("and \"true \"false").unwrap(),
        Some(LogoValue::Word("false".to_string()))
    );
    assert_eq!(
        eval.evaluate("or \"false \"true").unwrap(),
        Some(LogoValue::Word("true".to_string()))
    );
    assert_eq!(
        eval.evaluate("not \"true").unwrap(),
        Some(LogoValue::Word("false".to_string()))
    );
}

#[test]
fn test_list_operations() {
    let (mut eval, _) = create_evaluator();
    assert_eq!(
        eval.evaluate("first [a b c]").unwrap(),
        Some(LogoValue::Word("a".to_string()))
    );
    assert_eq!(
        eval.evaluate("last [a b c]").unwrap(),
        Some(LogoValue::Word("c".to_string()))
    );
    assert_eq!(
        eval.evaluate("count [a b c]").unwrap(),
        Some(LogoValue::Number(3.0))
    );
}

#[test]
fn test_repeat() {
    let (mut eval, output) = create_evaluator();
    eval.evaluate("repeat 3 [print \"hi]").unwrap();
    assert_eq!(output.lock().unwrap().len(), 3);
}

#[test]
fn test_if() {
    let (mut eval, output) = create_evaluator();
    eval.evaluate("if 3 > 2 [print \"yes]").unwrap();
    eval.evaluate("if 3 < 2 [print \"no]").unwrap();
    assert_eq!(output.lock().unwrap().as_slice(), &["yes"]);
}

#[test]
fn test_ifelse() {
    let (mut eval, output) = create_evaluator();
    eval.evaluate("ifelse 1 = 1 [print \"same] [print \"diff]")
        .unwrap();
    eval.evaluate("ifelse 1 = 2 [print \"same] [print \"diff]")
        .unwrap();
    assert_eq!(output.lock().unwrap().as_slice(), &["same", "diff"]);
}

#[test]
fn test_ifelse_as_reporter() {
    let (mut eval, _) = create_evaluator();
    assert_eq!(
        eval.evaluate("ifelse 1 = 1 [\"yes] [\"no]").unwrap(),
        Some(LogoValue::Word("yes".to_string()))
    );
    assert_eq!(
        eval.evaluate("ifelse 1 = 2 [\"yes] [\"no]").unwrap(),
        Some(LogoValue::Word("no".to_string()))
    );
}

#[test]
fn test_procedure() {
    let (mut eval, output) = create_evaluator();
    eval.evaluate("to greet :name print word \"Hello :name end")
        .unwrap();
    eval.evaluate("greet \"World").unwrap();
    assert_eq!(output.lock().unwrap().as_slice(), &["HelloWorld"]);
}

#[test]
fn test_recursion() {
    let (mut eval, output) = create_evaluator();
    eval.evaluate("to countdown :n if :n = 0 [print \"done stop] print :n countdown :n - 1 end")
        .unwrap();
    eval.evaluate("countdown 3").unwrap();
    assert_eq!(output.lock().unwrap().as_slice(), &["3", "2", "1", "done"]);
}

#[test]
fn test_output() {
    let (mut eval, _) = create_evaluator();
    eval.evaluate("to double :n output :n * 2 end").unwrap();
    assert_eq!(
        eval.evaluate("double 5").unwrap(),
        Some(LogoValue::Number(10.0))
    );
}

#[test]
fn test_didnt_output() {
    let (mut eval, _) = create_evaluator();
    assert!(eval.evaluate("print print 5").is_err());
}

#[test]
fn test_unknown_procedure() {
    let (mut eval, _) = create_evaluator();
    assert!(eval.evaluate("blorp").is_err());
}

#[test]
fn test_carefully_catches() {
    let (mut eval, output) = create_evaluator();
    eval.evaluate("carefully [print blorp] [print \"caught]")
        .unwrap();
    assert_eq!(output.lock().unwrap().as_slice(), &["caught"]);
}

#[test]
fn test_carefully_no_error() {
    let (mut eval, output) = create_evaluator();
    eval.evaluate("carefully [print \"ok] [print \"error]")
        .unwrap();
    assert_eq!(output.lock().unwrap().as_slice(), &["ok"]);
}

#[test]
fn test_carefully_as_reporter() {
    let (mut eval, _) = create_evaluator();
    assert_eq!(
        eval.evaluate("carefully [sum 1 2] [0]").unwrap(),
        Some(LogoValue::Number(3.0))
    );
    assert_eq!(
        eval.evaluate("carefully [blorp] [42]").unwrap(),
        Some(LogoValue::Number(42.0))
    );
}

#[test]
fn test_data_list() {
    let (mut eval, output) = create_evaluator();
    eval.evaluate("show [a b c]").unwrap();
    assert_eq!(output.lock().unwrap().as_slice(), &["[a b c]"]);
}

#[test]
fn test_division_by_zero() {
    let (mut eval, _) = create_evaluator();
    assert!(eval.evaluate("quotient 5 0").is_err());
}

#[test]
fn test_stop_in_procedure() {
    let (mut eval, output) = create_evaluator();
    eval.evaluate("to test print \"before stop print \"after end")
        .unwrap();
    eval.evaluate("test").unwrap();
    assert_eq!(output.lock().unwrap().as_slice(), &["before"]);
}

#[test]
fn test_request_stop() {
    let (mut eval, _) = create_evaluator();
    let stop = eval.stop_flag();
    eval.register_primitive(
        "tick",
        PrimitiveSpec {
            min_args: 0,
            max_args: 0,
            func: Arc::new(move |_, _, _| {
                stop.store(true, Ordering::SeqCst);
                Ok(None)
            }),
        },
    );
    assert!(eval.evaluate("forever [tick]").is_err());
}

#[test]
fn test_page_commands_round_trip() {
    let (mut eval, _) = create_evaluator();
    let unique = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let temp_dir = std::env::temp_dir().join(format!("bricklogo-lang-test-{}", unique));
    std::fs::create_dir_all(&temp_dir).unwrap();

    eval.evaluate(&format!("setdisk \"{}", temp_dir.display()))
        .unwrap();
    eval.evaluate("to greet print \"hi end").unwrap();
    eval.evaluate("namepage \"demo").unwrap();
    eval.evaluate("save").unwrap();

    let saved_path = temp_dir.join("demo.logo");
    assert!(saved_path.exists());

    let (mut loaded_eval, output) = create_evaluator();
    loaded_eval
        .evaluate(&format!("setdisk \"{}", temp_dir.display()))
        .unwrap();
    loaded_eval.evaluate("load \"demo").unwrap();
    loaded_eval.evaluate("greet").unwrap();
    assert_eq!(output.lock().unwrap().as_slice(), &["hi"]);

    std::fs::remove_dir_all(&temp_dir).unwrap();
}

#[test]
fn test_disk_reports_current_directory() {
    let (mut eval, _) = create_evaluator();
    let unique = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let temp_dir = std::env::temp_dir().join(format!("bricklogo-lang-disk-{}", unique));
    std::fs::create_dir_all(&temp_dir).unwrap();

    eval.evaluate(&format!("setdisk \"{}", temp_dir.display()))
        .unwrap();
    assert_eq!(
        eval.evaluate("disk").unwrap(),
        Some(LogoValue::Word(temp_dir.display().to_string()))
    );

    std::fs::remove_dir_all(&temp_dir).unwrap();
}

#[test]
fn test_save_without_namepage_errors() {
    let (mut eval, _) = create_evaluator();
    let err = eval.evaluate("save").unwrap_err();
    assert_eq!(err.to_string(), "No page name set (use namepage first)");
}

#[test]
fn test_load_missing_page_errors() {
    let (mut eval, _) = create_evaluator();
    let unique = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let temp_dir = std::env::temp_dir().join(format!("bricklogo-lang-missing-{}", unique));
    std::fs::create_dir_all(&temp_dir).unwrap();

    eval.evaluate(&format!("setdisk \"{}", temp_dir.display()))
        .unwrap();
    let err = eval.evaluate("load \"missing").unwrap_err();
    assert!(err.to_string().starts_with("Could not load:"));

    std::fs::remove_dir_all(&temp_dir).unwrap();
}

#[test]
fn test_setdisk_missing_path_errors() {
    let (mut eval, _) = create_evaluator();
    let unique = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let missing = std::env::temp_dir().join(format!("bricklogo-lang-nope-{}", unique));
    let err = eval
        .evaluate(&format!("setdisk \"{}", missing.display()))
        .unwrap_err();
    assert_eq!(
        err.to_string(),
        format!("Directory not found: {}", missing.display())
    );
}

#[test]
fn test_erase_missing_procedure_errors() {
    let (mut eval, _) = create_evaluator();
    let err = eval.evaluate("erase \"ghost").unwrap_err();
    assert_eq!(err.to_string(), "No procedure named \"ghost\"");
}

#[test]
fn test_page_commands_work_inside_procedure() {
    let (mut eval, output) = create_evaluator();
    let unique = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let temp_dir = std::env::temp_dir().join(format!("bricklogo-lang-proc-{}", unique));
    std::fs::create_dir_all(&temp_dir).unwrap();

    eval.evaluate(&format!("setdisk \"{}", temp_dir.display()))
        .unwrap();
    eval.evaluate("to greet print \"hi end").unwrap();
    eval.evaluate("to persist namepage \"inside save end")
        .unwrap();
    eval.evaluate("persist").unwrap();
    assert!(temp_dir.join("inside.logo").exists());

    eval.evaluate("erase \"greet").unwrap();
    assert!(eval.evaluate("greet").is_err());

    eval.evaluate("to restore load \"inside end").unwrap();
    eval.evaluate("restore").unwrap();
    eval.evaluate("greet").unwrap();
    assert_eq!(
        output.lock().unwrap().last().map(String::as_str),
        Some("hi")
    );

    std::fs::remove_dir_all(&temp_dir).unwrap();
}

#[test]
fn test_infix_with_parens_and_calls() {
    let (mut eval, _) = create_evaluator();
    eval.evaluate("to double :n output :n * 2 end").unwrap();

    assert_eq!(
        eval.evaluate("(sum 2 3) * 4").unwrap(),
        Some(LogoValue::Number(20.0))
    );
    assert_eq!(
        eval.evaluate("double 3 + 1").unwrap(),
        Some(LogoValue::Number(8.0))
    );
}

#[test]
fn test_infix_is_left_associative_without_precedence() {
    let (mut eval, _) = create_evaluator();
    assert_eq!(
        eval.evaluate("3 + 4 * 5").unwrap(),
        Some(LogoValue::Number(35.0))
    );
}

#[test]
fn test_make_at_top_level_is_global() {
    let (mut eval, output) = create_evaluator();
    eval.evaluate("make \"x 42").unwrap();
    eval.evaluate("print :x").unwrap();
    assert_eq!(output.lock().unwrap().as_slice(), &["42"]);
    assert_eq!(eval.get_global("x"), Some(LogoValue::Number(42.0)));
}

#[test]
fn test_make_in_procedure_is_global() {
    let (mut eval, output) = create_evaluator();
    eval.evaluate("to test make \"myvar 99 print :myvar end").unwrap();
    eval.evaluate("test").unwrap();
    assert_eq!(output.lock().unwrap().as_slice(), &["99"]);
    // make inside a procedure creates a global
    assert_eq!(eval.get_global("myvar"), Some(LogoValue::Number(99.0)));
}

#[test]
fn test_make_in_procedure_updates_global() {
    let (mut eval, output) = create_evaluator();
    eval.evaluate("make \"x 1").unwrap();
    eval.evaluate("to bump make \"x 2 end").unwrap();
    eval.evaluate("bump").unwrap();
    eval.evaluate("print :x").unwrap();
    assert_eq!(output.lock().unwrap().as_slice(), &["2"]);
    assert_eq!(eval.get_global("x"), Some(LogoValue::Number(2.0)));
}

#[test]
fn test_make_to_parameter_stays_local() {
    let (mut eval, _) = create_evaluator();
    eval.evaluate("to test :n make \"n 99 end").unwrap();
    eval.evaluate("test 5").unwrap();
    // Parameter is local, make updates the local, not global
    assert_eq!(eval.get_global("n"), None);
}

#[test]
fn test_global_visible_after_procedure() {
    let (mut eval, output) = create_evaluator();
    eval.evaluate("to test make \"secret 42 end").unwrap();
    eval.evaluate("test").unwrap();
    eval.evaluate("print :secret").unwrap();
    assert_eq!(output.lock().unwrap().as_slice(), &["42"]);
}

// ── foreach ─────────────────────────────────

#[test]
fn test_foreach_list() {
    let (mut eval, output) = create_evaluator();
    eval.evaluate("foreach \"x [1 2 3] [print :x]").unwrap();
    assert_eq!(output.lock().unwrap().as_slice(), &["1", "2", "3"]);
}

#[test]
fn test_foreach_word() {
    let (mut eval, output) = create_evaluator();
    eval.evaluate("foreach \"c \"abc [print :c]").unwrap();
    assert_eq!(output.lock().unwrap().as_slice(), &["a", "b", "c"]);
}

#[test]
fn test_foreach_with_computed_list() {
    let (mut eval, output) = create_evaluator();
    eval.evaluate("make \"ports [a b c]").unwrap();
    eval.evaluate("foreach \"p :ports [print :p]").unwrap();
    assert_eq!(output.lock().unwrap().as_slice(), &["a", "b", "c"]);
}

#[test]
fn test_foreach_empty_list() {
    let (mut eval, output) = create_evaluator();
    eval.evaluate("foreach \"x [] [print :x]").unwrap();
    assert!(output.lock().unwrap().is_empty());
}

// ── while / until ───────────────────────────

#[test]
fn test_while_loop() {
    let (mut eval, output) = create_evaluator();
    eval.evaluate("make \"n 3").unwrap();
    eval.evaluate("while [:n > 0] [print :n make \"n :n - 1]").unwrap();
    assert_eq!(output.lock().unwrap().as_slice(), &["3", "2", "1"]);
}

#[test]
fn test_while_false_immediately() {
    let (mut eval, output) = create_evaluator();
    eval.evaluate("while [\"false] [print \"nope]").unwrap();
    assert!(output.lock().unwrap().is_empty());
}

#[test]
fn test_until_loop() {
    let (mut eval, output) = create_evaluator();
    eval.evaluate("make \"n 1").unwrap();
    eval.evaluate("until [:n > 3] [print :n make \"n :n + 1]").unwrap();
    assert_eq!(output.lock().unwrap().as_slice(), &["1", "2", "3"]);
}

#[test]
fn test_until_true_immediately() {
    let (mut eval, output) = create_evaluator();
    eval.evaluate("until [\"true] [print \"nope]").unwrap();
    assert!(output.lock().unwrap().is_empty());
}

// ── local / localmake ───────────────────────

#[test]
fn test_localmake_stays_local() {
    let (mut eval, _) = create_evaluator();
    eval.evaluate("to test localmake \"x 42 end").unwrap();
    eval.evaluate("test").unwrap();
    let result = eval.evaluate("print :x");
    assert!(result.is_err());
}

#[test]
fn test_local_then_make() {
    let (mut eval, output) = create_evaluator();
    eval.evaluate("to test local \"x make \"x 99 print :x end").unwrap();
    eval.evaluate("test").unwrap();
    assert_eq!(output.lock().unwrap().as_slice(), &["99"]);
    // x should not leak to global
    let result = eval.evaluate("print :x");
    assert!(result.is_err());
}

#[test]
fn test_localmake_shadows_global() {
    let (mut eval, output) = create_evaluator();
    eval.evaluate("make \"x 10").unwrap();
    eval.evaluate("to test localmake \"x 20 print :x end").unwrap();
    eval.evaluate("test").unwrap();
    eval.evaluate("print :x").unwrap();
    assert_eq!(output.lock().unwrap().as_slice(), &["20", "10"]);
}

// ── power ───────────────────────────────────

#[test]
fn test_power_integer() {
    let (mut eval, _) = create_evaluator();
    assert_eq!(eval.evaluate("power 2 10").unwrap(), Some(LogoValue::Number(1024.0)));
}

#[test]
fn test_power_fractional_exponent() {
    let (mut eval, _) = create_evaluator();
    assert_eq!(eval.evaluate("power 9 0.5").unwrap(), Some(LogoValue::Number(3.0)));
}

#[test]
fn test_power_negative_exponent() {
    let (mut eval, _) = create_evaluator();
    assert_eq!(eval.evaluate("power 2 (minus 1)").unwrap(), Some(LogoValue::Number(0.5)));
}

#[test]
fn test_power_zero_exponent() {
    let (mut eval, _) = create_evaluator();
    assert_eq!(eval.evaluate("power 5 0").unwrap(), Some(LogoValue::Number(1.0)));
}

#[test]
fn test_power_zero_base() {
    let (mut eval, _) = create_evaluator();
    assert_eq!(eval.evaluate("power 0 5").unwrap(), Some(LogoValue::Number(0.0)));
}

// ── modulo ──────────────────────────────────

#[test]
fn test_modulo_positive() {
    let (mut eval, _) = create_evaluator();
    assert_eq!(eval.evaluate("modulo 10 3").unwrap(), Some(LogoValue::Number(1.0)));
}

#[test]
fn test_modulo_negative_dividend() {
    let (mut eval, _) = create_evaluator();
    // sign follows divisor (positive)
    assert_eq!(eval.evaluate("modulo (minus 10) 3").unwrap(), Some(LogoValue::Number(2.0)));
}

#[test]
fn test_modulo_negative_divisor() {
    let (mut eval, _) = create_evaluator();
    // sign follows divisor (negative)
    assert_eq!(eval.evaluate("modulo 10 (minus 3)").unwrap(), Some(LogoValue::Number(-2.0)));
}

#[test]
fn test_modulo_zero_dividend() {
    let (mut eval, _) = create_evaluator();
    assert_eq!(eval.evaluate("modulo 0 5").unwrap(), Some(LogoValue::Number(0.0)));
}

#[test]
fn test_modulo_angle_wrapping() {
    let (mut eval, _) = create_evaluator();
    // Classic use case: wrap negative angle into 0..360
    assert_eq!(eval.evaluate("modulo (minus 30) 360").unwrap(), Some(LogoValue::Number(330.0)));
}

// ── uppercase / lowercase ───────────────────

#[test]
fn test_uppercase_basic() {
    let (mut eval, _) = create_evaluator();
    assert_eq!(eval.evaluate("uppercase \"hello").unwrap(), Some(LogoValue::Word("HELLO".to_string())));
}

#[test]
fn test_lowercase_basic() {
    let (mut eval, _) = create_evaluator();
    assert_eq!(eval.evaluate("lowercase \"HELLO").unwrap(), Some(LogoValue::Word("hello".to_string())));
}

#[test]
fn test_uppercase_mixed() {
    let (mut eval, _) = create_evaluator();
    assert_eq!(eval.evaluate("uppercase \"HeLLo").unwrap(), Some(LogoValue::Word("HELLO".to_string())));
}

#[test]
fn test_lowercase_mixed() {
    let (mut eval, _) = create_evaluator();
    assert_eq!(eval.evaluate("lowercase \"HeLLo").unwrap(), Some(LogoValue::Word("hello".to_string())));
}

#[test]
fn test_uppercase_number_passthrough() {
    let (mut eval, _) = create_evaluator();
    // Numbers are converted to string representation then uppercased (no-op for digits)
    assert_eq!(eval.evaluate("uppercase 42").unwrap(), Some(LogoValue::Word("42".to_string())));
}

#[test]
fn test_lowercase_already_lowercase() {
    let (mut eval, _) = create_evaluator();
    assert_eq!(eval.evaluate("lowercase \"abc").unwrap(), Some(LogoValue::Word("abc".to_string())));
}

// ── comparison operators ────────────────────

#[test]
fn test_greater_equal() {
    let (mut eval, _) = create_evaluator();
    assert_eq!(eval.evaluate("5 >= 5").unwrap(), Some(LogoValue::Word("true".to_string())));
    assert_eq!(eval.evaluate("5 >= 6").unwrap(), Some(LogoValue::Word("false".to_string())));
    assert_eq!(eval.evaluate("6 >= 5").unwrap(), Some(LogoValue::Word("true".to_string())));
}

#[test]
fn test_greater_equal_negative() {
    let (mut eval, _) = create_evaluator();
    assert_eq!(eval.evaluate("-3 >= -5").unwrap(), Some(LogoValue::Word("true".to_string())));
    assert_eq!(eval.evaluate("-5 >= -3").unwrap(), Some(LogoValue::Word("false".to_string())));
}

#[test]
fn test_less_equal() {
    let (mut eval, _) = create_evaluator();
    assert_eq!(eval.evaluate("5 <= 5").unwrap(), Some(LogoValue::Word("true".to_string())));
    assert_eq!(eval.evaluate("6 <= 5").unwrap(), Some(LogoValue::Word("false".to_string())));
    assert_eq!(eval.evaluate("5 <= 6").unwrap(), Some(LogoValue::Word("true".to_string())));
}

#[test]
fn test_less_equal_negative() {
    let (mut eval, _) = create_evaluator();
    assert_eq!(eval.evaluate("-5 <= -3").unwrap(), Some(LogoValue::Word("true".to_string())));
    assert_eq!(eval.evaluate("-3 <= -5").unwrap(), Some(LogoValue::Word("false".to_string())));
}

#[test]
fn test_not_equal_numbers() {
    let (mut eval, _) = create_evaluator();
    assert_eq!(eval.evaluate("3 <> 4").unwrap(), Some(LogoValue::Word("true".to_string())));
    assert_eq!(eval.evaluate("3 <> 3").unwrap(), Some(LogoValue::Word("false".to_string())));
}

#[test]
fn test_not_equal_words() {
    let (mut eval, _) = create_evaluator();
    assert_eq!(eval.evaluate("\"hello <> \"world").unwrap(), Some(LogoValue::Word("true".to_string())));
    assert_eq!(eval.evaluate("\"hello <> \"hello").unwrap(), Some(LogoValue::Word("false".to_string())));
}

#[test]
fn test_not_equal_case_insensitive() {
    let (mut eval, _) = create_evaluator();
    // <> should match equal? behavior — case insensitive
    assert_eq!(eval.evaluate("\"hello <> \"HELLO").unwrap(), Some(LogoValue::Word("false".to_string())));
}

#[test]
fn test_comparison_in_condition() {
    let (mut eval, output) = create_evaluator();
    eval.evaluate("if 5 >= 3 [print \"yes]").unwrap();
    eval.evaluate("if 5 <= 3 [print \"no]").unwrap();
    eval.evaluate("if 5 <> 3 [print \"diff]").unwrap();
    assert_eq!(output.lock().unwrap().as_slice(), &["yes", "diff"]);
}

// ── foreach (extended) ──────────────────────

#[test]
fn test_foreach_modifies_outer_variable() {
    let (mut eval, _) = create_evaluator();
    eval.evaluate("make \"total 0").unwrap();
    eval.evaluate("foreach \"n [1 2 3] [make \"total :total + :n]").unwrap();
    assert_eq!(eval.evaluate(":total").unwrap(), Some(LogoValue::Number(6.0)));
}

#[test]
fn test_foreach_nested() {
    let (mut eval, output) = create_evaluator();
    eval.evaluate("foreach \"x [a b] [foreach \"y [1 2] [print word :x :y]]").unwrap();
    assert_eq!(output.lock().unwrap().as_slice(), &["a1", "a2", "b1", "b2"]);
}

// ── while / until (extended) ────────────────

#[test]
fn test_while_counter() {
    let (mut eval, _) = create_evaluator();
    eval.evaluate("make \"i 0").unwrap();
    eval.evaluate("while [:i < 5] [make \"i :i + 1]").unwrap();
    assert_eq!(eval.evaluate(":i").unwrap(), Some(LogoValue::Number(5.0)));
}

#[test]
fn test_until_counter() {
    let (mut eval, _) = create_evaluator();
    eval.evaluate("make \"i 0").unwrap();
    eval.evaluate("until [:i = 5] [make \"i :i + 1]").unwrap();
    assert_eq!(eval.evaluate(":i").unwrap(), Some(LogoValue::Number(5.0)));
}

#[test]
fn test_while_with_comparison_operators() {
    let (mut eval, output) = create_evaluator();
    eval.evaluate("make \"n 1").unwrap();
    eval.evaluate("while [:n <= 3] [print :n make \"n :n + 1]").unwrap();
    assert_eq!(output.lock().unwrap().as_slice(), &["1", "2", "3"]);
}

// ── local / localmake (extended) ────────────

#[test]
fn test_localmake_in_foreach() {
    let (mut eval, output) = create_evaluator();
    eval.evaluate("to test foreach \"x [1 2 3] [localmake \"doubled :x * 2 print :doubled] end").unwrap();
    eval.evaluate("test").unwrap();
    assert_eq!(output.lock().unwrap().as_slice(), &["2", "4", "6"]);
    // doubled should not leak
    assert!(eval.evaluate(":doubled").is_err());
}

#[test]
fn test_local_without_make_reads_empty() {
    let (mut eval, output) = create_evaluator();
    eval.evaluate("to test local \"x print :x end").unwrap();
    eval.evaluate("test").unwrap();
    // local initializes to empty string
    assert_eq!(output.lock().unwrap().as_slice(), &[""]);
}

#[test]
fn test_localmake_multiple_variables() {
    let (mut eval, output) = create_evaluator();
    eval.evaluate("to test localmake \"a 1 localmake \"b 2 print :a + :b end").unwrap();
    eval.evaluate("test").unwrap();
    assert_eq!(output.lock().unwrap().as_slice(), &["3"]);
    assert!(eval.evaluate(":a").is_err());
    assert!(eval.evaluate(":b").is_err());
}
