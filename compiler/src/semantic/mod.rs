/// Semantic analysis — scope resolution and name checking.
///
/// Runs after parsing but before type checking. Catches undefined
/// variables, duplicate bindings, and shadowing.

use std::collections::HashMap;

use crate::parser::ast::*;
use crate::stdlib;

// ---------------------------------------------------------------------------
// Errors
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SemanticErrorKind {
    UndefinedVar,
    DuplicateBinding,
    Shadowing,
}

#[derive(Debug, Clone)]
pub struct SemanticError {
    pub message: String,
    pub line: usize,
    pub kind: SemanticErrorKind,
}

impl std::fmt::Display for SemanticError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let tag = match self.kind {
            SemanticErrorKind::UndefinedVar => "undefined",
            SemanticErrorKind::DuplicateBinding => "duplicate",
            SemanticErrorKind::Shadowing => "shadowing",
        };
        write!(f, "[{tag} line {}] {}", self.line, self.message)
    }
}

impl std::error::Error for SemanticError {}

// ---------------------------------------------------------------------------
// Bindings & scopes
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BindingKind {
    Param,
    Let,
    Mut,
    LoopVar,
    OutParam,
}

#[derive(Debug, Clone)]
pub struct BindingInfo {
    pub name: String,
    pub kind: BindingKind,
    pub defined_at: usize,
}

#[derive(Debug, Clone)]
pub struct Scope {
    bindings: HashMap<String, BindingInfo>,
}

impl Scope {
    fn new() -> Self {
        Self { bindings: HashMap::new() }
    }

    fn define(&mut self, info: BindingInfo) {
        self.bindings.insert(info.name.clone(), info);
    }

    fn contains(&self, name: &str) -> bool {
        self.bindings.contains_key(name)
    }
}

#[derive(Debug)]
pub struct ScopeStack {
    stack: Vec<Scope>,
}

impl ScopeStack {
    fn new() -> Self {
        Self { stack: vec![Scope::new()] }
    }

    fn push(&mut self) {
        self.stack.push(Scope::new());
    }

    fn pop(&mut self) {
        self.stack.pop();
    }

    fn define(&mut self, info: BindingInfo) {
        if let Some(scope) = self.stack.last_mut() {
            scope.define(info);
        }
    }

    fn lookup(&self, name: &str) -> bool {
        self.stack.iter().rev().any(|s| s.contains(name))
    }

    fn current_scope_contains(&self, name: &str) -> bool {
        self.stack.last().map_or(false, |s| s.contains(name))
    }
}

// ---------------------------------------------------------------------------
// Analyzer
// ---------------------------------------------------------------------------

pub struct SemanticAnalyzer {
    errors: Vec<SemanticError>,
    scopes: ScopeStack,
    /// User-defined function names collected in a pre-pass.
    user_functions: Vec<String>,
}

impl SemanticAnalyzer {
    pub fn check(program: &Program) -> Vec<SemanticError> {
        let mut analyzer = Self {
            errors: Vec::new(),
            scopes: ScopeStack::new(),
            user_functions: Vec::new(),
        };
        analyzer.collect_declarations(program);
        for item in &program.items {
            if let Item::Function(f) = item {
                analyzer.check_function(f);
            }
        }
        analyzer.errors
    }

    fn collect_declarations(&mut self, program: &Program) {
        for item in &program.items {
            if let Item::Function(f) = item {
                self.user_functions.push(f.name.clone());
            }
        }
    }

    fn is_callable(&self, name: &str) -> bool {
        stdlib::is_builtin(name) || self.user_functions.contains(&String::from(name))
    }

    fn check_function(&mut self, func: &Function) {
        self.scopes = ScopeStack::new();

        for (i, param) in func.params.iter().enumerate() {
            self.scopes.define(BindingInfo {
                name: param.name.clone(),
                kind: BindingKind::Param,
                defined_at: i + 1,
            });
        }

        if let Some(ref ret) = func.returns {
            self.scopes.define(BindingInfo {
                name: ret.name.clone(),
                kind: BindingKind::OutParam,
                defined_at: 0,
            });
        }

        self.check_expr(&func.body);
    }

    fn check_expr(&mut self, expr: &Expr) {
        match expr {
            Expr::Ident(name) => {
                if !self.scopes.lookup(name) && !self.is_callable(name) {
                    self.errors.push(SemanticError {
                        message: format!("undefined variable '{name}'"),
                        line: 0,
                        kind: SemanticErrorKind::UndefinedVar,
                    });
                }
            }

            Expr::Let { name, value, mutable, .. } => {
                self.check_expr(value);

                if self.scopes.current_scope_contains(name) {
                    self.errors.push(SemanticError {
                        message: format!("duplicate binding '{name}'"),
                        line: 0,
                        kind: SemanticErrorKind::DuplicateBinding,
                    });
                } else if self.scopes.lookup(name) {
                    self.errors.push(SemanticError {
                        message: format!("'{name}' shadows an outer binding"),
                        line: 0,
                        kind: SemanticErrorKind::Shadowing,
                    });
                }

                let kind = if *mutable { BindingKind::Mut } else { BindingKind::Let };
                self.scopes.define(BindingInfo {
                    name: name.clone(),
                    kind,
                    defined_at: 0,
                });
            }

            Expr::Call(name, args) => {
                if !self.is_callable(name) && !self.scopes.lookup(name) {
                    self.errors.push(SemanticError {
                        message: format!("undefined function '{name}'"),
                        line: 0,
                        kind: SemanticErrorKind::UndefinedVar,
                    });
                }
                for arg in args {
                    self.check_expr(arg);
                }
            }

            Expr::Op(_, operands) => {
                for operand in operands {
                    self.check_expr(operand);
                }
            }

            Expr::Pipe(left, right) => {
                self.check_expr(left);
                self.check_expr(right);
            }

            Expr::Field(base, _field) => {
                self.check_expr(base);
            }

            Expr::Temporal(inner, _) => {
                self.check_expr(inner);
            }

            Expr::If { condition, then_body, elif_branches, else_body } => {
                self.check_expr(condition);

                self.scopes.push();
                for expr in then_body {
                    self.check_expr(expr);
                }
                self.scopes.pop();

                for (cond, body) in elif_branches {
                    self.check_expr(cond);
                    self.scopes.push();
                    for expr in body {
                        self.check_expr(expr);
                    }
                    self.scopes.pop();
                }

                if let Some(else_exprs) = else_body {
                    self.scopes.push();
                    for expr in else_exprs {
                        self.check_expr(expr);
                    }
                    self.scopes.pop();
                }
            }

            Expr::Each { binding, iter, body } => {
                self.check_expr(iter);
                self.scopes.push();
                self.scopes.define(BindingInfo {
                    name: binding.clone(),
                    kind: BindingKind::LoopVar,
                    defined_at: 0,
                });
                for expr in body {
                    self.check_expr(expr);
                }
                self.scopes.pop();
            }

            Expr::While { condition, body } => {
                self.scopes.push();
                self.check_expr(condition);
                for expr in body {
                    self.check_expr(expr);
                }
                self.scopes.pop();
            }

            Expr::Block(exprs) => {
                for expr in exprs {
                    self.check_expr(expr);
                }
            }

            Expr::IntLit(_) | Expr::FloatLit(_) | Expr::StrLit(_) | Expr::BoolLit(_) => {}

            Expr::EnumVariant(_, _, args) => {
                for arg in args {
                    self.check_expr(arg);
                }
            }

            Expr::Match { scrutinee, arms } => {
                self.check_expr(scrutinee);
                for arm in arms {
                    self.scopes.push();
                    self.bind_pattern(&arm.pattern);
                    for expr in &arm.body {
                        self.check_expr(expr);
                    }
                    self.scopes.pop();
                }
            }

            Expr::Spawn(inner) => self.check_expr(inner),
            Expr::Await(inner) => self.check_expr(inner),
            Expr::Send(chan, val) => {
                self.check_expr(chan);
                self.check_expr(val);
            }
            Expr::Recv(chan) => self.check_expr(chan),
        }
    }

    fn bind_pattern(&mut self, pattern: &Pattern) {
        match pattern {
            Pattern::Wildcard | Pattern::Literal(_) => {}
            Pattern::Binding(name) => {
                self.scopes.define(BindingInfo {
                    name: name.clone(),
                    kind: BindingKind::Let,
                    defined_at: 0,
                });
            }
            Pattern::Variant(_, sub_pats) => {
                for pat in sub_pats {
                    self.bind_pattern(pat);
                }
            }
            Pattern::Tuple(pats) => {
                for pat in pats {
                    self.bind_pattern(pat);
                }
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::lexer::Lexer;
    use crate::parser::Parser;

    fn analyze(src: &str) -> Vec<SemanticError> {
        let tokens = Lexer::new(src).tokenize().unwrap();
        let program = Parser::new(tokens).parse_program().unwrap();
        SemanticAnalyzer::check(&program)
    }

    fn has_error(errors: &[SemanticError], kind: SemanticErrorKind, fragment: &str) -> bool {
        errors.iter().any(|e| e.kind == kind && e.message.contains(fragment))
    }

    // -- undefined variable ------------------------------------------------

    #[test]
    fn undefined_variable() {
        let errors = analyze("fn bad\n  in x: int\n  out r: int\n  do add x unknown_var");
        assert!(
            has_error(&errors, SemanticErrorKind::UndefinedVar, "unknown_var"),
            "expected undefined variable error, got: {errors:?}"
        );
    }

    // -- duplicate binding -------------------------------------------------

    #[test]
    fn duplicate_binding() {
        let src = "\
fn dup
  in x: int
  out r: int
  do if gt x 0
    let a: int = 1
    let a: int = 2
  end";
        let errors = analyze(src);
        assert!(
            has_error(&errors, SemanticErrorKind::DuplicateBinding, "a"),
            "expected duplicate binding error, got: {errors:?}"
        );
    }

    // -- builtins are not flagged ------------------------------------------

    #[test]
    fn builtins_not_flagged() {
        let errors = analyze("fn f\n  in nums: [int]\n  out r: int\n  do len nums");
        let undef: Vec<_> = errors.iter().filter(|e| e.kind == SemanticErrorKind::UndefinedVar).collect();
        assert!(undef.is_empty(), "builtins should not be undefined: {undef:?}");
    }

    // -- scope isolation ---------------------------------------------------

    #[test]
    fn scope_isolation() {
        let src = "\
fn scoped
  in x: int
  out r: int
  do if gt x 0
    let inner: int = 1
  else
    inner
  end";
        let errors = analyze(src);
        assert!(
            has_error(&errors, SemanticErrorKind::UndefinedVar, "inner"),
            "inner defined in then-branch should not be visible in else-branch: {errors:?}"
        );
    }

    // -- function params are in scope --------------------------------------

    #[test]
    fn params_in_scope() {
        let errors = analyze("fn ok\n  in a: int b: int\n  out r: int\n  do add a b");
        let undef: Vec<_> = errors.iter().filter(|e| e.kind == SemanticErrorKind::UndefinedVar).collect();
        assert!(undef.is_empty(), "params should be in scope: {undef:?}");
    }

    // -- out param in scope ------------------------------------------------

    #[test]
    fn out_param_in_scope() {
        let errors = analyze("fn f\n  in x: int\n  out result: int\n  do result");
        let undef: Vec<_> = errors.iter().filter(|e| e.kind == SemanticErrorKind::UndefinedVar).collect();
        assert!(undef.is_empty(), "out param should be in scope: {undef:?}");
    }

    // -- each loop binding -------------------------------------------------

    #[test]
    fn each_loop_binding_in_scope() {
        let errors = analyze("fn f\n  in nums: [int]\n  out r: int\n  do each n in nums\n    print n\n  end");
        let undef: Vec<_> = errors.iter().filter(|e| e.kind == SemanticErrorKind::UndefinedVar).collect();
        assert!(undef.is_empty(), "each-loop binding should be in scope: {undef:?}");
    }

    // -- shadowing warning -------------------------------------------------

    #[test]
    fn shadowing_warning() {
        let src = "\
fn shadow
  in x: int
  out r: int
  do if gt x 0
    let x: int = 1
  end";
        let errors = analyze(src);
        assert!(
            has_error(&errors, SemanticErrorKind::Shadowing, "x"),
            "expected shadowing warning for x: {errors:?}"
        );
    }

    // -- user-defined functions recognized ---------------------------------

    #[test]
    fn user_function_calls_ok() {
        let src = "\
fn helper
  in x: int
  out r: int
  do x

fn main
  in a: int
  out r: int
  do helper a";
        let errors = analyze(src);
        let undef: Vec<_> = errors.iter().filter(|e| e.kind == SemanticErrorKind::UndefinedVar).collect();
        assert!(undef.is_empty(), "user-defined function call should not be undefined: {undef:?}");
    }
}
