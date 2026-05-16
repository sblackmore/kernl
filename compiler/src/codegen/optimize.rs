use crate::parser::ast::*;

/// Fold constant expressions in-place across the entire program.
pub fn fold_constants(program: &mut Program) {
    for item in &mut program.items {
        if let Item::Function(f) = item {
            for inv in &mut f.invariants {
                fold_expr(inv);
            }
            if let Some(fb) = &mut f.fallback {
                fold_expr(fb);
            }
            fold_expr(&mut f.body);
        }
    }
}

/// Eliminate dead branches whose conditions are statically known.
pub fn dead_code_elimination(program: &mut Program) {
    for item in &mut program.items {
        if let Item::Function(f) = item {
            dce_expr(&mut f.body);
        }
    }
}

/// Map kernl builtins to LLVM intrinsic names where a direct mapping exists.
/// Returns `None` for builtins that need custom instruction sequences (max, min).
pub fn is_llvm_intrinsic(name: &str) -> Option<&'static str> {
    match name {
        "abs" => Some("llvm.abs.i64"),
        "sqrt" => Some("llvm.sqrt.f64"),
        _ => None,
    }
}

/// Returns true if the builtin should be expanded inline rather than called.
pub fn is_inline_builtin(name: &str) -> bool {
    matches!(name, "max" | "min")
}

// ---------------------------------------------------------------------------
// Constant folding
// ---------------------------------------------------------------------------

fn fold_expr(expr: &mut Expr) {
    match expr {
        Expr::Op(_, args) => {
            for a in args.iter_mut() {
                fold_expr(a);
            }
        }
        Expr::Call(_, args) => {
            for a in args.iter_mut() {
                fold_expr(a);
            }
        }
        Expr::Pipe(l, r) => {
            fold_expr(l);
            fold_expr(r);
        }
        Expr::Field(base, _) | Expr::Temporal(base, _) => {
            fold_expr(base);
        }
        Expr::Let { value, .. } => {
            fold_expr(value);
        }
        Expr::If { condition, then_body, elif_branches, else_body } => {
            fold_expr(condition);
            for e in then_body.iter_mut() {
                fold_expr(e);
            }
            for (cond, body) in elif_branches.iter_mut() {
                fold_expr(cond);
                for e in body.iter_mut() {
                    fold_expr(e);
                }
            }
            if let Some(body) = else_body {
                for e in body.iter_mut() {
                    fold_expr(e);
                }
            }
        }
        Expr::Each { iter, body, .. } => {
            fold_expr(iter);
            for e in body.iter_mut() {
                fold_expr(e);
            }
        }
        Expr::While { condition, body } => {
            fold_expr(condition);
            for e in body.iter_mut() {
                fold_expr(e);
            }
        }
        Expr::Block(exprs) => {
            for e in exprs.iter_mut() {
                fold_expr(e);
            }
        }
        _ => {}
    }

    // After children are folded, try to reduce this node.
    if let Some(folded) = try_fold(expr) {
        *expr = folded;
    }
}

fn try_fold(expr: &Expr) -> Option<Expr> {
    let Expr::Op(op, args) = expr else { return None };

    // Unary: Not
    if *op == Op::Not && args.len() == 1 {
        if let Expr::BoolLit(b) = &args[0] {
            return Some(Expr::BoolLit(!b));
        }
        return None;
    }

    if args.len() != 2 {
        return None;
    }

    match (&args[0], &args[1]) {
        // Integer arithmetic
        (Expr::IntLit(l), Expr::IntLit(r)) => fold_int_op(op, *l, *r),
        // Float arithmetic
        (Expr::FloatLit(l), Expr::FloatLit(r)) => fold_float_op(op, *l, *r),
        // Boolean logic
        (Expr::BoolLit(l), Expr::BoolLit(r)) => fold_bool_op(op, *l, *r),
        _ => None,
    }
}

fn fold_int_op(op: &Op, l: i64, r: i64) -> Option<Expr> {
    match op {
        Op::Add => Some(Expr::IntLit(l.wrapping_add(r))),
        Op::Sub => Some(Expr::IntLit(l.wrapping_sub(r))),
        Op::Mul => Some(Expr::IntLit(l.wrapping_mul(r))),
        Op::Div => {
            if r == 0 { None } else { Some(Expr::IntLit(l / r)) }
        }
        Op::Modulo => {
            if r == 0 { None } else { Some(Expr::IntLit(l % r)) }
        }
        Op::Eq  => Some(Expr::BoolLit(l == r)),
        Op::Neq => Some(Expr::BoolLit(l != r)),
        Op::Gt  => Some(Expr::BoolLit(l > r)),
        Op::Lt  => Some(Expr::BoolLit(l < r)),
        Op::Gte => Some(Expr::BoolLit(l >= r)),
        Op::Lte => Some(Expr::BoolLit(l <= r)),
        _ => None,
    }
}

fn fold_float_op(op: &Op, l: f64, r: f64) -> Option<Expr> {
    match op {
        Op::Add => Some(Expr::FloatLit(l + r)),
        Op::Sub => Some(Expr::FloatLit(l - r)),
        Op::Mul => Some(Expr::FloatLit(l * r)),
        Op::Div => {
            if r == 0.0 { None } else { Some(Expr::FloatLit(l / r)) }
        }
        _ => None,
    }
}

fn fold_bool_op(op: &Op, l: bool, r: bool) -> Option<Expr> {
    match op {
        Op::And => Some(Expr::BoolLit(l && r)),
        Op::Or  => Some(Expr::BoolLit(l || r)),
        Op::Eq  => Some(Expr::BoolLit(l == r)),
        Op::Neq => Some(Expr::BoolLit(l != r)),
        _ => None,
    }
}

// ---------------------------------------------------------------------------
// Dead code elimination
// ---------------------------------------------------------------------------

fn dce_expr(expr: &mut Expr) {
    // Recurse first so inner constructs are simplified before we inspect them.
    match expr {
        Expr::Block(exprs) => {
            for e in exprs.iter_mut() {
                dce_expr(e);
            }
        }
        Expr::Let { value, .. } => dce_expr(value),
        Expr::If { condition, then_body, elif_branches, else_body, .. } => {
            dce_expr(condition);
            for e in then_body.iter_mut() {
                dce_expr(e);
            }
            for (cond, body) in elif_branches.iter_mut() {
                dce_expr(cond);
                for e in body.iter_mut() {
                    dce_expr(e);
                }
            }
            if let Some(body) = else_body {
                for e in body.iter_mut() {
                    dce_expr(e);
                }
            }
        }
        Expr::While { condition, body } => {
            dce_expr(condition);
            for e in body.iter_mut() {
                dce_expr(e);
            }
        }
        Expr::Each { iter, body, .. } => {
            dce_expr(iter);
            for e in body.iter_mut() {
                dce_expr(e);
            }
        }
        Expr::Pipe(l, r) => {
            dce_expr(l);
            dce_expr(r);
        }
        _ => {}
    }

    // Now attempt to simplify this node.
    match expr {
        Expr::If { condition, then_body, else_body, .. } => {
            if let Expr::BoolLit(true) = condition.as_ref() {
                *expr = Expr::Block(std::mem::take(then_body));
            } else if let Expr::BoolLit(false) = condition.as_ref() {
                *expr = Expr::Block(else_body.take().unwrap_or_default());
            }
        }
        Expr::While { condition, .. } => {
            if let Expr::BoolLit(false) = condition.as_ref() {
                *expr = Expr::Block(vec![]);
            }
        }
        _ => {}
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    fn int_op(op: Op, l: i64, r: i64) -> Expr {
        Expr::Op(op, vec![Expr::IntLit(l), Expr::IntLit(r)])
    }

    fn bool_op(op: Op, l: bool, r: bool) -> Expr {
        Expr::Op(op, vec![Expr::BoolLit(l), Expr::BoolLit(r)])
    }

    fn wrap_in_program(body: Expr) -> Program {
        Program {
            items: vec![Item::Function(Function {
                name: "test".into(),
                params: vec![],
                returns: None,
                invariants: vec![],
                requires: vec![],
                ensures: vec![],
                mode: FnMode::Strict,
                intent: None,
                confidence: None,
                fallback: None,
                guarantee: None,
                body,
            })],
        }
    }

    fn get_body(program: &Program) -> &Expr {
        match &program.items[0] {
            Item::Function(f) => &f.body,
            _ => panic!("expected function"),
        }
    }

    // -- Constant folding -----------------------------------------------------

    #[test]
    fn fold_add_integers() {
        let mut prog = wrap_in_program(int_op(Op::Add, 1, 2));
        fold_constants(&mut prog);
        assert!(matches!(get_body(&prog), Expr::IntLit(3)));
    }

    #[test]
    fn fold_sub_integers() {
        let mut prog = wrap_in_program(int_op(Op::Sub, 10, 3));
        fold_constants(&mut prog);
        assert!(matches!(get_body(&prog), Expr::IntLit(7)));
    }

    #[test]
    fn fold_mul_integers() {
        let mut prog = wrap_in_program(int_op(Op::Mul, 4, 5));
        fold_constants(&mut prog);
        assert!(matches!(get_body(&prog), Expr::IntLit(20)));
    }

    #[test]
    fn fold_div_integers() {
        let mut prog = wrap_in_program(int_op(Op::Div, 10, 3));
        fold_constants(&mut prog);
        assert!(matches!(get_body(&prog), Expr::IntLit(3)));
    }

    #[test]
    fn fold_div_by_zero_unchanged() {
        let mut prog = wrap_in_program(int_op(Op::Div, 10, 0));
        fold_constants(&mut prog);
        assert!(matches!(get_body(&prog), Expr::Op(Op::Div, _)));
    }

    #[test]
    fn fold_nested_add_mul() {
        // add 1 (mul 2 3) → 7
        let inner = int_op(Op::Mul, 2, 3);
        let outer = Expr::Op(Op::Add, vec![Expr::IntLit(1), inner]);
        let mut prog = wrap_in_program(outer);
        fold_constants(&mut prog);
        assert!(matches!(get_body(&prog), Expr::IntLit(7)));
    }

    #[test]
    fn fold_comparison_gt() {
        let mut prog = wrap_in_program(int_op(Op::Gt, 5, 3));
        fold_constants(&mut prog);
        assert!(matches!(get_body(&prog), Expr::BoolLit(true)));
    }

    #[test]
    fn fold_comparison_lt() {
        let mut prog = wrap_in_program(int_op(Op::Lt, 5, 3));
        fold_constants(&mut prog);
        assert!(matches!(get_body(&prog), Expr::BoolLit(false)));
    }

    #[test]
    fn fold_comparison_eq() {
        let mut prog = wrap_in_program(int_op(Op::Eq, 4, 4));
        fold_constants(&mut prog);
        assert!(matches!(get_body(&prog), Expr::BoolLit(true)));
    }

    #[test]
    fn fold_boolean_and() {
        let mut prog = wrap_in_program(bool_op(Op::And, true, false));
        fold_constants(&mut prog);
        assert!(matches!(get_body(&prog), Expr::BoolLit(false)));
    }

    #[test]
    fn fold_boolean_or() {
        let mut prog = wrap_in_program(bool_op(Op::Or, false, true));
        fold_constants(&mut prog);
        assert!(matches!(get_body(&prog), Expr::BoolLit(true)));
    }

    #[test]
    fn fold_boolean_not() {
        let not_expr = Expr::Op(Op::Not, vec![Expr::BoolLit(true)]);
        let mut prog = wrap_in_program(not_expr);
        fold_constants(&mut prog);
        assert!(matches!(get_body(&prog), Expr::BoolLit(false)));
    }

    #[test]
    fn fold_float_add() {
        let expr = Expr::Op(Op::Add, vec![Expr::FloatLit(1.5), Expr::FloatLit(2.5)]);
        let mut prog = wrap_in_program(expr);
        fold_constants(&mut prog);
        match get_body(&prog) {
            Expr::FloatLit(v) => assert!((*v - 4.0).abs() < f64::EPSILON),
            other => panic!("expected FloatLit, got {other:?}"),
        }
    }

    #[test]
    fn non_constant_unchanged() {
        let expr = Expr::Op(Op::Add, vec![Expr::Ident("x".into()), Expr::IntLit(1)]);
        let mut prog = wrap_in_program(expr.clone());
        fold_constants(&mut prog);
        assert!(matches!(get_body(&prog), Expr::Op(Op::Add, _)));
    }

    // -- Dead code elimination ------------------------------------------------

    #[test]
    fn dce_if_true() {
        let expr = Expr::If {
            condition: Box::new(Expr::BoolLit(true)),
            then_body: vec![Expr::IntLit(42)],
            elif_branches: vec![],
            else_body: Some(vec![Expr::IntLit(0)]),
        };
        let mut prog = wrap_in_program(expr);
        dead_code_elimination(&mut prog);
        match get_body(&prog) {
            Expr::Block(exprs) => {
                assert_eq!(exprs.len(), 1);
                assert!(matches!(exprs[0], Expr::IntLit(42)));
            }
            other => panic!("expected Block, got {other:?}"),
        }
    }

    #[test]
    fn dce_if_false() {
        let expr = Expr::If {
            condition: Box::new(Expr::BoolLit(false)),
            then_body: vec![Expr::IntLit(42)],
            elif_branches: vec![],
            else_body: Some(vec![Expr::IntLit(0)]),
        };
        let mut prog = wrap_in_program(expr);
        dead_code_elimination(&mut prog);
        match get_body(&prog) {
            Expr::Block(exprs) => {
                assert_eq!(exprs.len(), 1);
                assert!(matches!(exprs[0], Expr::IntLit(0)));
            }
            other => panic!("expected Block, got {other:?}"),
        }
    }

    #[test]
    fn dce_if_false_no_else() {
        let expr = Expr::If {
            condition: Box::new(Expr::BoolLit(false)),
            then_body: vec![Expr::IntLit(42)],
            elif_branches: vec![],
            else_body: None,
        };
        let mut prog = wrap_in_program(expr);
        dead_code_elimination(&mut prog);
        match get_body(&prog) {
            Expr::Block(exprs) => assert!(exprs.is_empty()),
            other => panic!("expected empty Block, got {other:?}"),
        }
    }

    #[test]
    fn dce_while_false() {
        let expr = Expr::While {
            condition: Box::new(Expr::BoolLit(false)),
            body: vec![Expr::IntLit(1)],
        };
        let mut prog = wrap_in_program(expr);
        dead_code_elimination(&mut prog);
        match get_body(&prog) {
            Expr::Block(exprs) => assert!(exprs.is_empty()),
            other => panic!("expected empty Block, got {other:?}"),
        }
    }

    // -- Intrinsics -----------------------------------------------------------

    #[test]
    fn intrinsic_abs() {
        assert_eq!(is_llvm_intrinsic("abs"), Some("llvm.abs.i64"));
    }

    #[test]
    fn intrinsic_sqrt() {
        assert_eq!(is_llvm_intrinsic("sqrt"), Some("llvm.sqrt.f64"));
    }

    #[test]
    fn intrinsic_unknown() {
        assert_eq!(is_llvm_intrinsic("foobar"), None);
    }

    #[test]
    fn inline_builtin_max_min() {
        assert!(is_inline_builtin("max"));
        assert!(is_inline_builtin("min"));
        assert!(!is_inline_builtin("abs"));
    }
}
