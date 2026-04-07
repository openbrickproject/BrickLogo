use std::sync::Arc;

use crate::error::LogoError;
use crate::evaluator::{Evaluator, PrimitiveSpec};
use crate::value::LogoValue;

pub fn register_core_primitives(eval: &mut Evaluator) {
    // ── Math ────────────────────────────────
    eval.register_primitive(
        "sum",
        PrimitiveSpec {
            min_args: 2,
            max_args: 2,
            func: Arc::new(|args, _, _| {
                Ok(Some(LogoValue::Number(
                    args[0].as_number()? + args[1].as_number()?,
                )))
            }),
        },
    );
    eval.register_primitive(
        "difference",
        PrimitiveSpec {
            min_args: 2,
            max_args: 2,
            func: Arc::new(|args, _, _| {
                Ok(Some(LogoValue::Number(
                    args[0].as_number()? - args[1].as_number()?,
                )))
            }),
        },
    );
    eval.register_primitive(
        "product",
        PrimitiveSpec {
            min_args: 2,
            max_args: 2,
            func: Arc::new(|args, _, _| {
                Ok(Some(LogoValue::Number(
                    args[0].as_number()? * args[1].as_number()?,
                )))
            }),
        },
    );
    eval.register_primitive(
        "quotient",
        PrimitiveSpec {
            min_args: 2,
            max_args: 2,
            func: Arc::new(|args, _, _| {
                let d = args[1].as_number()?;
                if d == 0.0 {
                    return Err(LogoError::Runtime("Division by zero".to_string()));
                }
                Ok(Some(LogoValue::Number(args[0].as_number()? / d)))
            }),
        },
    );
    eval.register_primitive(
        "remainder",
        PrimitiveSpec {
            min_args: 2,
            max_args: 2,
            func: Arc::new(|args, _, _| {
                Ok(Some(LogoValue::Number(
                    args[0].as_number()? % args[1].as_number()?,
                )))
            }),
        },
    );
    eval.register_primitive(
        "minus",
        PrimitiveSpec {
            min_args: 1,
            max_args: 1,
            func: Arc::new(|args, _, _| Ok(Some(LogoValue::Number(-args[0].as_number()?)))),
        },
    );
    eval.register_primitive(
        "abs",
        PrimitiveSpec {
            min_args: 1,
            max_args: 1,
            func: Arc::new(|args, _, _| Ok(Some(LogoValue::Number(args[0].as_number()?.abs())))),
        },
    );
    eval.register_primitive(
        "sqrt",
        PrimitiveSpec {
            min_args: 1,
            max_args: 1,
            func: Arc::new(|args, _, _| Ok(Some(LogoValue::Number(args[0].as_number()?.sqrt())))),
        },
    );
    eval.register_primitive(
        "sin",
        PrimitiveSpec {
            min_args: 1,
            max_args: 1,
            func: Arc::new(|args, _, _| {
                Ok(Some(LogoValue::Number(
                    (args[0].as_number()? * std::f64::consts::PI / 180.0).sin(),
                )))
            }),
        },
    );
    eval.register_primitive(
        "cos",
        PrimitiveSpec {
            min_args: 1,
            max_args: 1,
            func: Arc::new(|args, _, _| {
                Ok(Some(LogoValue::Number(
                    (args[0].as_number()? * std::f64::consts::PI / 180.0).cos(),
                )))
            }),
        },
    );
    eval.register_primitive(
        "tan",
        PrimitiveSpec {
            min_args: 1,
            max_args: 1,
            func: Arc::new(|args, _, _| {
                Ok(Some(LogoValue::Number(
                    (args[0].as_number()? * std::f64::consts::PI / 180.0).tan(),
                )))
            }),
        },
    );
    eval.register_primitive(
        "random",
        PrimitiveSpec {
            min_args: 1,
            max_args: 1,
            func: Arc::new(|args, _, _| {
                let n = args[0].as_number()? as u64;
                if n == 0 {
                    return Ok(Some(LogoValue::Number(0.0)));
                }
                let r = (std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap()
                    .subsec_nanos() as u64)
                    % n;
                Ok(Some(LogoValue::Number(r as f64)))
            }),
        },
    );
    eval.register_primitive(
        "int",
        PrimitiveSpec {
            min_args: 1,
            max_args: 1,
            func: Arc::new(|args, _, _| Ok(Some(LogoValue::Number(args[0].as_number()?.floor())))),
        },
    );
    eval.register_primitive(
        "round",
        PrimitiveSpec {
            min_args: 1,
            max_args: 1,
            func: Arc::new(|args, _, _| Ok(Some(LogoValue::Number(args[0].as_number()?.round())))),
        },
    );

    // ── Logic ───────────────────────────────
    eval.register_primitive(
        "and",
        PrimitiveSpec {
            min_args: 2,
            max_args: 2,
            func: Arc::new(|args, _, _| {
                let r = args[0].as_string() == "true" && args[1].as_string() == "true";
                Ok(Some(LogoValue::Word(
                    if r { "true" } else { "false" }.to_string(),
                )))
            }),
        },
    );
    eval.register_primitive(
        "or",
        PrimitiveSpec {
            min_args: 2,
            max_args: 2,
            func: Arc::new(|args, _, _| {
                let r = args[0].as_string() == "true" || args[1].as_string() == "true";
                Ok(Some(LogoValue::Word(
                    if r { "true" } else { "false" }.to_string(),
                )))
            }),
        },
    );
    eval.register_primitive(
        "not",
        PrimitiveSpec {
            min_args: 1,
            max_args: 1,
            func: Arc::new(|args, _, _| {
                Ok(Some(LogoValue::Word(
                    if args[0].as_string() == "true" {
                        "false"
                    } else {
                        "true"
                    }
                    .to_string(),
                )))
            }),
        },
    );
    eval.register_primitive(
        "equal?",
        PrimitiveSpec {
            min_args: 2,
            max_args: 2,
            func: Arc::new(|args, _, _| {
                Ok(Some(LogoValue::Word(
                    if args[0].logo_equal(&args[1]) {
                        "true"
                    } else {
                        "false"
                    }
                    .to_string(),
                )))
            }),
        },
    );
    eval.register_primitive(
        "number?",
        PrimitiveSpec {
            min_args: 1,
            max_args: 1,
            func: Arc::new(|args, _, _| {
                Ok(Some(LogoValue::Word(
                    if args[0].as_number().is_ok() {
                        "true"
                    } else {
                        "false"
                    }
                    .to_string(),
                )))
            }),
        },
    );
    eval.register_primitive(
        "list?",
        PrimitiveSpec {
            min_args: 1,
            max_args: 1,
            func: Arc::new(|args, _, _| {
                Ok(Some(LogoValue::Word(
                    if matches!(args[0], LogoValue::List(_)) {
                        "true"
                    } else {
                        "false"
                    }
                    .to_string(),
                )))
            }),
        },
    );
    eval.register_primitive(
        "word?",
        PrimitiveSpec {
            min_args: 1,
            max_args: 1,
            func: Arc::new(|args, _, _| {
                Ok(Some(LogoValue::Word(
                    if matches!(args[0], LogoValue::Word(_)) {
                        "true"
                    } else {
                        "false"
                    }
                    .to_string(),
                )))
            }),
        },
    );
    eval.register_primitive(
        "empty?",
        PrimitiveSpec {
            min_args: 1,
            max_args: 1,
            func: Arc::new(|args, _, _| {
                let empty = match &args[0] {
                    LogoValue::List(l) => l.is_empty(),
                    _ => args[0].as_string().is_empty(),
                };
                Ok(Some(LogoValue::Word(
                    if empty { "true" } else { "false" }.to_string(),
                )))
            }),
        },
    );
    eval.register_primitive(
        "member?",
        PrimitiveSpec {
            min_args: 2,
            max_args: 2,
            func: Arc::new(|args, _, _| {
                let found = match &args[1] {
                    LogoValue::List(l) => l.iter().any(|v| v.logo_equal(&args[0])),
                    _ => args[1].as_string().contains(&args[0].as_string()),
                };
                Ok(Some(LogoValue::Word(
                    if found { "true" } else { "false" }.to_string(),
                )))
            }),
        },
    );
    eval.register_primitive(
        "name?",
        PrimitiveSpec {
            min_args: 1,
            max_args: 1,
            func: Arc::new(|args, env, _| {
                let found = env.get_variable(&args[0].as_string()).is_ok();
                Ok(Some(LogoValue::Word(
                    if found { "true" } else { "false" }.to_string(),
                )))
            }),
        },
    );

    // ── Lists ───────────────────────────────
    eval.register_primitive(
        "first",
        PrimitiveSpec {
            min_args: 1,
            max_args: 1,
            func: Arc::new(|args, _, _| match &args[0] {
                LogoValue::List(l) if l.is_empty() => {
                    Err(LogoError::Runtime("first of empty list".to_string()))
                }
                LogoValue::List(l) => Ok(Some(l[0].clone())),
                other => {
                    let s = other.as_string();
                    if s.is_empty() {
                        Err(LogoError::Runtime("first of empty word".to_string()))
                    } else {
                        Ok(Some(LogoValue::Word(s.chars().next().unwrap().to_string())))
                    }
                }
            }),
        },
    );
    eval.register_primitive(
        "last",
        PrimitiveSpec {
            min_args: 1,
            max_args: 1,
            func: Arc::new(|args, _, _| match &args[0] {
                LogoValue::List(l) if l.is_empty() => {
                    Err(LogoError::Runtime("last of empty list".to_string()))
                }
                LogoValue::List(l) => Ok(Some(l[l.len() - 1].clone())),
                other => {
                    let s = other.as_string();
                    if s.is_empty() {
                        Err(LogoError::Runtime("last of empty word".to_string()))
                    } else {
                        Ok(Some(LogoValue::Word(s.chars().last().unwrap().to_string())))
                    }
                }
            }),
        },
    );
    eval.register_primitive(
        "butfirst",
        PrimitiveSpec {
            min_args: 1,
            max_args: 1,
            func: Arc::new(|args, _, _| match &args[0] {
                LogoValue::List(l) => Ok(Some(LogoValue::List(l[1..].to_vec()))),
                other => {
                    let s = other.as_string();
                    Ok(Some(LogoValue::Word(s.chars().skip(1).collect())))
                }
            }),
        },
    );
    eval.register_alias("bf", "butfirst");
    eval.register_primitive(
        "butlast",
        PrimitiveSpec {
            min_args: 1,
            max_args: 1,
            func: Arc::new(|args, _, _| match &args[0] {
                LogoValue::List(l) => Ok(Some(LogoValue::List(l[..l.len() - 1].to_vec()))),
                other => {
                    let s = other.as_string();
                    let chars: Vec<char> = s.chars().collect();
                    Ok(Some(LogoValue::Word(
                        chars[..chars.len() - 1].iter().collect(),
                    )))
                }
            }),
        },
    );
    eval.register_alias("bl", "butlast");
    eval.register_primitive(
        "item",
        PrimitiveSpec {
            min_args: 2,
            max_args: 2,
            func: Arc::new(|args, _, _| {
                let idx = args[0].as_number()? as usize;
                match &args[1] {
                    LogoValue::List(l) => {
                        if idx < 1 || idx > l.len() {
                            Err(LogoError::Runtime("item index out of range".to_string()))
                        } else {
                            Ok(Some(l[idx - 1].clone()))
                        }
                    }
                    other => {
                        let s = other.as_string();
                        let chars: Vec<char> = s.chars().collect();
                        if idx < 1 || idx > chars.len() {
                            Err(LogoError::Runtime("item index out of range".to_string()))
                        } else {
                            Ok(Some(LogoValue::Word(chars[idx - 1].to_string())))
                        }
                    }
                }
            }),
        },
    );
    eval.register_primitive(
        "count",
        PrimitiveSpec {
            min_args: 1,
            max_args: 1,
            func: Arc::new(|args, _, _| match &args[0] {
                LogoValue::List(l) => Ok(Some(LogoValue::Number(l.len() as f64))),
                other => Ok(Some(LogoValue::Number(other.as_string().len() as f64))),
            }),
        },
    );
    eval.register_primitive(
        "fput",
        PrimitiveSpec {
            min_args: 2,
            max_args: 2,
            func: Arc::new(|args, _, _| {
                if let LogoValue::List(l) = &args[1] {
                    let mut new = vec![args[0].clone()];
                    new.extend(l.iter().cloned());
                    Ok(Some(LogoValue::List(new)))
                } else {
                    Err(LogoError::Runtime("fput expects a list".to_string()))
                }
            }),
        },
    );
    eval.register_primitive(
        "lput",
        PrimitiveSpec {
            min_args: 2,
            max_args: 2,
            func: Arc::new(|args, _, _| {
                if let LogoValue::List(l) = &args[1] {
                    let mut new = l.clone();
                    new.push(args[0].clone());
                    Ok(Some(LogoValue::List(new)))
                } else {
                    Err(LogoError::Runtime("lput expects a list".to_string()))
                }
            }),
        },
    );
    eval.register_primitive(
        "list",
        PrimitiveSpec {
            min_args: 2,
            max_args: 2,
            func: Arc::new(|args, _, _| {
                Ok(Some(LogoValue::List(vec![
                    args[0].clone(),
                    args[1].clone(),
                ])))
            }),
        },
    );
    eval.register_primitive(
        "sentence",
        PrimitiveSpec {
            min_args: 2,
            max_args: 2,
            func: Arc::new(|args, _, _| {
                let flat_a = match &args[0] {
                    LogoValue::List(l) => l.clone(),
                    other => vec![other.clone()],
                };
                let flat_b = match &args[1] {
                    LogoValue::List(l) => l.clone(),
                    other => vec![other.clone()],
                };
                let mut result = flat_a;
                result.extend(flat_b);
                Ok(Some(LogoValue::List(result)))
            }),
        },
    );
    eval.register_alias("se", "sentence");
    eval.register_primitive(
        "word",
        PrimitiveSpec {
            min_args: 2,
            max_args: 2,
            func: Arc::new(|args, _, _| {
                Ok(Some(LogoValue::Word(format!(
                    "{}{}",
                    args[0].as_string(),
                    args[1].as_string()
                ))))
            }),
        },
    );

    // ── Output ──────────────────────────────
    let out_fn = eval.output_fn();
    let print_fn = out_fn.clone();
    eval.register_primitive(
        "print",
        PrimitiveSpec {
            min_args: 1,
            max_args: 1,
            func: Arc::new(move |args, _, _| {
                let text = match &args[0] {
                    LogoValue::List(l) => l
                        .iter()
                        .map(|v| v.as_string())
                        .collect::<Vec<_>>()
                        .join(" "),
                    other => other.as_string(),
                };
                print_fn(&text);
                Ok(None)
            }),
        },
    );
    eval.register_alias("pr", "print");

    let show_fn = out_fn.clone();
    eval.register_primitive(
        "show",
        PrimitiveSpec {
            min_args: 1,
            max_args: 1,
            func: Arc::new(move |args, _, _| {
                show_fn(&args[0].show());
                Ok(None)
            }),
        },
    );

    let type_fn = out_fn.clone();
    eval.register_primitive(
        "type",
        PrimitiveSpec {
            min_args: 1,
            max_args: 1,
            func: Arc::new(move |args, _, _| {
                type_fn(&args[0].as_string());
                Ok(None)
            }),
        },
    );

    // ── Variables ───────────────────────────
    eval.register_primitive(
        "make",
        PrimitiveSpec {
            min_args: 2,
            max_args: 2,
            func: Arc::new(|args, env, _| {
                env.set_variable(&args[0].as_string(), args[1].clone());
                Ok(None)
            }),
        },
    );
    eval.register_primitive(
        "thing",
        PrimitiveSpec {
            min_args: 1,
            max_args: 1,
            func: Arc::new(|args, env, _| Ok(Some(env.get_variable(&args[0].as_string())?))),
        },
    );

    // ── Wait ────────────────────────────────
    eval.register_primitive(
        "wait",
        PrimitiveSpec {
            min_args: 1,
            max_args: 1,
            func: Arc::new(|args, _, _| {
                let tenths = args[0].as_number()? as u64;
                std::thread::sleep(std::time::Duration::from_millis(tenths * 100));
                Ok(None)
            }),
        },
    );

    // ── Session / Pages ─────────────────────
    eval.register_primitive(
        "namepage",
        PrimitiveSpec {
            min_args: 1,
            max_args: 1,
            func: Arc::new(|args, _, eval| {
                let name = args[0].as_string();
                eval.set_page_name(&name);
                eval.system_output(&format!("Page named \"{}\"", name));
                Ok(None)
            }),
        },
    );
    eval.register_alias("np", "namepage");

    eval.register_primitive(
        "save",
        PrimitiveSpec {
            min_args: 0,
            max_args: 0,
            func: Arc::new(|_, _, eval| {
                eval.save_page()?;
                Ok(None)
            }),
        },
    );

    eval.register_primitive(
        "load",
        PrimitiveSpec {
            min_args: 1,
            max_args: 1,
            func: Arc::new(|args, _, eval| {
                let name = args[0].as_string();
                eval.load_page(&name)?;
                Ok(None)
            }),
        },
    );
    eval.register_alias("getpage", "load");
    eval.register_alias("gp", "load");

    eval.register_primitive(
        "setdisk",
        PrimitiveSpec {
            min_args: 1,
            max_args: 1,
            func: Arc::new(|args, _, eval| {
                let path = args[0].as_string();
                eval.set_disk(&path)?;
                Ok(None)
            }),
        },
    );

    eval.register_primitive(
        "disk",
        PrimitiveSpec {
            min_args: 0,
            max_args: 0,
            func: Arc::new(|_, _, eval| {
                Ok(Some(LogoValue::Word(
                    eval.disk_path().display().to_string(),
                )))
            }),
        },
    );

    eval.register_primitive(
        "erase",
        PrimitiveSpec {
            min_args: 1,
            max_args: 1,
            func: Arc::new(|args, _, eval| {
                let name = args[0].as_string();
                if eval.erase_procedure(&name) {
                    eval.system_output(&format!("Erased \"{}\"", name));
                    Ok(None)
                } else {
                    Err(LogoError::Runtime(format!(
                        "No procedure named \"{}\"",
                        name
                    )))
                }
            }),
        },
    );

    // ── Timer ───────────────────────────────
    eval.register_primitive(
        "timer",
        PrimitiveSpec {
            min_args: 0,
            max_args: 0,
            func: Arc::new(|_, _, eval| Ok(Some(LogoValue::Number(eval.timer_elapsed() as f64)))),
        },
    );
    eval.register_primitive(
        "resett",
        PrimitiveSpec {
            min_args: 0,
            max_args: 0,
            func: Arc::new(|_, _, eval| {
                eval.reset_timer();
                Ok(None)
            }),
        },
    );
}
