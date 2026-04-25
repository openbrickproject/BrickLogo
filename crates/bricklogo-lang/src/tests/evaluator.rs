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
    // `forever [tick]` is halted by setting the stop flag from inside
    // `tick` itself. check_stop() fires, raises LogoError::Stop, and the
    // top-level `evaluate` intentionally catches that variant and
    // returns `Ok(None)` — pressing Esc shouldn't look like an error to
    // the caller.
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
    let result = eval.evaluate("forever [tick]");
    assert!(result.is_ok(), "stop should unwind to Ok(None), got {:?}", result);
    assert_eq!(result.unwrap(), None);
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

// ── Background task lifecycle ───────────────────

/// Build an evaluator with capturing output + system callbacks. System
/// messages route to the second returned `Arc<Mutex<Vec<String>>>` so
/// tests can observe background-task errors / panics.
fn create_evaluator_with_system_capture(
) -> (Evaluator, Arc<Mutex<Vec<String>>>, Arc<Mutex<Vec<String>>>) {
    let output = Arc::new(Mutex::new(Vec::new()));
    let system = Arc::new(Mutex::new(Vec::new()));
    let output_clone = output.clone();
    let system_clone = system.clone();
    let mut eval = Evaluator::new(Arc::new(move |text: &str| {
        output_clone.lock().unwrap().push(text.to_string());
    }));
    eval.set_system_fn(Arc::new(move |text: &str| {
        system_clone.lock().unwrap().push(text.to_string());
    }));
    crate::primitives::register_core_primitives(&mut eval);
    (eval, output, system)
}

/// Poll `cond` with a short sleep until it returns true, up to `timeout`.
fn wait_for<F: Fn() -> bool>(cond: F, timeout: std::time::Duration) -> bool {
    let deadline = std::time::Instant::now() + timeout;
    while std::time::Instant::now() < deadline {
        if cond() {
            return true;
        }
        std::thread::sleep(std::time::Duration::from_millis(5));
    }
    cond()
}

#[test]
fn test_check_stop_returns_stop_variant() {
    // Directly exercise `check_stop` — the whole point of the refactor
    // was to stop returning `LogoError::Runtime("Stopped")` and return
    // `LogoError::Stop` instead. Going through `evaluate` would hide
    // the variant because top-level `evaluate` maps Stop to `Ok(None)`
    // on purpose.
    use crate::error::LogoError;
    let (eval, _) = create_evaluator();

    // No stop requested yet → Ok(()).
    assert!(eval.check_stop().is_ok());

    // Trip the flag; next check_stop must return Stop and clear the
    // flag.
    eval.request_stop();
    let err = eval.check_stop().unwrap_err();
    assert!(
        matches!(err, LogoError::Stop),
        "check_stop returned wrong variant: {:?}",
        err
    );

    // The flag was consumed — a subsequent call succeeds again.
    assert!(eval.check_stop().is_ok());
}

#[test]
fn test_launch_surfaces_runtime_error() {
    let (mut eval, _out, system) = create_evaluator_with_system_capture();
    eval.evaluate("launch [first []]").unwrap();
    let captured_something = wait_for(
        || !system.lock().unwrap().is_empty(),
        std::time::Duration::from_secs(2),
    );
    assert!(captured_something, "background error was never surfaced");
    let msgs = system.lock().unwrap().clone();
    let joined = msgs.join(" | ");
    assert!(
        joined.contains("Background task error"),
        "expected 'Background task error' in {:?}",
        msgs
    );
}

#[test]
fn test_launch_does_not_surface_stop() {
    let (mut eval, _out, system) = create_evaluator_with_system_capture();
    eval.evaluate("launch [forever [wait 1]]").unwrap();
    // Let the task start and loop at least once.
    std::thread::sleep(std::time::Duration::from_millis(50));
    eval.kill_all_launched();
    // Give the task time to notice the stop flag and drop its child.
    let reaped = wait_for(
        || {
            let tasks = eval.launched_tasks.lock().unwrap();
            tasks.iter().all(|t| Arc::strong_count(&t.stop) == 1)
        },
        std::time::Duration::from_secs(2),
    );
    assert!(reaped, "launched task never exited after stop");
    // A stop must never be reported as an error.
    let msgs = system.lock().unwrap().clone();
    assert!(
        !msgs.iter().any(|m| m.contains("Background task error")),
        "stop was misreported as an error: {:?}",
        msgs
    );
}

#[test]
fn test_launched_tasks_vec_is_bounded() {
    let (mut eval, _out, _system) = create_evaluator_with_system_capture();
    // Launch a bunch of extremely short tasks.
    for _ in 0..10 {
        eval.evaluate("launch [print 1]").unwrap();
    }
    // Wait for every spawned thread to exit (drop its child evaluator).
    let settled = wait_for(
        || {
            let tasks = eval.launched_tasks.lock().unwrap();
            tasks.iter().all(|t| Arc::strong_count(&t.stop) == 1)
        },
        std::time::Duration::from_secs(2),
    );
    assert!(settled, "not all launched tasks finished in time");
    // The next launch triggers a prune. After it returns, the only
    // entry that should remain is the newly-pushed one.
    eval.evaluate("launch [print 2]").unwrap();
    let len = eval.launched_tasks.lock().unwrap().len();
    assert!(
        len <= 2,
        "launched_tasks did not prune: {} entries remain",
        len
    );
}

#[test]
fn test_launched_task_panic_is_reported() {
    let (mut eval, _out, system) = create_evaluator_with_system_capture();
    // Register a test-only primitive that panics inside the spawned
    // thread. `launch [boom]` should trap the panic via JoinHandle and
    // route it through system_callback.
    eval.register_primitive(
        "boom",
        PrimitiveSpec {
            min_args: 0,
            max_args: 0,
            func: Arc::new(|_, _, _| panic!("boom!")),
        },
    );
    eval.evaluate("launch [boom]").unwrap();
    // The thread panics immediately; wait for it to finish, then trigger
    // a prune (via another launch) so the join + report fires.
    let finished = wait_for(
        || {
            let tasks = eval.launched_tasks.lock().unwrap();
            tasks.iter().all(|t| Arc::strong_count(&t.stop) == 1)
        },
        std::time::Duration::from_secs(2),
    );
    assert!(finished);
    eval.evaluate("launch [print 1]").unwrap();
    let got_panic_msg = wait_for(
        || {
            system
                .lock()
                .unwrap()
                .iter()
                .any(|m| m.contains("Background task panicked"))
        },
        std::time::Duration::from_secs(2),
    );
    let msgs = system.lock().unwrap().clone();
    assert!(
        got_panic_msg,
        "expected panic report in system messages, got {:?}",
        msgs
    );
    assert!(msgs.iter().any(|m| m.contains("boom!")));
}

#[test]
fn test_panic_not_lost_under_concurrent_launches() {
    // Regression guard for the race where a fast-panicking task could
    // drop its child and fall to strong_count==1 before the parent
    // attached its handle. A second thread running prune_finished in
    // that window would reap the entry with no handle, silently losing
    // the panic. The current design holds the launched_tasks lock
    // across spawn+push so prune can't run between them. This test
    // hammers the happy path with many fast panicking launches from
    // multiple threads in parallel.
    let (mut eval, _out, system) = create_evaluator_with_system_capture();
    eval.register_primitive(
        "boom",
        PrimitiveSpec {
            min_args: 0,
            max_args: 0,
            func: Arc::new(|_, _, _| panic!("boom!")),
        },
    );

    // Wrap the evaluator so we can share it across spawner threads.
    let eval = Arc::new(Mutex::new(eval));

    let mut spawners = Vec::new();
    for _ in 0..4 {
        let eval = eval.clone();
        spawners.push(std::thread::spawn(move || {
            for _ in 0..10 {
                let mut e = eval.lock().unwrap();
                e.evaluate("launch [boom]").unwrap();
            }
        }));
    }
    for h in spawners {
        h.join().unwrap();
    }

    // Wait for every spawned boom-thread to exit.
    let settled = wait_for(
        || {
            let e = eval.lock().unwrap();
            let tasks = e.launched_tasks.lock().unwrap();
            tasks.iter().all(|t| Arc::strong_count(&t.stop) == 1)
        },
        std::time::Duration::from_secs(5),
    );
    assert!(settled, "launched tasks didn't all finish");

    // Trigger one more launch to run prune_finished on the accumulated
    // panicked entries.
    eval.lock().unwrap().evaluate("launch [print 1]").unwrap();

    // Every boom launch should have produced one panic report.
    let got = wait_for(
        || {
            system
                .lock()
                .unwrap()
                .iter()
                .filter(|m| m.contains("Background task panicked"))
                .count()
                >= 40
        },
        std::time::Duration::from_secs(5),
    );
    let msgs = system.lock().unwrap().clone();
    let panic_count = msgs
        .iter()
        .filter(|m| m.contains("Background task panicked"))
        .count();
    assert!(
        got,
        "expected 40 panic reports from 4×10 boom launches, got {} ({:?})",
        panic_count, msgs
    );
}

// ── Task observability: IDs, task, tasks, kill, waitfor ──

#[test]
fn test_launch_returns_monotonic_id() {
    let (mut eval, _) = create_evaluator();
    let a = eval.evaluate("launch [wait 1000]").unwrap();
    let b = eval.evaluate("launch [wait 1000]").unwrap();
    let c = eval.evaluate("launch [wait 1000]").unwrap();
    let as_num = |v: Option<LogoValue>| -> f64 {
        match v {
            Some(LogoValue::Number(n)) => n,
            other => panic!("expected Number, got {:?}", other),
        }
    };
    let (ai, bi, ci) = (as_num(a), as_num(b), as_num(c));
    assert!(ai < bi && bi < ci, "expected ai < bi < ci, got {} {} {}", ai, bi, ci);
    eval.kill_all_launched();
}

#[test]
fn test_launch_first_id_is_one() {
    let (mut eval, _) = create_evaluator();
    let first = eval.evaluate("launch [wait 1000]").unwrap();
    assert_eq!(first, Some(LogoValue::Number(1.0)));
    eval.kill_all_launched();
}

#[test]
fn test_task_on_main_thread_is_zero() {
    let (mut eval, _) = create_evaluator();
    let id = eval.evaluate("task").unwrap();
    assert_eq!(id, Some(LogoValue::Number(0.0)));
}

#[test]
fn test_task_inside_launch_matches_launch_return() {
    let (mut eval, output) = create_evaluator();
    // `make "j launch [print task]` — launched task prints its own ID.
    eval.evaluate("make \"j launch [print task]").unwrap();
    // Give the task a moment to run.
    for _ in 0..50 {
        if !output.lock().unwrap().is_empty() {
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(10));
    }
    let printed = output.lock().unwrap().clone();
    assert!(!printed.is_empty(), "launched task never printed");
    let task_id_printed: f64 = printed[0].trim().parse().unwrap();
    let j = eval.evaluate(":j").unwrap();
    assert_eq!(j, Some(LogoValue::Number(task_id_printed)));
}

#[test]
fn test_task_inside_procedure_called_from_launch() {
    let (mut eval, output) = create_evaluator();
    eval.evaluate("to who print task end").unwrap();
    eval.evaluate("make \"j launch [who]").unwrap();
    for _ in 0..50 {
        if !output.lock().unwrap().is_empty() {
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(10));
    }
    let printed = output.lock().unwrap().clone();
    assert!(!printed.is_empty(), "procedure inside launch didn't print");
    let task_id_printed: f64 = printed[0].trim().parse().unwrap();
    let j = eval.evaluate(":j").unwrap();
    assert_eq!(j, Some(LogoValue::Number(task_id_printed)));
}

#[test]
fn test_tasks_empty_when_idle() {
    let (mut eval, _) = create_evaluator();
    let v = eval.evaluate("tasks").unwrap();
    assert_eq!(v, Some(LogoValue::List(Vec::new())));
}

#[test]
fn test_tasks_insertion_order_preserved() {
    let (mut eval, _) = create_evaluator();
    let a = eval.evaluate("launch [forever [wait 1]]").unwrap();
    let b = eval.evaluate("launch [forever [wait 1]]").unwrap();
    let c = eval.evaluate("launch [forever [wait 1]]").unwrap();
    let get_id = |v: Option<LogoValue>| match v {
        Some(LogoValue::Number(n)) => n,
        _ => panic!(),
    };
    let (ai, bi, ci) = (get_id(a), get_id(b), get_id(c));

    // Wait a tick for the spawns to settle, then read tasks.
    std::thread::sleep(std::time::Duration::from_millis(20));
    let list = eval.evaluate("tasks").unwrap();
    match list {
        Some(LogoValue::List(v)) => {
            let ids: Vec<f64> = v
                .into_iter()
                .map(|x| match x {
                    LogoValue::Number(n) => n,
                    _ => panic!(),
                })
                .collect();
            assert_eq!(ids, vec![ai, bi, ci]);
        }
        _ => panic!(),
    }

    // Stop the middle one specifically, wait for it to exit, then
    // confirm the remaining tasks are still in launch order (a, c).
    eval.evaluate(&format!("kill {}", bi)).unwrap();
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(2);
    while std::time::Instant::now() < deadline {
        let list = eval.evaluate("tasks").unwrap();
        if let Some(LogoValue::List(v)) = &list {
            if v.len() == 2 {
                let ids: Vec<f64> = v
                    .iter()
                    .map(|x| match x {
                        LogoValue::Number(n) => *n,
                        _ => panic!(),
                    })
                    .collect();
                assert_eq!(ids, vec![ai, ci], "order should be preserved after prune");
                eval.kill_all_launched();
                return;
            }
        }
        std::thread::sleep(std::time::Duration::from_millis(20));
    }
    panic!("middle task didn't finish in time");
}

#[test]
fn test_tasks_includes_own_id_from_launched_body() {
    let (mut eval, output) = create_evaluator();
    eval.evaluate("launch [show tasks]").unwrap();
    for _ in 0..50 {
        if !output.lock().unwrap().is_empty() {
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(10));
    }
    let printed = output.lock().unwrap().clone();
    assert!(!printed.is_empty(), "launched task didn't print");
    // `show [1]` renders as `[1]` — the list should contain the task's
    // own ID.
    assert!(
        printed[0].contains("[1]"),
        "expected launched task to see itself in `tasks`: got {:?}",
        printed[0]
    );
}

#[test]
fn test_tasks_excludes_pruned_entries() {
    let (mut eval, _) = create_evaluator();
    eval.evaluate("launch [wait 1]").unwrap();
    // `wait 1` is 100 ms (tenths of a second). Wait well past that so
    // the thread has definitely exited before we query `tasks`.
    std::thread::sleep(std::time::Duration::from_millis(250));
    // Call tasks — prune runs inside — list should be empty.
    let v = eval.evaluate("tasks").unwrap();
    assert_eq!(v, Some(LogoValue::List(Vec::new())));
}

#[test]
fn test_kill_stops_specific_task() {
    let (mut eval, _) = create_evaluator();
    let id = match eval.evaluate("launch [forever [wait 1]]").unwrap() {
        Some(LogoValue::Number(n)) => n as u64,
        _ => panic!(),
    };
    // Confirm it's alive.
    std::thread::sleep(std::time::Duration::from_millis(20));
    eval.evaluate(&format!("kill {}", id)).unwrap();
    // Wait for it to exit.
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(2);
    loop {
        if std::time::Instant::now() > deadline {
            panic!("task didn't stop");
        }
        if let Some(LogoValue::List(v)) = eval.evaluate("tasks").unwrap() {
            if v.is_empty() {
                break;
            }
        }
        std::thread::sleep(std::time::Duration::from_millis(20));
    }
}

#[test]
fn test_kill_leaves_others_running() {
    let (mut eval, _) = create_evaluator();
    let a = match eval.evaluate("launch [forever [wait 1]]").unwrap() {
        Some(LogoValue::Number(n)) => n as u64,
        _ => panic!(),
    };
    let b = match eval.evaluate("launch [forever [wait 1]]").unwrap() {
        Some(LogoValue::Number(n)) => n as u64,
        _ => panic!(),
    };
    eval.evaluate(&format!("kill {}", a)).unwrap();

    // Wait for a to exit. b should still be running.
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(2);
    loop {
        if std::time::Instant::now() > deadline {
            panic!("a didn't stop");
        }
        if let Some(LogoValue::List(v)) = eval.evaluate("tasks").unwrap() {
            if v.len() == 1 {
                match &v[0] {
                    LogoValue::Number(n) => assert_eq!(*n as u64, b),
                    _ => panic!(),
                }
                break;
            }
        }
        std::thread::sleep(std::time::Duration::from_millis(20));
    }
    eval.kill_all_launched();
}

#[test]
fn test_kill_unknown_id_errors() {
    let (mut eval, _) = create_evaluator();
    let err = eval.evaluate("kill 999").unwrap_err();
    assert!(
        format!("{}", err).contains("No task with id 999"),
        "expected not-found error, got {:?}",
        err
    );
}

#[test]
fn test_kill_main_thread_errors() {
    // The main thread's task ID (0) is never tracked in launched_tasks,
    // so killing it surfaces the same not-found error as any other
    // unknown id.
    let (mut eval, _) = create_evaluator();
    let err = eval.evaluate("kill 0").unwrap_err();
    assert!(format!("{}", err).contains("No task with id 0"));
}

#[test]
fn test_kill_after_task_finished_no_intervening_prune() {
    // A task that has exited but hasn't been pruned yet (no `launch`,
    // `tasks`, or `waitfor` call between exit and kill) is still
    // physically present in `launched_tasks`. Without pruning at the
    // start of `kill_task`, lookup would find the dead entry and
    // return Ok against an already-finished task — disagreeing with
    // both the docs and the user's mental model. This guards that path.
    let (mut eval, _) = create_evaluator();
    let id = match eval.evaluate("launch [print 1]").unwrap() {
        Some(LogoValue::Number(n)) => n as u64,
        other => panic!("expected Number from launch, got {:?}", other),
    };
    // Wait for the spawned thread to exit (drops its child evaluator
    // → strong_count on stop falls to 1). Inspect the vec directly so
    // we don't accidentally trigger a prune by going through a primitive.
    let settled = wait_for(
        || {
            let tasks = eval.launched_tasks.lock().unwrap();
            tasks
                .iter()
                .any(|t| t.id == id && Arc::strong_count(&t.stop) == 1)
        },
        std::time::Duration::from_secs(2),
    );
    assert!(settled, "spawned task didn't exit in time");
    // No prune-triggering call has run since the exit. `kill` must
    // still error.
    let err = eval.evaluate(&format!("kill {}", id)).unwrap_err();
    assert!(
        format!("{}", err).contains(&format!("No task with id {}", id)),
        "expected not-found error, got {:?}",
        err
    );
}

#[test]
fn test_kill_rejects_negative_id() {
    let (mut eval, _) = create_evaluator();
    // `-1` is parsed as binary subtraction in Logo, so use the unary
    // `minus` form to get a real negative literal.
    let err = eval.evaluate("kill (minus 1)").unwrap_err();
    assert!(
        format!("{}", err).contains("non-negative integer"),
        "got {:?}",
        err
    );
}

#[test]
fn test_kill_rejects_fractional_id() {
    let (mut eval, _) = create_evaluator();
    let err = eval.evaluate("kill 1.9").unwrap_err();
    assert!(
        format!("{}", err).contains("non-negative integer"),
        "got {:?}",
        err
    );
}

#[test]
fn test_waitfor_rejects_negative_id() {
    let (mut eval, _) = create_evaluator();
    let err = eval.evaluate("waitfor (minus 5)").unwrap_err();
    assert!(format!("{}", err).contains("non-negative integer"));
}

#[test]
fn test_waitfor_rejects_fractional_id() {
    let (mut eval, _) = create_evaluator();
    let err = eval.evaluate("waitfor 2.5").unwrap_err();
    assert!(format!("{}", err).contains("non-negative integer"));
}

#[test]
fn test_kill_accepts_whole_number_float() {
    // `1.0` is conceptually an integer, just expressed as a float —
    // accept it. Mostly to confirm the validator's `n.fract() != 0.0`
    // check doesn't reject these.
    let (mut eval, _) = create_evaluator();
    // No task with id 1 exists, so we expect the not-found error,
    // *not* the validation error.
    let err = eval.evaluate("kill 1.0").unwrap_err();
    let msg = format!("{}", err);
    assert!(
        msg.contains("No task with id 1"),
        "expected not-found, got validation rejection: {:?}",
        err
    );
}

#[test]
fn test_waitfor_live_task_blocks_until_exit() {
    let (mut eval, _) = create_evaluator();
    // Task finishes after ~60 ms (wait in tenths of a second).
    let id = match eval.evaluate("launch [wait 1]").unwrap() {
        Some(LogoValue::Number(n)) => n as u64,
        _ => panic!(),
    };
    let start = std::time::Instant::now();
    eval.evaluate(&format!("waitfor {}", id)).unwrap();
    let elapsed = start.elapsed();
    // `wait 1` is 100 ms of real time. waitfor must not return earlier.
    assert!(
        elapsed >= std::time::Duration::from_millis(80),
        "waitfor returned too fast: {:?}",
        elapsed
    );
}

#[test]
fn test_waitfor_self_is_error() {
    let (mut eval, _out, system) = create_evaluator_with_system_capture();
    // `launch [waitfor task]` — the launched body tries to wait on its
    // own ID and should surface an error through the system callback.
    eval.evaluate("launch [waitfor task]").unwrap();
    let got = wait_for(
        || {
            system
                .lock()
                .unwrap()
                .iter()
                .any(|m| m.contains("Cannot wait for the current task"))
        },
        std::time::Duration::from_secs(2),
    );
    assert!(got, "self-wait wasn't reported: {:?}", system.lock().unwrap());
}

#[test]
fn test_waitfor_unknown_id_returns_immediately() {
    let (mut eval, _) = create_evaluator();
    let start = std::time::Instant::now();
    eval.evaluate("waitfor 999").unwrap();
    assert!(start.elapsed() < std::time::Duration::from_millis(50));
}

#[test]
fn test_waitfor_surfaces_panic_from_target() {
    let (mut eval, _out, system) = create_evaluator_with_system_capture();
    eval.register_primitive(
        "boom",
        PrimitiveSpec {
            min_args: 0,
            max_args: 0,
            func: Arc::new(|_, _, _| panic!("boom!")),
        },
    );
    let id = match eval.evaluate("launch [boom]").unwrap() {
        Some(LogoValue::Number(n)) => n as u64,
        _ => panic!(),
    };
    eval.evaluate(&format!("waitfor {}", id)).unwrap();
    // Panic must be reported *already*, not on the next launch.
    let msgs = system.lock().unwrap().clone();
    assert!(
        msgs.iter().any(|m| m.contains("Background task panicked")),
        "waitfor didn't surface panic: {:?}",
        msgs
    );
}

#[test]
fn test_waitfor_allows_multiple_waiters_on_same_task() {
    let (mut eval, output) = create_evaluator();
    eval.evaluate("make \"target launch [wait 3]").unwrap();
    eval.evaluate("make \"w1 launch [waitfor :target print \"a]").unwrap();
    eval.evaluate("make \"w2 launch [waitfor :target print \"b]").unwrap();

    eval.evaluate("waitfor :w1").unwrap();
    eval.evaluate("waitfor :w2").unwrap();

    let mut printed = output.lock().unwrap().clone();
    printed.sort();
    assert_eq!(printed, vec!["a".to_string(), "b".to_string()]);
}

#[test]
fn test_waitfor_allows_concurrent_waiters_on_different_tasks() {
    let (mut eval, output) = create_evaluator();
    eval.evaluate("make \"a launch [wait 2]").unwrap();
    eval.evaluate("make \"b launch [wait 3]").unwrap();
    eval.evaluate("make \"wa launch [waitfor :a print \"a]").unwrap();
    eval.evaluate("make \"wb launch [waitfor :b print \"b]").unwrap();

    eval.evaluate("waitfor :wa").unwrap();
    eval.evaluate("waitfor :wb").unwrap();

    let mut printed = output.lock().unwrap().clone();
    printed.sort();
    assert_eq!(printed, vec!["a".to_string(), "b".to_string()]);
}

#[test]
fn test_kill_all_launched_does_not_clear_vec() {
    let (mut eval, _out, _system) = create_evaluator_with_system_capture();
    eval.evaluate("launch [forever [wait 1]]").unwrap();
    // Give the task a moment to start.
    std::thread::sleep(std::time::Duration::from_millis(20));
    assert_eq!(
        eval.launched_tasks.lock().unwrap().len(),
        1,
        "expected exactly one task tracked after first launch"
    );
    eval.kill_all_launched();
    // Immediately after the signal, the entry is still tracked — prune
    // happens on the next launch so the JoinHandle can be reaped cleanly.
    assert_eq!(
        eval.launched_tasks.lock().unwrap().len(),
        1,
        "kill_all_launched should not clear the vec eagerly"
    );
    // Wait for the task to actually exit.
    let reaped = wait_for(
        || {
            let tasks = eval.launched_tasks.lock().unwrap();
            tasks.iter().all(|t| Arc::strong_count(&t.stop) == 1)
        },
        std::time::Duration::from_secs(2),
    );
    assert!(reaped);
    // Next launch prunes the stopped one and adds a new one.
    eval.evaluate("launch [print 1]").unwrap();
    let len = eval.launched_tasks.lock().unwrap().len();
    assert!(len <= 2, "expected at most 2 entries after prune, got {}", len);
}
