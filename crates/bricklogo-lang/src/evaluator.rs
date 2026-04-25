use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, RwLock};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};

use crate::ast::{AstNode, UserProcedure};
use crate::error::{LogoError, LogoResult};
use crate::parser::Parser;
use crate::tokenizer::tokenize;
use crate::value::LogoValue;

type PrimitiveFn = Arc<
    dyn Fn(&[LogoValue], &mut Environment, &mut Evaluator) -> LogoResult<Option<LogoValue>>
        + Send
        + Sync,
>;

#[derive(Clone)]
pub struct PrimitiveSpec {
    pub min_args: usize,
    pub max_args: usize,
    pub func: PrimitiveFn,
}

#[derive(Debug, Clone)]
pub struct SessionState {
    page_name: Option<String>,
    disk_path: PathBuf,
}

impl SessionState {
    fn new() -> Self {
        SessionState {
            page_name: None,
            disk_path: std::env::current_dir().unwrap_or_default(),
        }
    }
}

pub struct Environment {
    variables: HashMap<String, LogoValue>,
    parent: Option<Box<Environment>>,
    pub in_procedure: bool,
}

impl Environment {
    pub fn new() -> Self {
        Environment {
            variables: HashMap::new(),
            parent: None,
            in_procedure: false,
        }
    }

    pub fn child(parent_vars: &HashMap<String, LogoValue>) -> Self {
        Environment {
            variables: HashMap::new(),
            parent: Some(Box::new(Environment {
                variables: parent_vars.clone(),
                parent: None,
                in_procedure: false,
            })),
            in_procedure: true,
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
        Err(LogoError::Runtime(format!(
            "I don't know about \"{}\"",
            name
        )))
    }

    pub fn set_local(&mut self, name: &str, value: LogoValue) {
        self.variables.insert(name.to_lowercase(), value);
    }

    pub fn has_local(&self, name: &str) -> bool {
        let normalized = name.to_lowercase();
        if self.variables.contains_key(&normalized) {
            return true;
        }
        if let Some(ref parent) = self.parent {
            return parent.variables.contains_key(&normalized);
        }
        false
    }

    pub fn all_variables(&self) -> &HashMap<String, LogoValue> {
        &self.variables
    }
}

/// Tracking entry for a single backgrounded `launch` task.
///
/// `stop` is shared with the spawned thread's child evaluator and with
/// this tracking vec. The spawn is observable via `Arc::strong_count`:
/// while the thread is alive it holds the Arc (through its child
/// evaluator), giving `strong_count >= 2`; once it exits and drops the
/// child, only the vec entry remains (`strong_count == 1`) — that's the
/// signal for `prune_finished` to reap.
///
/// The entry is only ever inserted together with its `handle`, under
/// the `launched_tasks` lock, so there is never a window where a
/// started task is visible without a handle to join.
///
/// `id` is the monotonic task ID returned from `launch`, consumed by
/// `task`, `tasks`, `kill`, and `waitfor`. Main thread evaluators
/// use id `0` as a sentinel and never appear in `launched_tasks`.
struct LaunchedTask {
    id: u64,
    stop: Arc<AtomicBool>,
    handle: std::thread::JoinHandle<()>,
}

pub struct Evaluator {
    global_vars: Arc<RwLock<HashMap<String, LogoValue>>>,
    procedures: HashMap<String, UserProcedure>,
    primitives: HashMap<String, PrimitiveSpec>,
    aliases: HashMap<String, String>,
    stop_requested: Arc<AtomicBool>,
    output_callback: Arc<dyn Fn(&str) + Send + Sync>,
    system_callback: Arc<dyn Fn(&str) + Send + Sync>,
    timer_start: std::time::Instant,
    session: SessionState,
    launched_tasks: Arc<Mutex<Vec<LaunchedTask>>>,
    /// Shared monotonic counter across the whole evaluator tree — each
    /// child clones this Arc so nested launches from a launched task
    /// get IDs from the same sequence.
    next_task_id: Arc<AtomicU64>,
    /// The task ID this evaluator is running under. `0` on the main
    /// evaluator; set by `eval_launch` on the child before it's moved
    /// into the spawned thread.
    current_task_id: u64,
    selected_outputs: Vec<String>,
    selected_inputs: Vec<String>,
    var_broadcast: Option<std::sync::mpsc::Sender<(String, LogoValue)>>,
}

impl Evaluator {
    pub fn new(output_callback: Arc<dyn Fn(&str) + Send + Sync>) -> Self {
        Evaluator {
            global_vars: Arc::new(RwLock::new(HashMap::new())),
            procedures: HashMap::new(),
            primitives: HashMap::new(),
            aliases: HashMap::new(),
            stop_requested: Arc::new(AtomicBool::new(false)),
            output_callback,
            system_callback: Arc::new(|_| {}),
            timer_start: std::time::Instant::now(),
            session: SessionState::new(),
            launched_tasks: Arc::new(Mutex::new(Vec::new())),
            next_task_id: Arc::new(AtomicU64::new(1)),
            current_task_id: 0,
            selected_outputs: Vec::new(),
            selected_inputs: Vec::new(),
            var_broadcast: None,
        }
    }

    pub fn output_fn(&self) -> Arc<dyn Fn(&str) + Send + Sync> {
        self.output_callback.clone()
    }

    pub fn register_primitive(&mut self, name: &str, spec: PrimitiveSpec) {
        self.primitives.insert(name.to_lowercase(), spec);
    }

    pub fn set_system_fn(&mut self, system_callback: Arc<dyn Fn(&str) + Send + Sync>) {
        self.system_callback = system_callback;
    }

    pub fn register_alias(&mut self, alias: &str, canonical: &str) {
        self.aliases
            .insert(alias.to_lowercase(), canonical.to_lowercase());
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

    /// The task ID this evaluator is running under. `0` on the main
    /// evaluator; nonzero for any launched task's child evaluator.
    pub fn current_task_id(&self) -> u64 {
        self.current_task_id
    }

    /// IDs of currently-running launched tasks, in the order they were
    /// launched. Runs a prune first so finished-but-not-yet-reaped
    /// tasks don't linger in the result.
    pub fn running_task_ids(&self) -> Vec<u64> {
        let mut tasks = self.launched_tasks.lock().unwrap();
        prune_finished(&mut tasks, &self.system_callback);
        tasks.iter().map(|t| t.id).collect()
    }

    /// Signal the specified task to stop. Errors if `id` does not
    /// match a currently-running task. Prunes finished tasks before
    /// looking up so a task that exited but wasn't reaped yet is
    /// correctly reported as not running. Covers typos, already-
    /// finished tasks, and tasks that died via panic or a previous
    /// kill — all surface as the same "No task with id N" error so
    /// the caller knows the kill did not hit a live target.
    pub fn kill_task(&self, id: u64) -> LogoResult<()> {
        let mut tasks = self.launched_tasks.lock().unwrap();
        prune_finished(&mut tasks, &self.system_callback);
        if let Some(t) = tasks.iter().find(|t| t.id == id) {
            t.stop.store(true, Ordering::SeqCst);
            Ok(())
        } else {
            Err(LogoError::Runtime(format!("No task with id {}", id)))
        }
    }

    /// Block until the specified task finishes. Polls at 60 Hz and
    /// checks the caller's own stop flag each iteration so Esc
    /// interrupts the wait rather than the target task.
    ///
    /// - `id == self.current_task_id` → self-wait; returns a runtime
    ///   error immediately (would deadlock otherwise).
    /// - `id` not tracked → already finished (or never existed); returns
    ///   `Ok(())` immediately.
    /// - Otherwise: polls until the task disappears from
    ///   `launched_tasks`, pruning finished entries each iteration so
    ///   panics are surfaced right away. This remains correct with any
    ///   number of concurrent waiters on the same task.
    pub fn wait_for_task(&self, id: u64) -> LogoResult<()> {
        if id == self.current_task_id {
            return Err(LogoError::Runtime(
                "Cannot wait for the current task".to_string(),
            ));
        }
        loop {
            self.check_stop()?;
            let done = {
                let mut tasks = self.launched_tasks.lock().unwrap();
                prune_finished(&mut tasks, &self.system_callback);
                !tasks.iter().any(|t| t.id == id)
            };
            if done {
                return Ok(());
            }
            std::thread::sleep(std::time::Duration::from_millis(16));
        }
    }

    pub fn set_selected_outputs(&mut self, ports: Vec<String>) {
        self.selected_outputs = ports;
    }

    pub fn set_selected_inputs(&mut self, ports: Vec<String>) {
        self.selected_inputs = ports;
    }

    pub fn selected_outputs(&self) -> &[String] {
        &self.selected_outputs
    }

    pub fn selected_inputs(&self) -> &[String] {
        &self.selected_inputs
    }

    /// Signal every currently-launched background task to stop (the
    /// `killall` primitive). Leaves the tracking entries in place so
    /// `prune_finished` (called from the next `eval_launch`) can
    /// `join` their handles cleanly — a task that panics between the
    /// signal and the prune still gets reported.
    pub fn kill_all_launched(&self) {
        let tasks = self.launched_tasks.lock().unwrap();
        for t in tasks.iter() {
            t.stop.store(true, Ordering::SeqCst);
        }
    }

    pub fn set_global(&self, name: &str, value: LogoValue) {
        let normalized = name.to_lowercase();
        let changed = {
            let mut globals = self.global_vars.write().unwrap();
            if globals.get(&normalized) == Some(&value) {
                false
            } else {
                globals.insert(normalized.clone(), value.clone());
                true
            }
        };
        if changed {
            if let Some(ref tx) = self.var_broadcast {
            let _ = tx.send((normalized, value));
            }
        }
    }

    pub fn get_global(&self, name: &str) -> Option<LogoValue> {
        self.global_vars.read().unwrap().get(&name.to_lowercase()).cloned()
    }

    pub fn set_var_broadcast(&mut self, tx: std::sync::mpsc::Sender<(String, LogoValue)>) {
        self.var_broadcast = Some(tx);
    }

    pub fn global_vars_ref(&self) -> Arc<RwLock<HashMap<String, LogoValue>>> {
        self.global_vars.clone()
    }

    fn spawn_child(&self) -> (Evaluator, Arc<AtomicBool>) {
        let stop = Arc::new(AtomicBool::new(false));
        let child = Evaluator {
            global_vars: self.global_vars.clone(),
            procedures: self.procedures.clone(),
            primitives: self.primitives.clone(),
            aliases: self.aliases.clone(),
            stop_requested: stop.clone(),
            output_callback: self.output_callback.clone(),
            system_callback: self.system_callback.clone(),
            timer_start: self.timer_start,
            session: SessionState::new(),
            launched_tasks: self.launched_tasks.clone(),
            next_task_id: self.next_task_id.clone(),
            current_task_id: 0,
            selected_outputs: self.selected_outputs.clone(),
            selected_inputs: self.selected_inputs.clone(),
            var_broadcast: self.var_broadcast.clone(),
        };
        (child, stop)
    }

    pub fn define_procedure(&mut self, name: &str, params: Vec<String>, body: Vec<AstNode>) {
        self.procedures.insert(
            name.to_lowercase(),
            UserProcedure {
                name: name.to_string(),
                params,
                body,
            },
        );
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

    pub fn system_output(&self, text: &str) {
        (self.system_callback)(text);
    }

    pub fn page_name(&self) -> Option<&str> {
        self.session.page_name.as_deref()
    }

    pub fn disk_path(&self) -> &Path {
        &self.session.disk_path
    }

    pub fn set_page_name(&mut self, name: &str) {
        self.session.page_name = Some(name.to_string());
    }

    pub fn set_disk_path(&mut self, path: PathBuf) {
        self.session.disk_path = path;
    }

    pub fn save_page(&mut self) -> LogoResult<()> {
        let name = self.page_name().ok_or_else(|| {
            LogoError::Runtime("No page name set (use namepage first)".to_string())
        })?;
        let filename = if name.ends_with(".logo") {
            name.to_string()
        } else {
            format!("{}.logo", name)
        };
        let path = self.disk_path().join(&filename);
        let procs = self.get_all_procedures();
        let source = crate::unparse::procedures_to_source(&procs);
        std::fs::write(&path, &source)
            .map_err(|e| LogoError::Runtime(format!("Could not save: {}", e)))?;
        self.system_output(&format!("Saved {}", path.display()));
        Ok(())
    }

    pub fn load_page(&mut self, name: &str) -> LogoResult<()> {
        let filename = if name.ends_with(".logo") {
            name.to_string()
        } else {
            format!("{}.logo", name)
        };
        let path = crate::paths::resolve_bundled(&filename, self.disk_path(), "examples");
        let source = std::fs::read_to_string(&path)
            .map_err(|e| LogoError::Runtime(format!("Could not load: {}", e)))?;
        self.load_source(&source)?;
        self.session.page_name = Some(name.to_string());
        self.system_output(&format!("Loaded {}", path.display()));
        Ok(())
    }

    pub fn set_disk(&mut self, path_str: &str) -> LogoResult<()> {
        let resolved = self.disk_path().join(path_str);
        if !resolved.exists() {
            return Err(LogoError::Runtime(format!(
                "Directory not found: {}",
                resolved.display()
            )));
        }
        self.session.disk_path = resolved;
        self.system_output(&format!("Disk set to {}", self.session.disk_path.display()));
        Ok(())
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
        for node in &ast {
            match self.eval_node(node, &mut env) {
                Ok(val) => result = val,
                Err(LogoError::Stop) => return Ok(None),
                Err(LogoError::Output(val)) => return Ok(Some(val)),
                Err(e) => return Err(e),
            }
        }
        Ok(result)
    }

    pub fn build_arity_map(&self) -> HashMap<String, usize> {
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
            Err(LogoError::Stop)
        } else {
            Ok(())
        }
    }

    fn eval_node(
        &mut self,
        node: &AstNode,
        env: &mut Environment,
    ) -> LogoResult<Option<LogoValue>> {
        self.check_stop()?;

        match node {
            AstNode::Number(n) => Ok(Some(LogoValue::Number(*n))),
            AstNode::Word(s) => Ok(Some(LogoValue::Word(s.clone()))),
            AstNode::Variable(name) => {
                let val = env.get_variable(name).or_else(|_| {
                    self.get_global(name)
                        .ok_or_else(|| {
                            LogoError::Runtime(format!("I don't know about \"{}\"", name))
                        })
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
            AstNode::Infix {
                operator,
                left,
                right,
            } => {
                let l = self.require_value(left, env)?;
                let r = self.require_value(right, env)?;
                self.eval_infix(operator, &l, &r).map(Some)
            }
            AstNode::Paren { name, args } => self.eval_call(name, args, env),
            AstNode::Call {
                name,
                args,
                token: _,
            } => self.eval_call(name, args, env),
            AstNode::ProcDef { name, params, body } => {
                self.define_procedure(name, params.clone(), body.clone());
                Ok(None)
            }
            AstNode::Repeat { count, body } => self.eval_repeat(count, body, env),
            AstNode::Forever { body } => self.eval_forever(body, env),
            AstNode::Launch { body } => self.eval_launch(body),
            AstNode::If { condition, body } => self.eval_if(condition, body, env),
            AstNode::IfElse {
                condition,
                then_body,
                else_body,
            } => self.eval_ifelse(condition, then_body, else_body, env),
            AstNode::WaitUntil { condition } => self.eval_waituntil(condition, env),
            AstNode::ForEach { var, list, body } => self.eval_foreach(var, list, body, env),
            AstNode::While { condition, body } => self.eval_while(condition, body, env),
            AstNode::Until { condition, body } => self.eval_until(condition, body, env),
            AstNode::Carefully { body, handler } => self.eval_carefully(body, handler, env),
            AstNode::Output(value) => {
                let val = self.require_value(value, env)?;
                Err(LogoError::Output(val))
            }
            AstNode::Stop => Err(LogoError::Stop),
        }
    }

    fn require_value(&mut self, node: &AstNode, env: &mut Environment) -> LogoResult<LogoValue> {
        self.eval_node(node, env)?.ok_or_else(|| {
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
            "=" => Ok(LogoValue::Word(
                if l.logo_equal(r) { "true" } else { "false" }.to_string(),
            )),
            "<" => Ok(LogoValue::Word(
                if l.as_number()? < r.as_number()? {
                    "true"
                } else {
                    "false"
                }
                .to_string(),
            )),
            ">" => Ok(LogoValue::Word(
                if l.as_number()? > r.as_number()? {
                    "true"
                } else {
                    "false"
                }
                .to_string(),
            )),
            ">=" => Ok(LogoValue::Word(
                if l.as_number()? >= r.as_number()? {
                    "true"
                } else {
                    "false"
                }
                .to_string(),
            )),
            "<=" => Ok(LogoValue::Word(
                if l.as_number()? <= r.as_number()? {
                    "true"
                } else {
                    "false"
                }
                .to_string(),
            )),
            "<>" => Ok(LogoValue::Word(
                if !l.logo_equal(r) { "true" } else { "false" }.to_string(),
            )),
            _ => Err(LogoError::Runtime(format!("Unknown operator '{}'", op))),
        }
    }

    fn eval_call(
        &mut self,
        name: &str,
        arg_nodes: &[AstNode],
        env: &mut Environment,
    ) -> LogoResult<Option<LogoValue>> {
        let resolved = self
            .aliases
            .get(&name.to_lowercase())
            .cloned()
            .unwrap_or_else(|| name.to_lowercase());

        // Check primitive
        if self.primitives.contains_key(&resolved) {
            let mut args = Vec::new();
            for a in arg_nodes.iter() {
                let val = self.eval_node(a, env)?.ok_or_else(|| {
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
            let prim_fn = self.primitives.get(&resolved).unwrap().func.clone();
            let result = (prim_fn)(&args, env, self)?;
            return Ok(result);
        }

        // Check user procedure
        if let Some(proc) = self.procedures.get(&resolved).cloned() {
            if arg_nodes.len() != proc.params.len() {
                return Err(LogoError::Runtime(format!(
                    "{} expects {} input(s), got {}",
                    name,
                    proc.params.len(),
                    arg_nodes.len()
                )));
            }
            let mut args = Vec::new();
            for a in arg_nodes {
                let val = self.eval_node(a, env)?.ok_or_else(|| {
                    let arg_name = match a {
                        AstNode::Call { name, .. } | AstNode::Paren { name, .. } => name.clone(),
                        _ => "expression".to_string(),
                    };
                    LogoError::Runtime(format!("{} didn't output to {}", arg_name, name))
                })?;
                args.push(val);
            }
            let mut child_env = Environment::new();
            child_env.in_procedure = true;
            // Copy caller's local variables so they're visible
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
            return Ok(None);
        }

        Err(LogoError::Runtime(format!("I don't know how to {}", name)))
    }

    fn eval_repeat(
        &mut self,
        count_node: &AstNode,
        body: &[AstNode],
        env: &mut Environment,
    ) -> LogoResult<Option<LogoValue>> {
        let count = self.require_value(count_node, env)?.as_number()? as i64;
        for _ in 0..count {
            self.check_stop()?;
            for node in body {
                self.eval_node(node, env)?;
            }
        }
        Ok(None)
    }

    fn eval_forever(
        &mut self,
        body: &[AstNode],
        env: &mut Environment,
    ) -> LogoResult<Option<LogoValue>> {
        loop {
            self.check_stop()?;
            for node in body {
                self.eval_node(node, env)?;
            }
            // Yield at ~60hz to avoid CPU spinning and lock starvation
            std::thread::sleep(std::time::Duration::from_millis(16));
        }
    }

    fn eval_launch(&mut self, body: &[AstNode]) -> LogoResult<Option<LogoValue>> {
        let (mut child, stop_flag) = self.spawn_child();
        let body = body.to_vec();
        let system_fn = self.system_callback.clone();
        let closure_system_fn = system_fn.clone();

        // Allocate a monotonic task ID for this launch. The child
        // evaluator receives it so the `task` primitive can report it
        // from inside the launched body. IDs start at 1; `0` is the
        // main-thread sentinel.
        let id = self.next_task_id.fetch_add(1, Ordering::SeqCst);
        child.current_task_id = id;

        // Hold the `launched_tasks` lock across thread::spawn AND the
        // push. That window is microseconds. It closes two races at once:
        //
        //   - `kill_all_launched` can't miss a just-launched task — it
        //     blocks on this lock, then sees the new entry.
        //   - A concurrent `eval_launch` on another thread can't run
        //     `prune_finished` between spawn and push. Before the fix
        //     that was possible: a fast-exiting thread would drop its
        //     child, strong_count would fall to 1, and a racing prune
        //     could reap the entry before the parent had attached a
        //     handle, silently dropping any panic.
        //
        // The spawned thread can only re-enter this lock via a nested
        // `launch` primitive inside its body. That requires parsing and
        // evaluating at least one node, which takes orders of magnitude
        // longer than our spawn+push window — no deadlock in practice.
        let mut tasks = self.launched_tasks.lock().unwrap();
        prune_finished(&mut tasks, &system_fn);
        let handle = std::thread::spawn(move || {
            let mut env = Environment::new();
            for node in &body {
                if child.check_stop().is_err() {
                    return;
                }
                if let Err(e) = child.eval_node(node, &mut env) {
                    // Silent exit on a stop signal — that's the normal
                    // "user hit Esc" / `killall` path. Everything else
                    // is a bug the user wants to know about.
                    if !matches!(e, LogoError::Stop) {
                        closure_system_fn(&format!("Background task error: {}", e));
                    }
                    return;
                }
            }
        });
        tasks.push(LaunchedTask { id, stop: stop_flag, handle });
        drop(tasks);

        Ok(Some(LogoValue::Number(id as f64)))
    }

    fn eval_if(
        &mut self,
        condition: &AstNode,
        body: &[AstNode],
        env: &mut Environment,
    ) -> LogoResult<Option<LogoValue>> {
        let val = self.require_value(condition, env)?;
        if val.is_truthy().map_err(|e| LogoError::Runtime(e))? {
            for node in body {
                self.eval_node(node, env)?;
            }
        }
        Ok(None)
    }

    fn eval_ifelse(
        &mut self,
        condition: &AstNode,
        then_body: &[AstNode],
        else_body: &[AstNode],
        env: &mut Environment,
    ) -> LogoResult<Option<LogoValue>> {
        let val = self.require_value(condition, env)?;
        let body = if val.is_truthy().map_err(|e| LogoError::Runtime(e))? {
            then_body
        } else {
            else_body
        };
        let mut result = None;
        for node in body {
            result = self.eval_node(node, env)?;
        }
        Ok(result)
    }

    fn eval_waituntil(
        &mut self,
        condition: &[AstNode],
        env: &mut Environment,
    ) -> LogoResult<Option<LogoValue>> {
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
            std::thread::sleep(std::time::Duration::from_millis(16));
        }
    }

    fn eval_carefully(
        &mut self,
        body: &[AstNode],
        handler: &[AstNode],
        env: &mut Environment,
    ) -> LogoResult<Option<LogoValue>> {
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

    fn eval_foreach(
        &mut self,
        var: &str,
        list_node: &AstNode,
        body: &[AstNode],
        env: &mut Environment,
    ) -> LogoResult<Option<LogoValue>> {
        let list_val = self.require_value(list_node, env)?;
        let items = match list_val {
            LogoValue::List(l) => l,
            LogoValue::Word(w) => w.chars().map(|c| LogoValue::Word(c.to_string())).collect(),
            LogoValue::Number(_) => {
                return Err(LogoError::Runtime(
                    "foreach expects a list or word".to_string(),
                ));
            }
        };
        for item in items {
            self.check_stop()?;
            env.set_local(var, item);
            for node in body {
                self.eval_node(node, env)?;
            }
        }
        Ok(None)
    }

    fn eval_while(
        &mut self,
        condition: &[AstNode],
        body: &[AstNode],
        env: &mut Environment,
    ) -> LogoResult<Option<LogoValue>> {
        loop {
            self.check_stop()?;
            let mut result = None;
            for node in condition {
                result = self.eval_node(node, env)?;
            }
            let truthy = result
                .as_ref()
                .map(|v| v.is_truthy())
                .unwrap_or(Ok(false))
                .map_err(|e| LogoError::Runtime(e))?;
            if !truthy {
                return Ok(None);
            }
            for node in body {
                self.eval_node(node, env)?;
            }
        }
    }

    fn eval_until(
        &mut self,
        condition: &[AstNode],
        body: &[AstNode],
        env: &mut Environment,
    ) -> LogoResult<Option<LogoValue>> {
        loop {
            self.check_stop()?;
            let mut result = None;
            for node in condition {
                result = self.eval_node(node, env)?;
            }
            let truthy = result
                .as_ref()
                .map(|v| v.is_truthy())
                .unwrap_or(Ok(false))
                .map_err(|e| LogoError::Runtime(e))?;
            if truthy {
                return Ok(None);
            }
            for node in body {
                self.eval_node(node, env)?;
            }
        }
    }
}

/// Reap any `LaunchedTask` whose spawned thread has finished. Detection
/// is via `Arc::strong_count`: while the thread is alive it holds the
/// stop Arc (through its child evaluator), giving `strong_count >= 2`;
/// once it exits and drops the child, the vec's own entry is the only
/// remaining reference (`strong_count == 1`). For reaped entries we
/// `join` the handle so a panicked thread gets reported through
/// `system_fn` rather than disappearing silently.
/// Order-preserving: we drain the vec into a temporary, keep live
/// entries in the same relative order, and join+report on every reaped
/// entry. Preserving insertion order matters for the `tasks` primitive,
/// which shows users the order in which their launches happened.
fn prune_finished(
    tasks: &mut Vec<LaunchedTask>,
    system_fn: &Arc<dyn Fn(&str) + Send + Sync>,
) {
    let mut kept: Vec<LaunchedTask> = Vec::with_capacity(tasks.len());
    for task in tasks.drain(..) {
        if Arc::strong_count(&task.stop) == 1 {
            if let Err(panic) = task.handle.join() {
                system_fn(&format!(
                    "Background task panicked: {}",
                    panic_message(&panic)
                ));
            }
        } else {
            kept.push(task);
        }
    }
    *tasks = kept;
}

fn panic_message(payload: &Box<dyn std::any::Any + Send>) -> String {
    if let Some(s) = payload.downcast_ref::<&str>() {
        return (*s).to_string();
    }
    if let Some(s) = payload.downcast_ref::<String>() {
        return s.clone();
    }
    "unknown panic".to_string()
}

#[cfg(test)]
#[path = "tests/evaluator.rs"]
mod tests;
