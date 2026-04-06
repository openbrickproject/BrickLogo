use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

use crate::ast::{AstNode, UserProcedure};
use crate::error::{LogoError, LogoResult};
use crate::parser::Parser;
use crate::tokenizer::tokenize;
use crate::value::LogoValue;

type PrimitiveFn = Box<dyn Fn(&[LogoValue], &mut Environment) -> LogoResult<Option<LogoValue>> + Send>;

pub struct PrimitiveSpec {
    pub min_args: usize,
    pub max_args: usize,
    pub func: PrimitiveFn,
}

pub struct Environment {
    variables: HashMap<String, LogoValue>,
    parent: Option<Box<Environment>>,
}

impl Environment {
    pub fn new() -> Self {
        Environment {
            variables: HashMap::new(),
            parent: None,
        }
    }

    pub fn child(parent_vars: &HashMap<String, LogoValue>) -> Self {
        Environment {
            variables: HashMap::new(),
            parent: Some(Box::new(Environment {
                variables: parent_vars.clone(),
                parent: None,
            })),
        }
    }

    pub fn set_variable(&mut self, name: &str, value: LogoValue) {
        let normalized = name.to_lowercase();
        // Walk up to find existing binding
        if self.variables.contains_key(&normalized) {
            self.variables.insert(normalized, value);
            return;
        }
        if let Some(ref mut parent) = self.parent {
            if parent.variables.contains_key(&normalized) {
                parent.variables.insert(normalized, value);
                return;
            }
        }
        self.variables.insert(normalized, value);
    }

    pub fn get_variable(&self, name: &str) -> LogoResult<LogoValue> {
        let normalized = name.to_lowercase();
        if let Some(val) = self.variables.get(&normalized) {
            return Ok(val.clone());
        }
        if let Some(ref parent) = self.parent {
            if let Some(val) = parent.variables.get(&normalized) {
                return Ok(val.clone());
            }
        }
        Err(LogoError::Runtime(format!("I don't know about \"{}\"", name)))
    }

    pub fn set_local(&mut self, name: &str, value: LogoValue) {
        self.variables.insert(name.to_lowercase(), value);
    }

    pub fn all_variables(&self) -> &HashMap<String, LogoValue> {
        &self.variables
    }
}

pub struct Evaluator {
    global_vars: HashMap<String, LogoValue>,
    procedures: HashMap<String, UserProcedure>,
    primitives: HashMap<String, PrimitiveSpec>,
    aliases: HashMap<String, String>,
    stop_requested: Arc<AtomicBool>,
    output_callback: Arc<dyn Fn(&str) + Send + Sync>,
    timer_start: std::time::Instant,
}

impl Evaluator {
    pub fn new(output_callback: Arc<dyn Fn(&str) + Send + Sync>) -> Self {
        Evaluator {
            global_vars: HashMap::new(),
            procedures: HashMap::new(),
            primitives: HashMap::new(),
            aliases: HashMap::new(),
            stop_requested: Arc::new(AtomicBool::new(false)),
            output_callback,
            timer_start: std::time::Instant::now(),
        }
    }

    pub fn output_fn(&self) -> Arc<dyn Fn(&str) + Send + Sync> {
        self.output_callback.clone()
    }

    pub fn register_primitive(&mut self, name: &str, spec: PrimitiveSpec) {
        self.primitives.insert(name.to_lowercase(), spec);
    }

    pub fn register_alias(&mut self, alias: &str, canonical: &str) {
        self.aliases.insert(alias.to_lowercase(), canonical.to_lowercase());
    }

    pub fn get_arity(&self, name: &str) -> Option<usize> {
        let normalized = name.to_lowercase();
        let resolved = self.aliases.get(&normalized).unwrap_or(&normalized);
        if let Some(prim) = self.primitives.get(resolved) {
            return Some(prim.min_args);
        }
        if let Some(proc) = self.procedures.get(resolved) {
            return Some(proc.params.len());
        }
        None
    }

    pub fn stop_flag(&self) -> Arc<AtomicBool> {
        self.stop_requested.clone()
    }

    pub fn request_stop(&self) {
        self.stop_requested.store(true, Ordering::SeqCst);
    }

    pub fn define_procedure(&mut self, name: &str, params: Vec<String>, body: Vec<AstNode>) {
        self.procedures.insert(name.to_lowercase(), UserProcedure {
            name: name.to_string(),
            params,
            body,
        });
    }

    pub fn erase_procedure(&mut self, name: &str) -> bool {
        self.procedures.remove(&name.to_lowercase()).is_some()
    }

    pub fn get_all_procedures(&self) -> Vec<&UserProcedure> {
        self.procedures.values().collect()
    }

    pub fn get_user_procedure(&self, name: &str) -> Option<&UserProcedure> {
        self.procedures.get(&name.to_lowercase())
    }

    pub fn timer_elapsed(&self) -> u64 {
        self.timer_start.elapsed().as_millis() as u64 / 100
    }

    pub fn reset_timer(&mut self) {
        self.timer_start = std::time::Instant::now();
    }

    pub fn output(&self, text: &str) {
        (self.output_callback)(text);
    }

    pub fn load_source(&mut self, source: &str) -> LogoResult<()> {
        let tokens = tokenize(source)?;
        let arities = self.build_arity_map();
        let mut parser = Parser::new(arities);
        let ast = parser.parse(tokens)?;
        for node in ast {
            self.eval_node(&node, &mut Environment::new())?;
        }
        Ok(())
    }

    pub fn evaluate(&mut self, input: &str) -> LogoResult<Option<LogoValue>> {
        self.stop_requested.store(false, Ordering::SeqCst);
        let tokens = tokenize(input)?;
        let arities = self.build_arity_map();
        let mut parser = Parser::new(arities);
        let ast = parser.parse(tokens)?;
        let mut result = None;
        let mut env = Environment::new();
        // Copy global vars into env
        for (k, v) in &self.global_vars {
            env.set_local(k, v.clone());
        }
        for node in &ast {
            match self.eval_node(node, &mut env) {
                Ok(val) => result = val,
                Err(LogoError::Stop) => return Ok(None),
                Err(LogoError::Output(val)) => return Ok(Some(val)),
                Err(e) => return Err(e),
            }
        }
        // Copy env vars back to global
        for (k, v) in env.all_variables() {
            self.global_vars.insert(k.clone(), v.clone());
        }
        Ok(result)
    }

    fn build_arity_map(&self) -> HashMap<String, usize> {
        let mut arities = HashMap::new();
        for (name, spec) in &self.primitives {
            arities.insert(name.clone(), spec.min_args);
        }
        for (alias, canonical) in &self.aliases {
            if let Some(arity) = arities.get(canonical).copied() {
                arities.insert(alias.clone(), arity);
            }
        }
        for (name, proc) in &self.procedures {
            arities.insert(name.clone(), proc.params.len());
        }
        arities
    }

    fn check_stop(&self) -> LogoResult<()> {
        if self.stop_requested.load(Ordering::SeqCst) {
            self.stop_requested.store(false, Ordering::SeqCst);
            Err(LogoError::Runtime("Stopped".to_string()))
        } else {
            Ok(())
        }
    }

    fn eval_node(&mut self, node: &AstNode, env: &mut Environment) -> LogoResult<Option<LogoValue>> {
        self.check_stop()?;

        match node {
            AstNode::Number(n) => Ok(Some(LogoValue::Number(*n))),
            AstNode::Word(s) => Ok(Some(LogoValue::Word(s.clone()))),
            AstNode::Variable(name) => {
                let val = env.get_variable(name).or_else(|_| {
                    self.global_vars.get(&name.to_lowercase())
                        .cloned()
                        .ok_or_else(|| LogoError::Runtime(format!("I don't know about \"{}\"", name)))
                })?;
                Ok(Some(val))
            }
            AstNode::List(elements) => {
                let mut result = Vec::new();
                for el in elements {
                    if let Some(val) = self.eval_node(el, env)? {
                        result.push(val);
                    }
                }
                Ok(Some(LogoValue::List(result)))
            }
            AstNode::Infix { operator, left, right } => {
                let l = self.require_value(left, env)?;
                let r = self.require_value(right, env)?;
                self.eval_infix(operator, &l, &r).map(Some)
            }
            AstNode::Paren { name, args } => {
                self.eval_call(name, args, env)
            }
            AstNode::Call { name, args, token: _ } => {
                self.eval_call(name, args, env)
            }
            AstNode::ProcDef { name, params, body } => {
                self.define_procedure(name, params.clone(), body.clone());
                Ok(None)
            }
            AstNode::Repeat { count, body } => {
                self.eval_repeat(count, body, env)
            }
            AstNode::Forever { body } => {
                self.eval_forever(body, env)
            }
            AstNode::If { condition, body } => {
                self.eval_if(condition, body, env)
            }
            AstNode::IfElse { condition, then_body, else_body } => {
                self.eval_ifelse(condition, then_body, else_body, env)
            }
            AstNode::WaitUntil { condition } => {
                self.eval_waituntil(condition, env)
            }
            AstNode::Carefully { body, handler } => {
                self.eval_carefully(body, handler, env)
            }
            AstNode::Output(value) => {
                let val = self.require_value(value, env)?;
                Err(LogoError::Output(val))
            }
            AstNode::Stop => Err(LogoError::Stop),
        }
    }

    fn require_value(&mut self, node: &AstNode, env: &mut Environment) -> LogoResult<LogoValue> {
        self.eval_node(node, env)?
            .ok_or_else(|| {
                let name = match node {
                    AstNode::Call { name, .. } | AstNode::Paren { name, .. } => name.clone(),
                    _ => "expression".to_string(),
                };
                LogoError::Runtime(format!("{} didn't output", name))
            })
    }

    fn eval_infix(&self, op: &str, l: &LogoValue, r: &LogoValue) -> LogoResult<LogoValue> {
        match op {
            "+" => Ok(LogoValue::Number(l.as_number()? + r.as_number()?)),
            "-" => Ok(LogoValue::Number(l.as_number()? - r.as_number()?)),
            "*" => Ok(LogoValue::Number(l.as_number()? * r.as_number()?)),
            "/" => {
                let divisor = r.as_number()?;
                if divisor == 0.0 {
                    Err(LogoError::Runtime("Division by zero".to_string()))
                } else {
                    Ok(LogoValue::Number(l.as_number()? / divisor))
                }
            }
            "=" => Ok(LogoValue::Word(if l.logo_equal(r) { "true" } else { "false" }.to_string())),
            "<" => Ok(LogoValue::Word(if l.as_number()? < r.as_number()? { "true" } else { "false" }.to_string())),
            ">" => Ok(LogoValue::Word(if l.as_number()? > r.as_number()? { "true" } else { "false" }.to_string())),
            _ => Err(LogoError::Runtime(format!("Unknown operator '{}'", op))),
        }
    }

    fn eval_call(&mut self, name: &str, arg_nodes: &[AstNode], env: &mut Environment) -> LogoResult<Option<LogoValue>> {
        let resolved = self.aliases.get(&name.to_lowercase()).cloned().unwrap_or_else(|| name.to_lowercase());

        // Handle timer/resett specially since they need evaluator state
        if resolved == "timer" {
            return Ok(Some(LogoValue::Number(self.timer_elapsed() as f64)));
        }
        if resolved == "resett" {
            self.reset_timer();
            return Ok(None);
        }

        // Check primitive
        if self.primitives.contains_key(&resolved) {
            let mut args = Vec::new();
            for a in arg_nodes.iter() {
                let val = self.eval_node(a, env)?
                    .ok_or_else(|| {
                        let arg_name = match a {
                            AstNode::Call { name, .. } | AstNode::Paren { name, .. } => name.clone(),
                            _ => "expression".to_string(),
                        };
                        LogoError::Runtime(format!("{} didn't output to {}", arg_name, name))
                    })?;
                args.push(val);
            }
            // Need to borrow self.primitives immutably while env is mutable
            // Use a temporary to avoid borrow conflict
            let prim = self.primitives.get(&resolved).unwrap();
            let result = (prim.func)(&args, env)?;
            return Ok(result);
        }

        // Check user procedure
        if let Some(proc) = self.procedures.get(&resolved).cloned() {
            if arg_nodes.len() != proc.params.len() {
                return Err(LogoError::Runtime(format!(
                    "{} expects {} input(s), got {}",
                    name, proc.params.len(), arg_nodes.len()
                )));
            }
            let mut args = Vec::new();
            for a in arg_nodes {
                let val = self.eval_node(a, env)?
                    .ok_or_else(|| {
                        let arg_name = match a {
                            AstNode::Call { name, .. } | AstNode::Paren { name, .. } => name.clone(),
                            _ => "expression".to_string(),
                        };
                        LogoError::Runtime(format!("{} didn't output to {}", arg_name, name))
                    })?;
                args.push(val);
            }
            let mut child_env = Environment::child(&self.global_vars);
            // Copy parent env vars
            for (k, v) in env.all_variables() {
                child_env.set_local(k, v.clone());
            }
            for (i, param) in proc.params.iter().enumerate() {
                child_env.set_local(param, args[i].clone());
            }
            for body_node in &proc.body {
                match self.eval_node(body_node, &mut child_env) {
                    Ok(_) => {}
                    Err(LogoError::Stop) => return Ok(None),
                    Err(LogoError::Output(val)) => return Ok(Some(val)),
                    Err(e) => return Err(e),
                }
            }
            // Copy vars back
            for (k, v) in child_env.all_variables() {
                if self.global_vars.contains_key(k) || env.all_variables().contains_key(k) {
                    env.set_variable(k, v.clone());
                }
            }
            return Ok(None);
        }

        Err(LogoError::Runtime(format!("I don't know how to {}", name)))
    }

    fn eval_repeat(&mut self, count_node: &AstNode, body: &[AstNode], env: &mut Environment) -> LogoResult<Option<LogoValue>> {
        let count = self.require_value(count_node, env)?.as_number()? as i64;
        for _ in 0..count {
            self.check_stop()?;
            for node in body {
                self.eval_node(node, env)?;
            }
        }
        Ok(None)
    }

    fn eval_forever(&mut self, body: &[AstNode], env: &mut Environment) -> LogoResult<Option<LogoValue>> {
        loop {
            self.check_stop()?;
            for node in body {
                self.eval_node(node, env)?;
            }
            // Yield briefly
            std::thread::sleep(std::time::Duration::from_millis(0));
        }
    }

    fn eval_if(&mut self, condition: &AstNode, body: &[AstNode], env: &mut Environment) -> LogoResult<Option<LogoValue>> {
        let val = self.require_value(condition, env)?;
        if val.is_truthy().map_err(|e| LogoError::Runtime(e))? {
            for node in body {
                self.eval_node(node, env)?;
            }
        }
        Ok(None)
    }

    fn eval_ifelse(&mut self, condition: &AstNode, then_body: &[AstNode], else_body: &[AstNode], env: &mut Environment) -> LogoResult<Option<LogoValue>> {
        let val = self.require_value(condition, env)?;
        let body = if val.is_truthy().map_err(|e| LogoError::Runtime(e))? { then_body } else { else_body };
        let mut result = None;
        for node in body {
            result = self.eval_node(node, env)?;
        }
        Ok(result)
    }

    fn eval_waituntil(&mut self, condition: &[AstNode], env: &mut Environment) -> LogoResult<Option<LogoValue>> {
        loop {
            self.check_stop()?;
            let mut result = None;
            for node in condition {
                result = self.eval_node(node, env)?;
            }
            if let Some(val) = result {
                if val.is_truthy().map_err(|e| LogoError::Runtime(e))? {
                    return Ok(None);
                }
            }
            std::thread::sleep(std::time::Duration::from_millis(50));
        }
    }

    fn eval_carefully(&mut self, body: &[AstNode], handler: &[AstNode], env: &mut Environment) -> LogoResult<Option<LogoValue>> {
        let body_result = (|| -> LogoResult<Option<LogoValue>> {
            let mut result = None;
            for node in body {
                result = self.eval_node(node, env)?;
            }
            Ok(result)
        })();

        match body_result {
            Ok(val) => Ok(val),
            Err(LogoError::Stop) => Err(LogoError::Stop),
            Err(LogoError::Output(val)) => Err(LogoError::Output(val)),
            Err(_) => {
                let mut result = None;
                for node in handler {
                    result = self.eval_node(node, env)?;
                }
                Ok(result)
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Mutex;

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
        assert_eq!(eval.evaluate("sum 3 4").unwrap(), Some(LogoValue::Number(7.0)));
    }

    #[test]
    fn test_infix() {
        let (mut eval, _) = create_evaluator();
        assert_eq!(eval.evaluate("3 + 4").unwrap(), Some(LogoValue::Number(7.0)));
        assert_eq!(eval.evaluate("10 - 3").unwrap(), Some(LogoValue::Number(7.0)));
        assert_eq!(eval.evaluate("3 * 4").unwrap(), Some(LogoValue::Number(12.0)));
        assert_eq!(eval.evaluate("10 / 2").unwrap(), Some(LogoValue::Number(5.0)));
    }

    #[test]
    fn test_comparison() {
        let (mut eval, _) = create_evaluator();
        assert_eq!(eval.evaluate("3 = 3").unwrap(), Some(LogoValue::Word("true".to_string())));
        assert_eq!(eval.evaluate("3 = 4").unwrap(), Some(LogoValue::Word("false".to_string())));
        assert_eq!(eval.evaluate("3 < 4").unwrap(), Some(LogoValue::Word("true".to_string())));
        assert_eq!(eval.evaluate("4 > 3").unwrap(), Some(LogoValue::Word("true".to_string())));
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
    fn test_undefined_variable() {
        let (mut eval, _) = create_evaluator();
        assert!(eval.evaluate("print :nope").is_err());
    }

    #[test]
    fn test_logic() {
        let (mut eval, _) = create_evaluator();
        assert_eq!(eval.evaluate("and \"true \"true").unwrap(), Some(LogoValue::Word("true".to_string())));
        assert_eq!(eval.evaluate("and \"true \"false").unwrap(), Some(LogoValue::Word("false".to_string())));
        assert_eq!(eval.evaluate("or \"false \"true").unwrap(), Some(LogoValue::Word("true".to_string())));
        assert_eq!(eval.evaluate("not \"true").unwrap(), Some(LogoValue::Word("false".to_string())));
    }

    #[test]
    fn test_list_operations() {
        let (mut eval, _) = create_evaluator();
        assert_eq!(eval.evaluate("first [a b c]").unwrap(), Some(LogoValue::Word("a".to_string())));
        assert_eq!(eval.evaluate("last [a b c]").unwrap(), Some(LogoValue::Word("c".to_string())));
        assert_eq!(eval.evaluate("count [a b c]").unwrap(), Some(LogoValue::Number(3.0)));
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
        eval.evaluate("ifelse 1 = 1 [print \"same] [print \"diff]").unwrap();
        eval.evaluate("ifelse 1 = 2 [print \"same] [print \"diff]").unwrap();
        assert_eq!(output.lock().unwrap().as_slice(), &["same", "diff"]);
    }

    #[test]
    fn test_ifelse_as_reporter() {
        let (mut eval, _) = create_evaluator();
        assert_eq!(eval.evaluate("ifelse 1 = 1 [\"yes] [\"no]").unwrap(), Some(LogoValue::Word("yes".to_string())));
        assert_eq!(eval.evaluate("ifelse 1 = 2 [\"yes] [\"no]").unwrap(), Some(LogoValue::Word("no".to_string())));
    }

    #[test]
    fn test_procedure() {
        let (mut eval, output) = create_evaluator();
        eval.evaluate("to greet :name print word \"Hello :name end").unwrap();
        eval.evaluate("greet \"World").unwrap();
        assert_eq!(output.lock().unwrap().as_slice(), &["HelloWorld"]);
    }

    #[test]
    fn test_recursion() {
        let (mut eval, output) = create_evaluator();
        eval.evaluate("to countdown :n if :n = 0 [print \"done stop] print :n countdown :n - 1 end").unwrap();
        eval.evaluate("countdown 3").unwrap();
        assert_eq!(output.lock().unwrap().as_slice(), &["3", "2", "1", "done"]);
    }

    #[test]
    fn test_output() {
        let (mut eval, _) = create_evaluator();
        eval.evaluate("to double :n output :n * 2 end").unwrap();
        assert_eq!(eval.evaluate("double 5").unwrap(), Some(LogoValue::Number(10.0)));
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
        eval.evaluate("carefully [print blorp] [print \"caught]").unwrap();
        assert_eq!(output.lock().unwrap().as_slice(), &["caught"]);
    }

    #[test]
    fn test_carefully_no_error() {
        let (mut eval, output) = create_evaluator();
        eval.evaluate("carefully [print \"ok] [print \"error]").unwrap();
        assert_eq!(output.lock().unwrap().as_slice(), &["ok"]);
    }

    #[test]
    fn test_carefully_as_reporter() {
        let (mut eval, _) = create_evaluator();
        assert_eq!(eval.evaluate("carefully [sum 1 2] [0]").unwrap(), Some(LogoValue::Number(3.0)));
        assert_eq!(eval.evaluate("carefully [blorp] [42]").unwrap(), Some(LogoValue::Number(42.0)));
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
        eval.evaluate("to test print \"before stop print \"after end").unwrap();
        eval.evaluate("test").unwrap();
        assert_eq!(output.lock().unwrap().as_slice(), &["before"]);
    }

    #[test]
    fn test_request_stop() {
        let (mut eval, _) = create_evaluator();
        let stop = eval.stop_flag();
        eval.register_primitive("tick", PrimitiveSpec {
            min_args: 0,
            max_args: 0,
            func: Box::new(move |_, _| {
                stop.store(true, Ordering::SeqCst);
                Ok(None)
            }),
        });
        assert!(eval.evaluate("forever [tick]").is_err());
    }
}
