use std::collections::HashMap;
use std::io::{self, BufRead, Write};

use crate::runtime::executor::Value;

#[derive(Debug, Clone)]
pub struct Breakpoint {
    pub id: usize,
    pub function: String,
    pub enabled: bool,
}

#[derive(Debug, Clone, PartialEq)]
pub enum DebugAction {
    Continue,
    StepIn,
    StepOver,
    StepOut,
    Print(String),
    Locals,
    Backtrace,
    ListBreakpoints,
    Quit,
}

pub struct Debugger {
    breakpoints: Vec<Breakpoint>,
    next_bp_id: usize,
    stepping: bool,
    call_stack: Vec<StackFrame>,
}

#[derive(Debug, Clone)]
pub struct StackFrame {
    pub function: String,
    pub locals: HashMap<String, String>,
}

impl Debugger {
    pub fn new() -> Self {
        Self {
            breakpoints: Vec::new(),
            next_bp_id: 0,
            stepping: false,
            call_stack: Vec::new(),
        }
    }

    pub fn add_breakpoint(&mut self, function: &str) -> usize {
        let id = self.next_bp_id;
        self.next_bp_id += 1;
        self.breakpoints.push(Breakpoint {
            id,
            function: function.to_string(),
            enabled: true,
        });
        id
    }

    pub fn remove_breakpoint(&mut self, id: usize) -> bool {
        if let Some(pos) = self.breakpoints.iter().position(|bp| bp.id == id) {
            self.breakpoints.remove(pos);
            true
        } else {
            false
        }
    }

    pub fn should_break(&self, function: &str) -> bool {
        self.stepping
            || self
                .breakpoints
                .iter()
                .any(|bp| bp.enabled && bp.function == function)
    }

    pub fn set_stepping(&mut self, stepping: bool) {
        self.stepping = stepping;
    }

    pub fn enter_function(&mut self, name: &str, locals: &HashMap<String, Value>) {
        let frame = StackFrame {
            function: name.to_string(),
            locals: locals
                .iter()
                .map(|(k, v)| (k.clone(), format!("{v}")))
                .collect(),
        };
        self.call_stack.push(frame);
    }

    pub fn exit_function(&mut self) {
        self.call_stack.pop();
    }

    pub fn call_stack(&self) -> &[StackFrame] {
        &self.call_stack
    }

    pub fn breakpoints(&self) -> &[Breakpoint] {
        &self.breakpoints
    }

    pub fn prompt(&self) -> DebugAction {
        let stdin = io::stdin();
        let mut stdout = io::stdout();

        loop {
            print!("(kernl-dbg) ");
            stdout.flush().unwrap();

            let mut line = String::new();
            if stdin.lock().read_line(&mut line).unwrap() == 0 {
                return DebugAction::Quit;
            }

            let parts: Vec<&str> = line.trim().split_whitespace().collect();
            match parts.first().copied() {
                Some("c" | "continue") => return DebugAction::Continue,
                Some("s" | "step") => return DebugAction::StepIn,
                Some("n" | "next") => return DebugAction::StepOver,
                Some("out" | "finish") => return DebugAction::StepOut,
                Some("p" | "print") => {
                    if let Some(var) = parts.get(1) {
                        return DebugAction::Print(var.to_string());
                    }
                    println!("usage: print <variable>");
                }
                Some("locals" | "l") => return DebugAction::Locals,
                Some("bt" | "backtrace") => return DebugAction::Backtrace,
                Some("b" | "break") => {
                    if parts.get(1).is_some() {
                        println!("use add_breakpoint() before running");
                    }
                    println!("usage: break <function>");
                }
                Some("info" | "i") => {
                    if parts.get(1) == Some(&"breakpoints") {
                        return DebugAction::ListBreakpoints;
                    }
                }
                Some("q" | "quit") => return DebugAction::Quit,
                Some("h" | "help") => {
                    println!("commands:");
                    println!("  c/continue  - continue execution");
                    println!("  s/step      - step into");
                    println!("  n/next      - step over");
                    println!("  out/finish  - step out");
                    println!("  p/print <v> - print variable");
                    println!("  locals/l    - show local variables");
                    println!("  bt          - backtrace");
                    println!("  q/quit      - quit debugger");
                }
                _ => {
                    if !line.trim().is_empty() {
                        println!("unknown command: {}", line.trim());
                    }
                }
            }
        }
    }

    pub fn print_backtrace(&self) {
        for (i, frame) in self.call_stack.iter().rev().enumerate() {
            println!("#{i} {}", frame.function);
        }
    }

    pub fn print_locals(&self) {
        if let Some(frame) = self.call_stack.last() {
            for (name, val) in &frame.locals {
                println!("  {name} = {val}");
            }
        }
    }

    pub fn print_variable(&self, name: &str) {
        if let Some(frame) = self.call_stack.last() {
            if let Some(val) = frame.locals.get(name) {
                println!("  {name} = {val}");
            } else {
                println!("  {name}: not found in current scope");
            }
        }
    }

    pub fn list_breakpoints(&self) {
        for bp in &self.breakpoints {
            let status = if bp.enabled { "enabled" } else { "disabled" };
            println!("  #{}: {} ({})", bp.id, bp.function, status);
        }
    }
}

impl Default for Debugger {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn breakpoint_add_remove() {
        let mut dbg = Debugger::new();
        let id0 = dbg.add_breakpoint("main");
        let id1 = dbg.add_breakpoint("helper");

        assert_eq!(id0, 0);
        assert_eq!(id1, 1);
        assert_eq!(dbg.breakpoints().len(), 2);

        assert!(dbg.remove_breakpoint(id0));
        assert_eq!(dbg.breakpoints().len(), 1);
        assert_eq!(dbg.breakpoints()[0].function, "helper");

        assert!(!dbg.remove_breakpoint(99));
    }

    #[test]
    fn should_break_matching_function() {
        let mut dbg = Debugger::new();
        dbg.add_breakpoint("target");

        assert!(dbg.should_break("target"));
    }

    #[test]
    fn should_break_no_breakpoints() {
        let dbg = Debugger::new();
        assert!(!dbg.should_break("anything"));
    }

    #[test]
    fn should_break_stepping_mode() {
        let mut dbg = Debugger::new();
        dbg.set_stepping(true);
        assert!(dbg.should_break("any_function"));
    }

    #[test]
    fn should_break_disabled_breakpoint() {
        let mut dbg = Debugger::new();
        let id = dbg.add_breakpoint("target");
        assert!(dbg.should_break("target"));

        dbg.breakpoints.iter_mut().find(|bp| bp.id == id).unwrap().enabled = false;
        assert!(!dbg.should_break("target"));
    }

    #[test]
    fn enter_exit_function_stack() {
        let mut dbg = Debugger::new();
        let mut locals_a = HashMap::new();
        locals_a.insert("x".to_string(), Value::Int(42));

        let mut locals_b = HashMap::new();
        locals_b.insert("y".to_string(), Value::Bool(true));

        dbg.enter_function("outer", &locals_a);
        dbg.enter_function("inner", &locals_b);

        assert_eq!(dbg.call_stack().len(), 2);
        assert_eq!(dbg.call_stack()[0].function, "outer");
        assert_eq!(dbg.call_stack()[1].function, "inner");

        dbg.exit_function();
        assert_eq!(dbg.call_stack().len(), 1);
        assert_eq!(dbg.call_stack()[0].function, "outer");

        dbg.exit_function();
        assert!(dbg.call_stack().is_empty());
    }

    #[test]
    fn backtrace_ordering() {
        let mut dbg = Debugger::new();
        dbg.enter_function("main", &HashMap::new());
        dbg.enter_function("foo", &HashMap::new());
        dbg.enter_function("bar", &HashMap::new());

        let stack = dbg.call_stack();
        assert_eq!(stack.len(), 3);
        // Bottom of stack (earliest) is first
        assert_eq!(stack[0].function, "main");
        assert_eq!(stack[1].function, "foo");
        assert_eq!(stack[2].function, "bar");
    }

    #[test]
    fn frame_locals_stored() {
        let mut dbg = Debugger::new();
        let mut locals = HashMap::new();
        locals.insert("a".to_string(), Value::Int(10));
        locals.insert("b".to_string(), Value::Str("hello".to_string()));

        dbg.enter_function("test", &locals);

        let frame = &dbg.call_stack()[0];
        assert_eq!(frame.locals.get("a").unwrap(), "10");
        assert_eq!(frame.locals.get("b").unwrap(), "hello");
    }

    #[test]
    fn debug_action_variants() {
        assert_eq!(DebugAction::Continue, DebugAction::Continue);
        assert_ne!(DebugAction::Continue, DebugAction::Quit);
        assert_eq!(
            DebugAction::Print("x".to_string()),
            DebugAction::Print("x".to_string())
        );
    }
}
