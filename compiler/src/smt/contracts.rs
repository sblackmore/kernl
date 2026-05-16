use crate::parser::ast::*;
use super::{SmtEncoder, SmtSolver, VerifyResult};

pub struct ContractChecker;

impl ContractChecker {
    /// Verify function contracts:
    /// 1. Preconditions are assumed (not checked -- they're the caller's responsibility)
    /// 2. Given preconditions hold, verify:
    ///    a. All invariants hold
    ///    b. All postconditions hold after the body executes
    pub fn encode_function(func: &Function) -> ContractResult {
        let mut script = String::new();
        script.push_str("(set-logic ALL)\n\n");

        for param in &func.params {
            script.push_str(&format!(
                "(declare-const {} {})\n",
                param.name,
                SmtEncoder::type_to_smt(&param.ty)
            ));
        }

        if let Some(ref ret) = func.returns {
            script.push_str(&format!(
                "(declare-const {} {})\n",
                ret.name,
                SmtEncoder::type_to_smt(&ret.ty)
            ));
        }

        script.push('\n');

        for req in &func.requires {
            script.push_str(&format!("(assert {})\n", SmtEncoder::expr_to_smt(req)));
        }

        if let Some(ref ret) = func.returns {
            script.push_str(&format!(
                "(assert (= {} {}))\n",
                ret.name,
                SmtEncoder::expr_to_smt(&func.body)
            ));
        }

        script.push('\n');

        let mut checks = Vec::new();

        for (i, ens) in func.ensures.iter().enumerate() {
            let mut check_script = script.clone();
            check_script.push_str(&format!(
                "(assert (not {}))\n",
                SmtEncoder::expr_to_smt(ens)
            ));
            check_script.push_str("(check-sat)\n");

            checks.push(ContractCheck {
                kind: ContractKind::Postcondition,
                index: i,
                script: check_script,
                description: SmtEncoder::describe_invariant(ens),
            });
        }

        for (i, inv) in func.invariants.iter().enumerate() {
            let mut check_script = script.clone();
            check_script.push_str(&format!(
                "(assert (not {}))\n",
                SmtEncoder::expr_to_smt(inv)
            ));
            check_script.push_str("(check-sat)\n");

            checks.push(ContractCheck {
                kind: ContractKind::Invariant,
                index: i,
                script: check_script,
                description: SmtEncoder::describe_invariant(inv),
            });
        }

        ContractResult {
            function_name: func.name.clone(),
            has_preconditions: !func.requires.is_empty(),
            checks,
        }
    }

    /// Verify all contracts in a program.
    pub fn verify_program(program: &Program) -> Vec<(String, Vec<(ContractCheck, VerifyResult)>)> {
        program
            .items
            .iter()
            .filter_map(|item| {
                if let Item::Function(f) = item {
                    if !f.requires.is_empty()
                        || !f.ensures.is_empty()
                        || !f.invariants.is_empty()
                    {
                        let result = Self::encode_function(f);
                        let checked: Vec<(ContractCheck, VerifyResult)> = result
                            .checks
                            .into_iter()
                            .map(|check| {
                                let vr = SmtSolver::check(&check.script);
                                (check, vr)
                            })
                            .collect();
                        Some((f.name.clone(), checked))
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

pub struct ContractResult {
    pub function_name: String,
    pub has_preconditions: bool,
    pub checks: Vec<ContractCheck>,
}

pub struct ContractCheck {
    pub kind: ContractKind,
    pub index: usize,
    pub script: String,
    pub description: String,
}

#[derive(Debug, PartialEq)]
pub enum ContractKind {
    Precondition,
    Postcondition,
    Invariant,
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
        requires: Vec<Expr>,
        ensures: Vec<Expr>,
        body: Expr,
    ) -> Function {
        Function {
            name: name.to_string(),
            params,
            returns,
            invariants,
            requires,
            ensures,
            mode: FnMode::Strict,
            intent: None,
            confidence: None,
            fallback: None,
            guarantee: None,
            body,
        }
    }

    #[test]
    fn encode_with_requires_ensures() {
        let func = make_function(
            "safe_div",
            vec![make_param("a", "int"), make_param("b", "int")],
            Some(make_param("result", "int")),
            vec![],
            vec![Expr::Op(
                Op::Neq,
                vec![Expr::Ident("b".into()), Expr::IntLit(0)],
            )],
            vec![Expr::Op(
                Op::Eq,
                vec![
                    Expr::Op(
                        Op::Mul,
                        vec![Expr::Ident("result".into()), Expr::Ident("b".into())],
                    ),
                    Expr::Ident("a".into()),
                ],
            )],
            Expr::Op(
                Op::Div,
                vec![Expr::Ident("a".into()), Expr::Ident("b".into())],
            ),
        );

        let result = ContractChecker::encode_function(&func);
        assert_eq!(result.function_name, "safe_div");
        assert!(result.has_preconditions);
        assert_eq!(result.checks.len(), 1);
        assert_eq!(result.checks[0].kind, ContractKind::Postcondition);

        let script = &result.checks[0].script;
        assert!(script.contains("(assert (not (= b 0)))"));
        assert!(script.contains("(assert (= result (div a b)))"));
        assert!(script.contains("(assert (not (= (* result b) a)))"));
        assert!(script.contains("(check-sat)"));
    }

    #[test]
    fn preconditions_asserted_as_assumptions() {
        let func = make_function(
            "test",
            vec![make_param("x", "int")],
            Some(make_param("result", "int")),
            vec![],
            vec![Expr::Op(
                Op::Gt,
                vec![Expr::Ident("x".into()), Expr::IntLit(0)],
            )],
            vec![Expr::Op(
                Op::Gt,
                vec![Expr::Ident("result".into()), Expr::IntLit(0)],
            )],
            Expr::Ident("x".into()),
        );

        let result = ContractChecker::encode_function(&func);
        let script = &result.checks[0].script;
        assert!(script.contains("(assert (> x 0))"));
    }

    #[test]
    fn postconditions_checked_negated() {
        let func = make_function(
            "test",
            vec![make_param("x", "int")],
            Some(make_param("result", "int")),
            vec![],
            vec![],
            vec![Expr::Op(
                Op::Gte,
                vec![Expr::Ident("result".into()), Expr::IntLit(0)],
            )],
            Expr::Ident("x".into()),
        );

        let result = ContractChecker::encode_function(&func);
        assert_eq!(result.checks.len(), 1);
        assert!(result.checks[0]
            .script
            .contains("(assert (not (>= result 0)))"));
    }

    #[test]
    fn no_contracts_produces_empty_checks() {
        let func = make_function(
            "simple",
            vec![make_param("x", "int")],
            Some(make_param("result", "int")),
            vec![],
            vec![],
            vec![],
            Expr::Ident("x".into()),
        );

        let result = ContractChecker::encode_function(&func);
        assert!(!result.has_preconditions);
        assert!(result.checks.is_empty());
    }

    #[test]
    fn invariants_checked_with_preconditions_assumed() {
        let func = make_function(
            "bounded",
            vec![make_param("x", "int")],
            Some(make_param("result", "int")),
            vec![Expr::Op(
                Op::Gte,
                vec![Expr::Ident("result".into()), Expr::IntLit(0)],
            )],
            vec![Expr::Op(
                Op::Gte,
                vec![Expr::Ident("x".into()), Expr::IntLit(0)],
            )],
            vec![],
            Expr::Ident("x".into()),
        );

        let result = ContractChecker::encode_function(&func);
        assert!(result.has_preconditions);
        assert_eq!(result.checks.len(), 1);
        assert_eq!(result.checks[0].kind, ContractKind::Invariant);

        let script = &result.checks[0].script;
        assert!(script.contains("(assert (>= x 0))"));
        assert!(script.contains("(assert (not (>= result 0)))"));
    }

    #[test]
    fn mixed_invariants_and_ensures() {
        let func = make_function(
            "mixed",
            vec![make_param("x", "int")],
            Some(make_param("result", "int")),
            vec![Expr::Op(
                Op::Gte,
                vec![Expr::Ident("result".into()), Expr::IntLit(0)],
            )],
            vec![],
            vec![Expr::Op(
                Op::Lte,
                vec![Expr::Ident("result".into()), Expr::IntLit(100)],
            )],
            Expr::Ident("x".into()),
        );

        let result = ContractChecker::encode_function(&func);
        assert_eq!(result.checks.len(), 2);
        assert_eq!(result.checks[0].kind, ContractKind::Postcondition);
        assert_eq!(result.checks[1].kind, ContractKind::Invariant);
    }

    #[test]
    fn verify_program_filters_functions_with_contracts() {
        let program = Program {
            items: vec![
                Item::Function(make_function(
                    "no_contracts",
                    vec![],
                    None,
                    vec![],
                    vec![],
                    vec![],
                    Expr::IntLit(0),
                )),
                Item::Function(make_function(
                    "has_ensures",
                    vec![make_param("x", "int")],
                    Some(make_param("result", "int")),
                    vec![],
                    vec![],
                    vec![Expr::Op(
                        Op::Gte,
                        vec![Expr::Ident("result".into()), Expr::IntLit(0)],
                    )],
                    Expr::Ident("x".into()),
                )),
            ],
        };

        let results = ContractChecker::verify_program(&program);
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].0, "has_ensures");
    }
}
