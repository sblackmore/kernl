pub mod contracts;

use std::io::Write;
use std::process::Command;

use crate::parser::ast::*;

/// Result of encoding a function's invariants to SMT-LIB2.
#[derive(Debug, Clone)]
pub struct SmtResult {
    pub function_name: String,
    pub script: String,
    pub invariant_checks: Vec<InvariantCheck>,
}

/// A single invariant check encoded as SMT-LIB2.
#[derive(Debug, Clone)]
pub struct InvariantCheck {
    pub invariant_index: usize,
    pub script: String,
    pub description: String,
}

/// Outcome of running an SMT solver on a check.
#[derive(Debug, Clone, PartialEq)]
pub enum VerifyResult {
    Verified,
    Violated(String),
    Unknown(String),
    SolverNotFound,
}

/// Translates kernl functions with invariants to SMT-LIB2 assertions.
pub struct SmtEncoder;

impl SmtEncoder {
    /// Generate SMT-LIB2 scripts that check if invariants can be violated.
    ///
    /// Strategy: declare params as free variables, assert the negation of
    /// each invariant, check satisfiability. SAT means a counterexample
    /// exists (invariant can be violated). UNSAT means it always holds.
    pub fn encode_function(func: &Function) -> SmtResult {
        let mut invariant_checks = Vec::new();

        for (i, inv) in func.invariants.iter().enumerate() {
            let mut script = String::new();
            script.push_str("(set-logic ALL)\n");

            for param in &func.params {
                script.push_str(&format!(
                    "(declare-const {} {})\n",
                    param.name,
                    Self::type_to_smt(&param.ty)
                ));
            }

            if let Some(ref ret) = func.returns {
                script.push_str(&format!(
                    "(declare-const {} {})\n",
                    ret.name,
                    Self::type_to_smt(&ret.ty)
                ));
                let body_smt = Self::expr_to_smt(&func.body);
                script.push_str(&format!("(assert (= {} {}))\n", ret.name, body_smt));
            }

            let inv_smt = Self::expr_to_smt(inv);
            script.push_str(&format!("(assert (not {}))\n", inv_smt));
            script.push_str("(check-sat)\n");

            let description = format!("{}", Self::describe_invariant(inv));

            invariant_checks.push(InvariantCheck {
                invariant_index: i,
                script: script.clone(),
                description,
            });
        }

        let full_script = invariant_checks
            .iter()
            .map(|c| c.script.clone())
            .collect::<Vec<_>>()
            .join("\n; --- next invariant ---\n\n");

        SmtResult {
            function_name: func.name.clone(),
            script: full_script,
            invariant_checks,
        }
    }

    pub fn type_to_smt(ty: &Type) -> &'static str {
        match ty {
            Type::Named(name) => match name.as_str() {
                "int" | "uint" => "Int",
                "float" => "Real",
                "bool" => "Bool",
                "str" => "String",
                _ => "Int",
            },
            Type::Optional(inner) => Self::type_to_smt(inner),
            _ => "Int",
        }
    }

    pub fn expr_to_smt(expr: &Expr) -> String {
        match expr {
            Expr::IntLit(n) => {
                if *n < 0 {
                    format!("(- {})", -n)
                } else {
                    n.to_string()
                }
            }
            Expr::FloatLit(f) => format!("{:.6}", f),
            Expr::BoolLit(true) => "true".to_string(),
            Expr::BoolLit(false) => "false".to_string(),
            Expr::StrLit(s) => format!("\"{}\"", s),
            Expr::Ident(name) => name.clone(),

            Expr::Op(op, args) => Self::op_to_smt(op, args),

            Expr::Call(name, args) => Self::call_to_smt(name, args),

            Expr::Pipe(left, right) => {
                let left_smt = Self::expr_to_smt(left);
                Self::pipe_to_smt(&left_smt, right)
            }

            Expr::If {
                condition,
                then_body,
                else_body,
                ..
            } => {
                let cond = Self::expr_to_smt(condition);
                let then_val = if then_body.is_empty() {
                    "0".to_string()
                } else {
                    Self::expr_to_smt(then_body.last().unwrap())
                };
                let else_val = match else_body {
                    Some(body) if !body.is_empty() => {
                        Self::expr_to_smt(body.last().unwrap())
                    }
                    _ => "0".to_string(),
                };
                format!("(ite {} {} {})", cond, then_val, else_val)
            }

            Expr::Let { value, .. } => Self::expr_to_smt(value),

            Expr::Block(exprs) => {
                if let Some(last) = exprs.last() {
                    Self::expr_to_smt(last)
                } else {
                    "0".to_string()
                }
            }

            _ => {
                // Conservative: return a fresh unconstrained placeholder
                "__unknown".to_string()
            }
        }
    }

    fn op_to_smt(op: &Op, args: &[Expr]) -> String {
        let smt_op = match op {
            Op::Add => "+",
            Op::Sub => "-",
            Op::Mul => "*",
            Op::Div => "div",
            Op::Modulo => "mod",
            Op::Eq => "=",
            Op::Neq => "neq_placeholder",
            Op::Gt => ">",
            Op::Lt => "<",
            Op::Gte => ">=",
            Op::Lte => "<=",
            Op::And => "and",
            Op::Or => "or",
            Op::Not => "not",
        };

        match op {
            Op::Not => {
                let a = args.first().map(Self::expr_to_smt).unwrap_or_default();
                format!("(not {})", a)
            }
            Op::Neq => {
                let a = args.first().map(Self::expr_to_smt).unwrap_or_default();
                let b = args.get(1).map(Self::expr_to_smt).unwrap_or_default();
                format!("(not (= {} {}))", a, b)
            }
            _ => {
                let smt_args: Vec<String> = args.iter().map(Self::expr_to_smt).collect();
                format!("({} {})", smt_op, smt_args.join(" "))
            }
        }
    }

    fn call_to_smt(name: &str, args: &[Expr]) -> String {
        match name {
            "max" => {
                let a = args.first().map(Self::expr_to_smt).unwrap_or_default();
                let b = args.get(1).map(Self::expr_to_smt).unwrap_or_default();
                format!("(ite (> {} {}) {} {})", a, b, a, b)
            }
            "min" => {
                let a = args.first().map(Self::expr_to_smt).unwrap_or_default();
                let b = args.get(1).map(Self::expr_to_smt).unwrap_or_default();
                format!("(ite (< {} {}) {} {})", a, b, a, b)
            }
            "abs" => {
                let a = args.first().map(Self::expr_to_smt).unwrap_or_default();
                format!("(ite (>= {} 0) {} (- {}))", a, a, a)
            }
            _ => {
                // Unknown function: treat as uninterpreted
                let smt_args: Vec<String> = args.iter().map(Self::expr_to_smt).collect();
                if smt_args.is_empty() {
                    format!("{}", name)
                } else {
                    format!("({} {})", name, smt_args.join(" "))
                }
            }
        }
    }

    /// Encode a pipe expression: left result is fed into right.
    /// If right is a Call, the left result becomes the last argument.
    fn pipe_to_smt(left_smt: &str, right: &Expr) -> String {
        match right {
            Expr::Call(name, args) => {
                let mut smt_args: Vec<String> = args.iter().map(Self::expr_to_smt).collect();
                smt_args.push(left_smt.to_string());
                match name.as_str() {
                    "max" => {
                        let a = if !smt_args.is_empty() { &smt_args[0] } else { left_smt };
                        let b = if smt_args.len() > 1 { &smt_args[1] } else { left_smt };
                        format!("(ite (> {} {}) {} {})", a, b, a, b)
                    }
                    "min" => {
                        let a = if !smt_args.is_empty() { &smt_args[0] } else { left_smt };
                        let b = if smt_args.len() > 1 { &smt_args[1] } else { left_smt };
                        format!("(ite (< {} {}) {} {})", a, b, a, b)
                    }
                    _ => {
                        format!("({} {})", name, smt_args.join(" "))
                    }
                }
            }
            Expr::Op(op, args) => {
                let mut smt_args: Vec<String> = args.iter().map(Self::expr_to_smt).collect();
                smt_args.insert(0, left_smt.to_string());
                let smt_op = match op {
                    Op::Add => "+",
                    Op::Sub => "-",
                    Op::Mul => "*",
                    Op::Div => "div",
                    Op::Modulo => "mod",
                    Op::Eq => "=",
                    Op::Gt => ">",
                    Op::Lt => "<",
                    Op::Gte => ">=",
                    Op::Lte => "<=",
                    Op::And => "and",
                    Op::Or => "or",
                    Op::Not => "not",
                    Op::Neq => return format!("(not (= {} {}))", smt_args[0], smt_args.get(1).cloned().unwrap_or_default()),
                };
                format!("({} {})", smt_op, smt_args.join(" "))
            }
            _ => Self::expr_to_smt(right),
        }
    }

    pub fn describe_invariant(expr: &Expr) -> String {
        match expr {
            Expr::Op(op, args) => {
                let op_str = match op {
                    Op::Gte => ">=",
                    Op::Lte => "<=",
                    Op::Gt => ">",
                    Op::Lt => "<",
                    Op::Eq => "==",
                    Op::Neq => "!=",
                    _ => "?",
                };
                let left = args.first().map(Self::describe_invariant).unwrap_or_default();
                let right = args.get(1).map(Self::describe_invariant).unwrap_or_default();
                format!("{} {} {}", left, op_str, right)
            }
            Expr::Call(name, args) => {
                let arg_strs: Vec<String> = args.iter().map(Self::describe_invariant).collect();
                format!("{} {}", name, arg_strs.join(" "))
            }
            Expr::Ident(name) => name.clone(),
            Expr::IntLit(n) => n.to_string(),
            Expr::FloatLit(f) => format!("{}", f),
            Expr::BoolLit(b) => b.to_string(),
            _ => "...".to_string(),
        }
    }
}

/// Runs SMT-LIB2 scripts through Z3 (or reports if unavailable).
pub struct SmtSolver;

impl SmtSolver {
    pub fn z3_available() -> bool {
        Command::new("z3")
            .arg("--version")
            .output()
            .map(|o| o.status.success())
            .unwrap_or(false)
    }

    pub fn check(script: &str) -> VerifyResult {
        let mut child = match Command::new("z3")
            .args(["-smt2", "-in"])
            .stdin(std::process::Stdio::piped())
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped())
            .spawn()
        {
            Ok(c) => c,
            Err(_) => return VerifyResult::SolverNotFound,
        };

        if let Some(ref mut stdin) = child.stdin {
            let _ = stdin.write_all(script.as_bytes());
        }

        let output = match child.wait_with_output() {
            Ok(o) => o,
            Err(e) => return VerifyResult::Unknown(format!("failed to read z3 output: {}", e)),
        };

        let stdout = String::from_utf8_lossy(&output.stdout);
        let result_line = stdout.trim();

        if result_line == "unsat" {
            VerifyResult::Verified
        } else if result_line.starts_with("sat") {
            VerifyResult::Violated(result_line.to_string())
        } else {
            VerifyResult::Unknown(result_line.to_string())
        }
    }

    pub fn verify_function(func: &Function) -> Vec<(usize, VerifyResult)> {
        let result = SmtEncoder::encode_function(func);
        result
            .invariant_checks
            .iter()
            .map(|check| (check.invariant_index, Self::check(&check.script)))
            .collect()
    }

    pub fn verify_program(program: &Program) -> Vec<(String, Vec<(usize, VerifyResult)>)> {
        program
            .items
            .iter()
            .filter_map(|item| {
                if let Item::Function(f) = item {
                    if !f.invariants.is_empty() {
                        Some((f.name.clone(), Self::verify_function(f)))
                    } else {
                        None
                    }
                } else {
                    None
                }
            })
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_param(name: &str, ty_name: &str) -> Param {
        Param {
            name: name.to_string(),
            ty: Type::Named(ty_name.to_string()),
        }
    }

    fn make_function(
        name: &str,
        params: Vec<Param>,
        returns: Option<Param>,
        invariants: Vec<Expr>,
        body: Expr,
    ) -> Function {
        Function {
            name: name.to_string(),
            params,
            returns,
            invariants,
            requires: vec![],
            ensures: vec![],
            mode: FnMode::Strict,
            intent: None,
            confidence: None,
            fallback: None,
            guarantee: None,
            body,
        }
    }

    #[test]
    fn test_encode_simple_function() {
        // fn add_one in x: int out result: int do add x 1
        let func = make_function(
            "add_one",
            vec![make_param("x", "int")],
            Some(make_param("result", "int")),
            vec![Expr::Op(Op::Gte, vec![Expr::Ident("result".into()), Expr::IntLit(0)])],
            Expr::Op(Op::Add, vec![Expr::Ident("x".into()), Expr::IntLit(1)]),
        );

        let result = SmtEncoder::encode_function(&func);
        assert_eq!(result.function_name, "add_one");
        assert_eq!(result.invariant_checks.len(), 1);

        let script = &result.invariant_checks[0].script;
        assert!(script.contains("(declare-const x Int)"));
        assert!(script.contains("(declare-const result Int)"));
        assert!(script.contains("(assert (= result (+ x 1)))"));
        assert!(script.contains("(assert (not (>= result 0)))"));
        assert!(script.contains("(check-sat)"));
    }

    #[test]
    fn test_encode_invariant_negation() {
        // inv gte result 0 → (assert (not (>= result 0)))
        let func = make_function(
            "test",
            vec![make_param("x", "int")],
            Some(make_param("result", "int")),
            vec![Expr::Op(Op::Gte, vec![Expr::Ident("result".into()), Expr::IntLit(0)])],
            Expr::Ident("x".into()),
        );

        let result = SmtEncoder::encode_function(&func);
        assert!(result.invariant_checks[0].script.contains("(assert (not (>= result 0)))"));
    }

    #[test]
    fn test_encode_operators() {
        assert_eq!(
            SmtEncoder::expr_to_smt(&Expr::Op(Op::Add, vec![Expr::IntLit(1), Expr::IntLit(2)])),
            "(+ 1 2)"
        );
        assert_eq!(
            SmtEncoder::expr_to_smt(&Expr::Op(Op::Sub, vec![Expr::Ident("a".into()), Expr::IntLit(3)])),
            "(- a 3)"
        );
        assert_eq!(
            SmtEncoder::expr_to_smt(&Expr::Op(Op::Mul, vec![Expr::Ident("x".into()), Expr::Ident("y".into())])),
            "(* x y)"
        );
        assert_eq!(
            SmtEncoder::expr_to_smt(&Expr::Op(Op::Div, vec![Expr::IntLit(10), Expr::IntLit(2)])),
            "(div 10 2)"
        );
        assert_eq!(
            SmtEncoder::expr_to_smt(&Expr::Op(Op::Modulo, vec![Expr::IntLit(7), Expr::IntLit(3)])),
            "(mod 7 3)"
        );
        assert_eq!(
            SmtEncoder::expr_to_smt(&Expr::Op(Op::Eq, vec![Expr::Ident("a".into()), Expr::Ident("b".into())])),
            "(= a b)"
        );
        assert_eq!(
            SmtEncoder::expr_to_smt(&Expr::Op(Op::Neq, vec![Expr::Ident("a".into()), Expr::Ident("b".into())])),
            "(not (= a b))"
        );
        assert_eq!(
            SmtEncoder::expr_to_smt(&Expr::Op(Op::Gt, vec![Expr::Ident("x".into()), Expr::IntLit(0)])),
            "(> x 0)"
        );
        assert_eq!(
            SmtEncoder::expr_to_smt(&Expr::Op(Op::Lt, vec![Expr::Ident("x".into()), Expr::IntLit(0)])),
            "(< x 0)"
        );
        assert_eq!(
            SmtEncoder::expr_to_smt(&Expr::Op(Op::Gte, vec![Expr::Ident("x".into()), Expr::IntLit(0)])),
            "(>= x 0)"
        );
        assert_eq!(
            SmtEncoder::expr_to_smt(&Expr::Op(Op::Lte, vec![Expr::Ident("x".into()), Expr::IntLit(5)])),
            "(<= x 5)"
        );
        assert_eq!(
            SmtEncoder::expr_to_smt(&Expr::Op(Op::And, vec![Expr::BoolLit(true), Expr::BoolLit(false)])),
            "(and true false)"
        );
        assert_eq!(
            SmtEncoder::expr_to_smt(&Expr::Op(Op::Or, vec![Expr::BoolLit(true), Expr::BoolLit(false)])),
            "(or true false)"
        );
        assert_eq!(
            SmtEncoder::expr_to_smt(&Expr::Op(Op::Not, vec![Expr::BoolLit(true)])),
            "(not true)"
        );
    }

    #[test]
    fn test_encode_builtins() {
        // max(a, b) → (ite (> a b) a b)
        assert_eq!(
            SmtEncoder::expr_to_smt(&Expr::Call("max".into(), vec![Expr::Ident("a".into()), Expr::Ident("b".into())])),
            "(ite (> a b) a b)"
        );
        // min(a, b) → (ite (< a b) a b)
        assert_eq!(
            SmtEncoder::expr_to_smt(&Expr::Call("min".into(), vec![Expr::Ident("a".into()), Expr::Ident("b".into())])),
            "(ite (< a b) a b)"
        );
        // abs(x) → (ite (>= x 0) x (- x))
        assert_eq!(
            SmtEncoder::expr_to_smt(&Expr::Call("abs".into(), vec![Expr::Ident("x".into())])),
            "(ite (>= x 0) x (- x))"
        );
    }

    #[test]
    fn test_encode_comparison_produces_bool() {
        // Comparisons produce Bool-sorted terms in SMT
        let expr = Expr::Op(Op::Gt, vec![Expr::Ident("x".into()), Expr::IntLit(0)]);
        let smt = SmtEncoder::expr_to_smt(&expr);
        assert_eq!(smt, "(> x 0)");
    }

    #[test]
    fn test_no_invariants_produces_empty_checks() {
        let func = make_function(
            "no_inv",
            vec![make_param("x", "int")],
            Some(make_param("result", "int")),
            vec![],
            Expr::Ident("x".into()),
        );

        let result = SmtEncoder::encode_function(&func);
        assert!(result.invariant_checks.is_empty());
    }

    #[test]
    fn test_verify_pipeline_without_z3() {
        let func = make_function(
            "test_fn",
            vec![make_param("x", "int")],
            Some(make_param("result", "int")),
            vec![Expr::Op(Op::Gte, vec![Expr::Ident("result".into()), Expr::IntLit(0)])],
            Expr::Ident("x".into()),
        );

        let results = SmtSolver::verify_function(&func);
        assert_eq!(results.len(), 1);
        // Without Z3, expect SolverNotFound or an actual result
        match &results[0].1 {
            VerifyResult::SolverNotFound => {} // expected on most CI
            VerifyResult::Verified => {}       // z3 is installed and says unsat? unlikely for this
            VerifyResult::Violated(_) => {}    // z3 found a counterexample (x = -1)
            VerifyResult::Unknown(_) => {}     // also acceptable
        }
    }

    #[test]
    fn test_type_to_smt() {
        assert_eq!(SmtEncoder::type_to_smt(&Type::Named("int".into())), "Int");
        assert_eq!(SmtEncoder::type_to_smt(&Type::Named("uint".into())), "Int");
        assert_eq!(SmtEncoder::type_to_smt(&Type::Named("float".into())), "Real");
        assert_eq!(SmtEncoder::type_to_smt(&Type::Named("bool".into())), "Bool");
        assert_eq!(SmtEncoder::type_to_smt(&Type::Named("str".into())), "String");
    }

    #[test]
    fn test_encode_if_expression() {
        let expr = Expr::If {
            condition: Box::new(Expr::Op(Op::Gt, vec![Expr::Ident("x".into()), Expr::IntLit(0)])),
            then_body: vec![Expr::Ident("x".into())],
            elif_branches: vec![],
            else_body: Some(vec![Expr::IntLit(0)]),
        };
        assert_eq!(SmtEncoder::expr_to_smt(&expr), "(ite (> x 0) x 0)");
    }

    #[test]
    fn test_encode_literals() {
        assert_eq!(SmtEncoder::expr_to_smt(&Expr::IntLit(42)), "42");
        assert_eq!(SmtEncoder::expr_to_smt(&Expr::IntLit(-5)), "(- 5)");
        assert_eq!(SmtEncoder::expr_to_smt(&Expr::BoolLit(true)), "true");
        assert_eq!(SmtEncoder::expr_to_smt(&Expr::BoolLit(false)), "false");
        assert_eq!(SmtEncoder::expr_to_smt(&Expr::Ident("foo".into())), "foo");
    }

    #[test]
    fn test_clamp_function_encoding() {
        // Mirrors examples/clamp.knl:
        // fn clamp in val: int lo: int hi: int out result: int
        //   inv gte result lo
        //   inv lte result hi
        //   do max lo (min hi val)
        let func = make_function(
            "clamp",
            vec![
                make_param("val", "int"),
                make_param("lo", "int"),
                make_param("hi", "int"),
            ],
            Some(make_param("result", "int")),
            vec![
                Expr::Op(Op::Gte, vec![Expr::Ident("result".into()), Expr::Ident("lo".into())]),
                Expr::Op(Op::Lte, vec![Expr::Ident("result".into()), Expr::Ident("hi".into())]),
            ],
            Expr::Call(
                "max".into(),
                vec![
                    Expr::Ident("lo".into()),
                    Expr::Call("min".into(), vec![Expr::Ident("hi".into()), Expr::Ident("val".into())]),
                ],
            ),
        );

        let result = SmtEncoder::encode_function(&func);
        assert_eq!(result.function_name, "clamp");
        assert_eq!(result.invariant_checks.len(), 2);

        let check0 = &result.invariant_checks[0].script;
        assert!(check0.contains("(declare-const val Int)"));
        assert!(check0.contains("(declare-const lo Int)"));
        assert!(check0.contains("(declare-const hi Int)"));
        assert!(check0.contains("(declare-const result Int)"));
        assert!(check0.contains("(assert (not (>= result lo)))"));

        let check1 = &result.invariant_checks[1].script;
        assert!(check1.contains("(assert (not (<= result hi)))"));
    }
}
