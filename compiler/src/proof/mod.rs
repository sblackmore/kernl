use crate::parser::ast::*;

pub struct LeanExporter;

impl LeanExporter {
    pub fn export_program(program: &Program) -> String {
        let mut out = String::new();
        out.push_str("-- Auto-generated from kernl\n");
        out.push_str("-- Proof obligations for invariants and contracts\n\n");

        for item in &program.items {
            match item {
                Item::Enum(e) => {
                    out.push_str(&Self::export_enum(e));
                    out.push('\n');
                }
                Item::Function(f)
                    if !f.invariants.is_empty()
                        || !f.requires.is_empty()
                        || !f.ensures.is_empty() =>
                {
                    out.push_str(&Self::export_function(f));
                    out.push('\n');
                }
                _ => {}
            }
        }
        out
    }

    pub fn export_enum(e: &EnumDef) -> String {
        let mut out = String::new();
        out.push_str(&format!("inductive {}\n", e.name));
        for v in &e.variants {
            out.push_str("  | ");
            out.push_str(&v.name);
            for (i, field_ty) in v.fields.iter().enumerate() {
                let ty = Self::type_to_lean(field_ty);
                out.push_str(&format!(" (x{i} : {ty})"));
            }
            out.push('\n');
        }
        out.push_str("deriving Repr, DecidableEq\n");
        out
    }

    pub fn export_function(func: &Function) -> String {
        let mut out = String::new();

        let params_str: Vec<String> = func
            .params
            .iter()
            .map(|p| format!("({} : {})", p.name, Self::type_to_lean(&p.ty)))
            .collect();
        let params_decl = params_str.join(" ");

        let body_lean = Self::expr_to_lean(&func.body);

        let ret_name = func
            .returns
            .as_ref()
            .map(|r| r.name.clone())
            .unwrap_or_else(|| "result".into());

        for (i, inv) in func.invariants.iter().enumerate() {
            let inv_lean = Self::expr_to_lean(inv);
            out.push_str(&format!(
                "theorem {}_inv_{i} {params_decl} :\n",
                func.name
            ));
            out.push_str(&format!("    let {ret_name} := {body_lean}\n"));
            out.push_str(&format!("    {inv_lean} := by\n"));
            out.push_str("  sorry\n\n");
        }

        for (i, req) in func.requires.iter().enumerate() {
            let req_lean = Self::expr_to_lean(req);
            out.push_str(&format!(
                "theorem {}_req_{i} {params_decl} :\n",
                func.name
            ));
            out.push_str(&format!("    {req_lean} := by\n"));
            out.push_str("  sorry\n\n");
        }

        for (i, ens) in func.ensures.iter().enumerate() {
            let ens_lean = Self::expr_to_lean(ens);
            out.push_str(&format!(
                "theorem {}_ens_{i} {params_decl} :\n",
                func.name
            ));
            out.push_str(&format!("    let {ret_name} := {body_lean}\n"));
            out.push_str(&format!("    {ens_lean} := by\n"));
            out.push_str("  sorry\n\n");
        }

        out
    }

    pub fn type_to_lean(ty: &Type) -> String {
        match ty {
            Type::Named(n) => match n.as_str() {
                "int" | "uint" => "Int".into(),
                "float" => "Float".into(),
                "bool" => "Bool".into(),
                "str" => "String".into(),
                "void" => "Unit".into(),
                other => other.to_string(),
            },
            Type::List(inner) => format!("List {}", Self::type_to_lean(inner)),
            Type::Optional(inner) => format!("Option {}", Self::type_to_lean(inner)),
            Type::Map(k, v) => format!("({} → {})", Self::type_to_lean(k), Self::type_to_lean(v)),
            Type::Tuple(elems) => {
                let parts: Vec<String> = elems.iter().map(|t| Self::type_to_lean(t)).collect();
                format!("({})", parts.join(" × "))
            }
        }
    }

    pub fn expr_to_lean(expr: &Expr) -> String {
        match expr {
            Expr::IntLit(n) => {
                if *n < 0 {
                    format!("({n})")
                } else {
                    format!("{n}")
                }
            }
            Expr::FloatLit(n) => format!("{n}"),
            Expr::BoolLit(true) => "True".into(),
            Expr::BoolLit(false) => "False".into(),
            Expr::StrLit(s) => format!("\"{s}\""),
            Expr::Ident(name) => name.clone(),
            Expr::Op(op, args) => {
                if args.len() == 1 {
                    let a = Self::expr_to_lean(&args[0]);
                    match op {
                        Op::Not => format!("(¬{a})"),
                        _ => format!("(sorry)"),
                    }
                } else if args.len() == 2 {
                    let l = Self::expr_to_lean(&args[0]);
                    let r = Self::expr_to_lean(&args[1]);
                    match op {
                        Op::Add => format!("({l} + {r})"),
                        Op::Sub => format!("({l} - {r})"),
                        Op::Mul => format!("({l} * {r})"),
                        Op::Div => format!("({l} / {r})"),
                        Op::Modulo => format!("({l} % {r})"),
                        Op::Eq => format!("({l} = {r})"),
                        Op::Neq => format!("({l} ≠ {r})"),
                        Op::Gt => format!("({l} > {r})"),
                        Op::Lt => format!("({l} < {r})"),
                        Op::Gte => format!("({l} ≥ {r})"),
                        Op::Lte => format!("({l} ≤ {r})"),
                        Op::And => format!("({l} ∧ {r})"),
                        Op::Or => format!("({l} ∨ {r})"),
                        Op::Not => format!("(¬{l})"),
                    }
                } else {
                    "sorry".into()
                }
            }
            Expr::Call(name, args) => {
                let arg_strs: Vec<String> = args.iter().map(|a| Self::expr_to_lean(a)).collect();
                match name.as_str() {
                    "max" => format!("(max {})", arg_strs.join(" ")),
                    "min" => format!("(min {})", arg_strs.join(" ")),
                    "abs" => format!("(Int.natAbs {})", arg_strs.join(" ")),
                    _ => {
                        if arg_strs.is_empty() {
                            name.clone()
                        } else {
                            format!("({} {})", name, arg_strs.join(" "))
                        }
                    }
                }
            }
            Expr::If {
                condition,
                then_body,
                else_body,
                ..
            } => {
                let cond = Self::expr_to_lean(condition);
                let then_expr = then_body
                    .last()
                    .map(|e| Self::expr_to_lean(e))
                    .unwrap_or_else(|| "sorry".into());
                let else_expr = else_body
                    .as_ref()
                    .and_then(|b| b.last())
                    .map(|e| Self::expr_to_lean(e))
                    .unwrap_or_else(|| "sorry".into());
                format!("(if {cond} then {then_expr} else {else_expr})")
            }
            Expr::Pipe(left, right) => {
                let l = Self::expr_to_lean(left);
                let r = Self::expr_to_lean(right);
                format!("({r} {l})")
            }
            Expr::Let { name, value, .. } => {
                let v = Self::expr_to_lean(value);
                format!("(let {name} := {v})")
            }
            Expr::Block(exprs) => {
                if let Some(last) = exprs.last() {
                    Self::expr_to_lean(last)
                } else {
                    "sorry".into()
                }
            }
            _ => "sorry".into(),
        }
    }
}

pub struct CoqExporter;

impl CoqExporter {
    pub fn export_program(program: &Program) -> String {
        let mut out = String::new();
        out.push_str("(* Auto-generated from kernl *)\n");
        out.push_str("(* Proof obligations for invariants and contracts *)\n");
        out.push_str("Require Import ZArith.\nOpen Scope Z_scope.\n\n");

        for item in &program.items {
            match item {
                Item::Enum(e) => {
                    out.push_str(&Self::export_enum(e));
                    out.push('\n');
                }
                Item::Function(f)
                    if !f.invariants.is_empty()
                        || !f.requires.is_empty()
                        || !f.ensures.is_empty() =>
                {
                    out.push_str(&Self::export_function(f));
                    out.push('\n');
                }
                _ => {}
            }
        }
        out
    }

    pub fn export_enum(e: &EnumDef) -> String {
        let mut out = String::new();
        if e.variants.is_empty() {
            out.push_str(&format!("(* enum {} has no variants *)\n", e.name));
            return out;
        }
        out.push_str(&format!("Inductive {} :=\n", e.name));
        let n = e.variants.len();
        for (vi, v) in e.variants.iter().enumerate() {
            out.push_str("  | ");
            out.push_str(&v.name);
            for (i, field_ty) in v.fields.iter().enumerate() {
                let ty = Self::type_to_coq(field_ty);
                out.push_str(&format!(" (x{i} : {ty})"));
            }
            if vi + 1 == n {
                out.push_str(".\n");
            } else {
                out.push('\n');
            }
        }
        out
    }

    pub fn export_function(func: &Function) -> String {
        let mut out = String::new();

        let params_str: Vec<String> = func
            .params
            .iter()
            .map(|p| format!("({} : {})", p.name, Self::type_to_coq(&p.ty)))
            .collect();
        let params_decl = if params_str.is_empty() {
            String::new()
        } else {
            format!("forall {}, ", params_str.join(" "))
        };

        let body_coq = Self::expr_to_coq(&func.body);

        let ret_name = func
            .returns
            .as_ref()
            .map(|r| r.name.clone())
            .unwrap_or_else(|| "result".into());

        for (i, inv) in func.invariants.iter().enumerate() {
            let inv_coq = Self::expr_to_coq(inv);
            out.push_str(&format!(
                "Theorem {}_inv_{i} : {params_decl}\n",
                func.name
            ));
            out.push_str(&format!("  let {ret_name} := {body_coq} in\n"));
            out.push_str(&format!("  {inv_coq}.\n"));
            out.push_str("Proof. Admitted.\n\n");
        }

        for (i, req) in func.requires.iter().enumerate() {
            let req_coq = Self::expr_to_coq(req);
            out.push_str(&format!(
                "Theorem {}_req_{i} : {params_decl}\n",
                func.name
            ));
            out.push_str(&format!("  {req_coq}.\n"));
            out.push_str("Proof. Admitted.\n\n");
        }

        for (i, ens) in func.ensures.iter().enumerate() {
            let ens_coq = Self::expr_to_coq(ens);
            out.push_str(&format!(
                "Theorem {}_ens_{i} : {params_decl}\n",
                func.name
            ));
            out.push_str(&format!("  let {ret_name} := {body_coq} in\n"));
            out.push_str(&format!("  {ens_coq}.\n"));
            out.push_str("Proof. Admitted.\n\n");
        }

        out
    }

    pub fn type_to_coq(ty: &Type) -> String {
        match ty {
            Type::Named(n) => match n.as_str() {
                "int" | "uint" => "Z".into(),
                "float" => "R".into(),
                "bool" => "bool".into(),
                "str" => "string".into(),
                "void" => "unit".into(),
                other => other.to_string(),
            },
            Type::List(inner) => format!("list {}", Self::type_to_coq(inner)),
            Type::Optional(inner) => format!("option {}", Self::type_to_coq(inner)),
            Type::Map(k, v) => format!("({} -> {})", Self::type_to_coq(k), Self::type_to_coq(v)),
            Type::Tuple(elems) => {
                let parts: Vec<String> = elems.iter().map(|t| Self::type_to_coq(t)).collect();
                format!("({})", parts.join(" * "))
            }
        }
    }

    pub fn expr_to_coq(expr: &Expr) -> String {
        match expr {
            Expr::IntLit(n) => {
                if *n < 0 {
                    format!("({})", n)
                } else {
                    format!("{n}")
                }
            }
            Expr::FloatLit(n) => format!("{n}"),
            Expr::BoolLit(true) => "true".into(),
            Expr::BoolLit(false) => "false".into(),
            Expr::StrLit(s) => format!("\"{s}\""),
            Expr::Ident(name) => name.clone(),
            Expr::Op(op, args) => {
                if args.len() == 1 {
                    let a = Self::expr_to_coq(&args[0]);
                    match op {
                        Op::Not => format!("(negb {a})"),
                        _ => format!("(* unsupported *)"),
                    }
                } else if args.len() == 2 {
                    let l = Self::expr_to_coq(&args[0]);
                    let r = Self::expr_to_coq(&args[1]);
                    match op {
                        Op::Add => format!("({l} + {r})"),
                        Op::Sub => format!("({l} - {r})"),
                        Op::Mul => format!("({l} * {r})"),
                        Op::Div => format!("({l} / {r})"),
                        Op::Modulo => format!("(Z.modulo {l} {r})"),
                        Op::Eq => format!("({l} =? {r})"),
                        Op::Neq => format!("(negb ({l} =? {r}))"),
                        Op::Gt => format!("({l} >? {r})"),
                        Op::Lt => format!("({l} <? {r})"),
                        Op::Gte => format!("({l} >=? {r})"),
                        Op::Lte => format!("({l} <=? {r})"),
                        Op::And => format!("(andb {l} {r})"),
                        Op::Or => format!("(orb {l} {r})"),
                        Op::Not => format!("(negb {l})"),
                    }
                } else {
                    "(* unsupported *)".into()
                }
            }
            Expr::Call(name, args) => {
                let arg_strs: Vec<String> = args.iter().map(|a| Self::expr_to_coq(a)).collect();
                match name.as_str() {
                    "max" => format!("(Z.max {})", arg_strs.join(" ")),
                    "min" => format!("(Z.min {})", arg_strs.join(" ")),
                    "abs" => format!("(Z.abs {})", arg_strs.join(" ")),
                    _ => {
                        if arg_strs.is_empty() {
                            name.clone()
                        } else {
                            format!("({} {})", name, arg_strs.join(" "))
                        }
                    }
                }
            }
            Expr::If {
                condition,
                then_body,
                else_body,
                ..
            } => {
                let cond = Self::expr_to_coq(condition);
                let then_expr = then_body
                    .last()
                    .map(|e| Self::expr_to_coq(e))
                    .unwrap_or_else(|| "(* empty *)".into());
                let else_expr = else_body
                    .as_ref()
                    .and_then(|b| b.last())
                    .map(|e| Self::expr_to_coq(e))
                    .unwrap_or_else(|| "(* empty *)".into());
                format!("(if {cond} then {then_expr} else {else_expr})")
            }
            Expr::Pipe(left, right) => {
                let l = Self::expr_to_coq(left);
                let r = Self::expr_to_coq(right);
                format!("({r} {l})")
            }
            Expr::Let { name, value, .. } => {
                let v = Self::expr_to_coq(value);
                format!("(let {name} := {v})")
            }
            Expr::Block(exprs) => {
                if let Some(last) = exprs.last() {
                    Self::expr_to_coq(last)
                } else {
                    "(* empty *)".into()
                }
            }
            _ => "(* unsupported *)".into(),
        }
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
    fn lean_simple_invariant() {
        let func = make_function(
            "add_one",
            vec![make_param("x", "int")],
            Some(make_param("result", "int")),
            vec![Expr::Op(
                Op::Gte,
                vec![Expr::Ident("result".into()), Expr::IntLit(0)],
            )],
            vec![],
            vec![],
            Expr::Op(Op::Add, vec![Expr::Ident("x".into()), Expr::IntLit(1)]),
        );

        let output = LeanExporter::export_function(&func);
        assert!(output.contains("theorem add_one_inv_0"));
        assert!(output.contains("(x : Int)"));
        assert!(output.contains("let result :="));
        assert!(output.contains("(result ≥ 0)"));
        assert!(output.contains("sorry"));
    }

    #[test]
    fn lean_function_with_contracts() {
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
            Expr::Op(Op::Div, vec![Expr::Ident("a".into()), Expr::Ident("b".into())]),
        );

        let output = LeanExporter::export_function(&func);
        assert!(output.contains("theorem safe_div_req_0"));
        assert!(output.contains("(b ≠ 0)"));
        assert!(output.contains("theorem safe_div_ens_0"));
    }

    #[test]
    fn lean_type_mapping() {
        assert_eq!(LeanExporter::type_to_lean(&Type::Named("int".into())), "Int");
        assert_eq!(LeanExporter::type_to_lean(&Type::Named("float".into())), "Float");
        assert_eq!(LeanExporter::type_to_lean(&Type::Named("bool".into())), "Bool");
        assert_eq!(LeanExporter::type_to_lean(&Type::Named("str".into())), "String");
        assert_eq!(LeanExporter::type_to_lean(&Type::Named("void".into())), "Unit");
        assert_eq!(
            LeanExporter::type_to_lean(&Type::List(Box::new(Type::Named("int".into())))),
            "List Int"
        );
        assert_eq!(
            LeanExporter::type_to_lean(&Type::Optional(Box::new(Type::Named("int".into())))),
            "Option Int"
        );
    }

    #[test]
    fn lean_operator_mapping() {
        assert_eq!(
            LeanExporter::expr_to_lean(&Expr::Op(
                Op::Add,
                vec![Expr::Ident("a".into()), Expr::Ident("b".into())]
            )),
            "(a + b)"
        );
        assert_eq!(
            LeanExporter::expr_to_lean(&Expr::Op(
                Op::Gte,
                vec![Expr::Ident("x".into()), Expr::IntLit(0)]
            )),
            "(x ≥ 0)"
        );
        assert_eq!(
            LeanExporter::expr_to_lean(&Expr::Op(
                Op::And,
                vec![Expr::BoolLit(true), Expr::BoolLit(false)]
            )),
            "(True ∧ False)"
        );
        assert_eq!(
            LeanExporter::expr_to_lean(&Expr::Op(Op::Not, vec![Expr::BoolLit(true)])),
            "(¬True)"
        );
    }

    #[test]
    fn coq_simple_invariant() {
        let func = make_function(
            "clamp",
            vec![
                make_param("val", "int"),
                make_param("lo", "int"),
                make_param("hi", "int"),
            ],
            Some(make_param("result", "int")),
            vec![Expr::Op(
                Op::Gte,
                vec![Expr::Ident("result".into()), Expr::Ident("lo".into())],
            )],
            vec![],
            vec![],
            Expr::Call(
                "max".into(),
                vec![
                    Expr::Ident("lo".into()),
                    Expr::Call(
                        "min".into(),
                        vec![Expr::Ident("hi".into()), Expr::Ident("val".into())],
                    ),
                ],
            ),
        );

        let output = CoqExporter::export_function(&func);
        assert!(output.contains("Theorem clamp_inv_0"));
        assert!(output.contains("forall"));
        assert!(output.contains("(val : Z)"));
        assert!(output.contains("Z.max"));
        assert!(output.contains("Admitted"));
    }

    #[test]
    fn coq_type_mapping() {
        assert_eq!(CoqExporter::type_to_coq(&Type::Named("int".into())), "Z");
        assert_eq!(CoqExporter::type_to_coq(&Type::Named("float".into())), "R");
        assert_eq!(CoqExporter::type_to_coq(&Type::Named("bool".into())), "bool");
        assert_eq!(CoqExporter::type_to_coq(&Type::Named("str".into())), "string");
        assert_eq!(
            CoqExporter::type_to_coq(&Type::List(Box::new(Type::Named("int".into())))),
            "list Z"
        );
    }

    #[test]
    fn coq_operator_mapping() {
        assert_eq!(
            CoqExporter::expr_to_coq(&Expr::Op(
                Op::Add,
                vec![Expr::Ident("a".into()), Expr::Ident("b".into())]
            )),
            "(a + b)"
        );
        assert_eq!(
            CoqExporter::expr_to_coq(&Expr::Op(
                Op::Gte,
                vec![Expr::Ident("x".into()), Expr::IntLit(0)]
            )),
            "(x >=? 0)"
        );
        assert_eq!(
            CoqExporter::expr_to_coq(&Expr::Op(
                Op::Modulo,
                vec![Expr::Ident("a".into()), Expr::Ident("b".into())]
            )),
            "(Z.modulo a b)"
        );
    }

    #[test]
    fn no_invariants_empty_output() {
        let func = make_function(
            "noop",
            vec![make_param("x", "int")],
            None,
            vec![],
            vec![],
            vec![],
            Expr::Ident("x".into()),
        );

        let lean = LeanExporter::export_function(&func);
        assert!(lean.is_empty());

        let coq = CoqExporter::export_function(&func);
        assert!(coq.is_empty());
    }

    #[test]
    fn lean_export_program_filters() {
        let program = Program {
            items: vec![
                Item::Function(make_function(
                    "with_inv",
                    vec![make_param("x", "int")],
                    Some(make_param("result", "int")),
                    vec![Expr::Op(
                        Op::Gte,
                        vec![Expr::Ident("result".into()), Expr::IntLit(0)],
                    )],
                    vec![],
                    vec![],
                    Expr::Ident("x".into()),
                )),
                Item::Function(make_function(
                    "without_inv",
                    vec![make_param("x", "int")],
                    None,
                    vec![],
                    vec![],
                    vec![],
                    Expr::Ident("x".into()),
                )),
            ],
        };

        let output = LeanExporter::export_program(&program);
        assert!(output.contains("with_inv_inv_0"));
        assert!(!output.contains("without_inv"));
    }

    #[test]
    fn coq_export_program_includes_header() {
        let program = Program {
            items: vec![Item::Function(make_function(
                "test",
                vec![make_param("x", "int")],
                Some(make_param("r", "int")),
                vec![Expr::Op(
                    Op::Eq,
                    vec![Expr::Ident("r".into()), Expr::Ident("x".into())],
                )],
                vec![],
                vec![],
                Expr::Ident("x".into()),
            ))],
        };

        let output = CoqExporter::export_program(&program);
        assert!(output.contains("Require Import ZArith"));
        assert!(output.contains("Open Scope Z_scope"));
        assert!(output.contains("test_inv_0"));
    }

    #[test]
    fn lean_call_expressions() {
        assert_eq!(
            LeanExporter::expr_to_lean(&Expr::Call(
                "max".into(),
                vec![Expr::Ident("a".into()), Expr::Ident("b".into())]
            )),
            "(max a b)"
        );
        assert_eq!(
            LeanExporter::expr_to_lean(&Expr::Call(
                "min".into(),
                vec![Expr::Ident("a".into()), Expr::Ident("b".into())]
            )),
            "(min a b)"
        );
        assert_eq!(
            LeanExporter::expr_to_lean(&Expr::Call(
                "abs".into(),
                vec![Expr::Ident("x".into())]
            )),
            "(Int.natAbs x)"
        );
    }

    #[test]
    fn coq_call_expressions() {
        assert_eq!(
            CoqExporter::expr_to_coq(&Expr::Call(
                "max".into(),
                vec![Expr::Ident("a".into()), Expr::Ident("b".into())]
            )),
            "(Z.max a b)"
        );
        assert_eq!(
            CoqExporter::expr_to_coq(&Expr::Call(
                "abs".into(),
                vec![Expr::Ident("x".into())]
            )),
            "(Z.abs x)"
        );
    }

    #[test]
    fn lean_export_enum_inductive() {
        let program = Program {
            items: vec![Item::Enum(EnumDef {
                name: "Opt".into(),
                variants: vec![
                    Variant {
                        name: "None".into(),
                        fields: vec![],
                    },
                    Variant {
                        name: "Some".into(),
                        fields: vec![Type::Named("int".into())],
                    },
                ],
            })],
        };
        let output = LeanExporter::export_program(&program);
        assert!(output.contains("inductive Opt"));
        assert!(output.contains("| None"));
        assert!(output.contains("| Some"));
        assert!(output.contains("(x0 : Int)"));
        assert!(output.contains("deriving Repr"));
    }

    #[test]
    fn coq_export_enum_inductive() {
        let program = Program {
            items: vec![Item::Enum(EnumDef {
                name: "Result".into(),
                variants: vec![
                    Variant {
                        name: "Ok".into(),
                        fields: vec![Type::Named("int".into())],
                    },
                    Variant {
                        name: "Err".into(),
                        fields: vec![Type::Named("str".into())],
                    },
                ],
            })],
        };
        let output = CoqExporter::export_program(&program);
        assert!(output.contains("Inductive Result"));
        assert!(output.contains("| Ok"));
        assert!(output.contains("| Err"));
        assert!(output.contains("(x0 : Z)"));
        assert!(output.contains("(x0 : string)."));
    }
}
