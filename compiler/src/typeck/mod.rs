use std::collections::HashMap;

use crate::parser::ast::*;
use crate::stdlib;

/// Resolved type — the concrete type after checking.
#[derive(Debug, Clone, PartialEq)]
pub enum Ty {
    Int,
    Uint,
    Float,
    Bool,
    Str,
    Void,
    List(Box<Ty>),
    Map(Box<Ty>, Box<Ty>),
    Tuple(Vec<Ty>),
    Optional(Box<Ty>),
    Fn(Vec<Ty>, Box<Ty>),
    UserDefined(String),
    Var(usize),
    Enum(String),
    Future(Box<Ty>),
    Channel(Box<Ty>),
    /// Type could not be resolved (permits partial checking).
    Unknown,
}

impl std::fmt::Display for Ty {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Ty::Int => write!(f, "int"),
            Ty::Uint => write!(f, "uint"),
            Ty::Float => write!(f, "float"),
            Ty::Bool => write!(f, "bool"),
            Ty::Str => write!(f, "str"),
            Ty::Void => write!(f, "void"),
            Ty::List(t) => write!(f, "[{t}]"),
            Ty::Map(k, v) => write!(f, "{{{k}: {v}}}"),
            Ty::Tuple(ts) => {
                let inner: Vec<_> = ts.iter().map(|t| t.to_string()).collect();
                write!(f, "({})", inner.join(", "))
            }
            Ty::Optional(t) => write!(f, "{t}?"),
            Ty::Fn(params, ret) => {
                let ps: Vec<_> = params.iter().map(|t| t.to_string()).collect();
                write!(f, "({} -> {})", ps.join(", "), ret)
            }
            Ty::UserDefined(n) => write!(f, "{n}"),
            Ty::Var(n) => write!(f, "?{n}"),
            Ty::Enum(n) => write!(f, "{n}"),
            Ty::Future(t) => write!(f, "Future<{t}>"),
            Ty::Channel(t) => write!(f, "Channel<{t}>"),
            Ty::Unknown => write!(f, "?"),
        }
    }
}

#[derive(Debug)]
pub struct TypeError {
    pub message: String,
    pub context: String,
}

impl std::fmt::Display for TypeError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "type error in {}: {}", self.context, self.message)
    }
}

impl std::error::Error for TypeError {}

// ── Substitution (Hindley-Milner unification) ──────────────────────────────

struct Substitution {
    map: HashMap<usize, Ty>,
}

impl Substitution {
    fn new() -> Self {
        Self { map: HashMap::new() }
    }

    fn apply(&self, ty: &Ty) -> Ty {
        match ty {
            Ty::Var(n) => {
                if let Some(resolved) = self.map.get(n) {
                    self.apply(resolved)
                } else {
                    ty.clone()
                }
            }
            Ty::List(inner) => Ty::List(Box::new(self.apply(inner))),
            Ty::Map(k, v) => Ty::Map(Box::new(self.apply(k)), Box::new(self.apply(v))),
            Ty::Tuple(ts) => Ty::Tuple(ts.iter().map(|t| self.apply(t)).collect()),
            Ty::Optional(inner) => Ty::Optional(Box::new(self.apply(inner))),
            Ty::Fn(params, ret) => Ty::Fn(
                params.iter().map(|t| self.apply(t)).collect(),
                Box::new(self.apply(ret)),
            ),
            Ty::Future(inner) => Ty::Future(Box::new(self.apply(inner))),
            Ty::Channel(inner) => Ty::Channel(Box::new(self.apply(inner))),
            _ => ty.clone(),
        }
    }

    /// Walk the substitution chain to find the final binding for a variable.
    fn resolve(&self, ty: &Ty) -> Ty {
        match ty {
            Ty::Var(n) => {
                if let Some(resolved) = self.map.get(n) {
                    self.resolve(resolved)
                } else {
                    ty.clone()
                }
            }
            _ => ty.clone(),
        }
    }

    fn occurs_in(&self, var: usize, ty: &Ty) -> bool {
        let ty = self.resolve(ty);
        match ty {
            Ty::Var(n) => n == var,
            Ty::List(inner) => self.occurs_in(var, &inner),
            Ty::Map(k, v) => self.occurs_in(var, &k) || self.occurs_in(var, &v),
            Ty::Tuple(ts) => ts.iter().any(|t| self.occurs_in(var, t)),
            Ty::Optional(inner) => self.occurs_in(var, &inner),
            Ty::Fn(params, ret) => {
                params.iter().any(|t| self.occurs_in(var, t)) || self.occurs_in(var, &ret)
            }
            Ty::Future(inner) => self.occurs_in(var, &inner),
            Ty::Channel(inner) => self.occurs_in(var, &inner),
            _ => false,
        }
    }

    fn unify(&mut self, a: &Ty, b: &Ty) -> Result<(), TypeError> {
        let a = self.resolve(a);
        let b = self.resolve(b);

        if a == b {
            return Ok(());
        }

        match (&a, &b) {
            // Either side is Unknown — always compatible (permits partial checking).
            (Ty::Unknown, _) | (_, Ty::Unknown) => Ok(()),

            (Ty::Var(n), _) => {
                if self.occurs_in(*n, &b) {
                    Err(TypeError {
                        message: format!("infinite type: ?{n} occurs in {b}"),
                        context: String::new(),
                    })
                } else {
                    self.map.insert(*n, b);
                    Ok(())
                }
            }
            (_, Ty::Var(n)) => {
                if self.occurs_in(*n, &a) {
                    Err(TypeError {
                        message: format!("infinite type: ?{n} occurs in {a}"),
                        context: String::new(),
                    })
                } else {
                    self.map.insert(*n, a);
                    Ok(())
                }
            }

            (Ty::List(a_inner), Ty::List(b_inner)) => self.unify(a_inner, b_inner),

            (Ty::Map(ak, av), Ty::Map(bk, bv)) => {
                self.unify(ak, bk)?;
                self.unify(av, bv)
            }

            (Ty::Tuple(a_ts), Ty::Tuple(b_ts)) if a_ts.len() == b_ts.len() => {
                for (at, bt) in a_ts.iter().zip(b_ts.iter()) {
                    self.unify(at, bt)?;
                }
                Ok(())
            }

            (Ty::Optional(a_inner), Ty::Optional(b_inner)) => self.unify(a_inner, b_inner),

            (Ty::Fn(a_params, a_ret), Ty::Fn(b_params, b_ret))
                if a_params.len() == b_params.len() =>
            {
                for (ap, bp) in a_params.iter().zip(b_params.iter()) {
                    self.unify(ap, bp)?;
                }
                self.unify(a_ret, b_ret)
            }

            (Ty::Enum(a_name), Ty::Enum(b_name)) if a_name == b_name => Ok(()),

            (Ty::Future(a_inner), Ty::Future(b_inner)) => self.unify(a_inner, b_inner),

            (Ty::Channel(a_inner), Ty::Channel(b_inner)) => self.unify(a_inner, b_inner),

            _ => Err(TypeError {
                message: format!("cannot unify {a} with {b}"),
                context: String::new(),
            }),
        }
    }
}

// ── Inference engine ───────────────────────────────────────────────────────

struct InferenceEngine {
    next_var: usize,
    substitution: Substitution,
    env: HashMap<String, Ty>,
    structs: HashMap<String, Vec<(String, Ty)>>,
    enums: HashMap<String, Vec<(String, Vec<Ty>)>>,
    functions: HashMap<String, FnSig>,
    errors: Vec<TypeError>,
    context: String,
}

#[derive(Clone)]
struct FnSig {
    #[allow(dead_code)]
    params: Vec<(String, Ty)>,
    returns: Option<Ty>,
}

impl InferenceEngine {
    fn new() -> Self {
        Self {
            next_var: 0,
            substitution: Substitution::new(),
            env: HashMap::new(),
            structs: HashMap::new(),
            enums: HashMap::new(),
            functions: HashMap::new(),
            errors: Vec::new(),
            context: String::new(),
        }
    }

    fn fresh_var(&mut self) -> Ty {
        let v = self.next_var;
        self.next_var += 1;
        Ty::Var(v)
    }

    fn unify(&mut self, a: &Ty, b: &Ty) {
        if let Err(mut e) = self.substitution.unify(a, b) {
            if e.context.is_empty() {
                e.context = self.context.clone();
            }
            self.errors.push(e);
        }
    }

    /// Unify a builtin parameter. If the expected param is a Fn type (e.g. a
    /// predicate `(T -> bool)`), kernl passes inline operator expressions rather
    /// than first-class function values, so we unify the argument with the Fn's
    /// return type instead of the full Fn type.
    fn unify_builtin_param(&mut self, arg_ty: &Ty, param_ty: &Ty) {
        let param_resolved = self.substitution.apply(param_ty);
        if let Ty::Fn(_fn_params, ref ret) = param_resolved {
            self.unify(arg_ty, ret);
        } else {
            self.unify(arg_ty, param_ty);
        }
    }

    fn resolve_ast_type(&self, ty: &Type) -> Ty {
        match ty {
            Type::Named(name) => match name.as_str() {
                "int" => Ty::Int,
                "uint" => Ty::Uint,
                "float" => Ty::Float,
                "bool" => Ty::Bool,
                "str" => Ty::Str,
                "void" => Ty::Void,
                _ => Ty::UserDefined(name.clone()),
            },
            Type::List(inner) => Ty::List(Box::new(self.resolve_ast_type(inner))),
            Type::Map(k, v) => Ty::Map(
                Box::new(self.resolve_ast_type(k)),
                Box::new(self.resolve_ast_type(v)),
            ),
            Type::Tuple(ts) => Ty::Tuple(ts.iter().map(|t| self.resolve_ast_type(t)).collect()),
            Type::Optional(inner) => Ty::Optional(Box::new(self.resolve_ast_type(inner))),
        }
    }

    /// Parse a stdlib type string (e.g. "[T]", "int", "(T -> bool)") into a Ty,
    /// using `generic_map` to resolve single-letter generic names to fresh vars.
    fn parse_stdlib_ty(&mut self, s: &str, generic_map: &mut HashMap<String, Ty>) -> Ty {
        let s = s.trim();
        if s.starts_with('[') && s.ends_with(']') {
            let inner = &s[1..s.len() - 1];
            Ty::List(Box::new(self.parse_stdlib_ty(inner, generic_map)))
        } else if s.starts_with('(') && s.ends_with(')') {
            let inner = &s[1..s.len() - 1];
            if let Some(arrow_pos) = inner.rfind("->") {
                let params_str = inner[..arrow_pos].trim();
                let ret_str = inner[arrow_pos + 2..].trim();
                let params: Vec<Ty> = params_str
                    .split(',')
                    .map(|p| self.parse_stdlib_ty(p.trim(), generic_map))
                    .collect();
                let ret = self.parse_stdlib_ty(ret_str, generic_map);
                Ty::Fn(params, Box::new(ret))
            } else {
                Ty::Unknown
            }
        } else {
            match s {
                "int" => Ty::Int,
                "uint" => Ty::Uint,
                "float" => Ty::Float,
                "bool" => Ty::Bool,
                "str" => Ty::Str,
                "void" => Ty::Void,
                name if name.len() == 1 && name.chars().next().unwrap().is_ascii_uppercase() => {
                    generic_map
                        .entry(name.to_string())
                        .or_insert_with(|| self.fresh_var())
                        .clone()
                }
                _ => Ty::UserDefined(s.to_string()),
            }
        }
    }

    fn infer(&mut self, expr: &Expr) -> Ty {
        match expr {
            Expr::IntLit(_) => Ty::Int,
            Expr::FloatLit(_) => Ty::Float,
            Expr::StrLit(_) => Ty::Str,
            Expr::BoolLit(_) => Ty::Bool,

            Expr::Ident(name) => {
                if let Some(ty) = self.env.get(name) {
                    ty.clone()
                } else {
                    let v = self.fresh_var();
                    self.env.insert(name.clone(), v.clone());
                    v
                }
            }

            Expr::Op(op, args) => self.infer_op(op, args),

            Expr::Call(name, args) => self.infer_call(name, args),

            Expr::Pipe(left, right) => {
                let left_ty = self.infer(left);
                self.infer_pipe_right(right, &left_ty)
            }

            Expr::Field(base, field_name) => {
                let base_ty = self.infer(base);
                let base_ty = self.substitution.apply(&base_ty);
                if let Ty::UserDefined(ref struct_name) = base_ty {
                    if let Some(fields) = self.structs.get(struct_name).cloned() {
                        return fields
                            .iter()
                            .find(|(n, _)| n == field_name)
                            .map(|(_, ty)| ty.clone())
                            .unwrap_or_else(|| self.fresh_var());
                    }
                }
                self.fresh_var()
            }

            Expr::Temporal(inner, _) => self.infer(inner),

            Expr::Let { name, ty, value, .. } => {
                let val_ty = self.infer(value);
                if let Some(declared) = ty {
                    let declared_ty = self.resolve_ast_type(declared);
                    self.unify(&val_ty, &declared_ty);
                    self.env.insert(name.clone(), declared_ty.clone());
                    declared_ty
                } else {
                    self.env.insert(name.clone(), val_ty.clone());
                    val_ty
                }
            }

            Expr::If {
                condition,
                then_body,
                elif_branches,
                else_body,
            } => {
                let cond_ty = self.infer(condition);
                self.unify(&cond_ty, &Ty::Bool);

                let then_ty = self.infer_block(then_body);

                for (elif_cond, elif_body) in elif_branches {
                    let ec_ty = self.infer(elif_cond);
                    self.unify(&ec_ty, &Ty::Bool);
                    let eb_ty = self.infer_block(elif_body);
                    self.unify(&then_ty, &eb_ty);
                }

                if let Some(else_exprs) = else_body {
                    let else_ty = self.infer_block(else_exprs);
                    self.unify(&then_ty, &else_ty);
                }

                then_ty
            }

            Expr::Each { binding, iter, body } => {
                let iter_ty = self.infer(iter);
                let elem_var = self.fresh_var();
                self.unify(&iter_ty, &Ty::List(Box::new(elem_var.clone())));
                self.env.insert(binding.clone(), elem_var);
                self.infer_block(body);
                Ty::Void
            }

            Expr::While { condition, body } => {
                let cond_ty = self.infer(condition);
                self.unify(&cond_ty, &Ty::Bool);
                self.infer_block(body);
                Ty::Void
            }

            Expr::Block(exprs) => self.infer_block(exprs),

            Expr::EnumVariant(enum_name, _variant_name, args) => {
                for arg in args {
                    self.infer(arg);
                }
                Ty::Enum(enum_name.clone())
            }

            Expr::Match { scrutinee, arms } => {
                let scrutinee_ty = self.infer(scrutinee);
                let result_ty = self.fresh_var();

                let mut covered_variants: Vec<String> = Vec::new();
                let mut has_wildcard = false;

                for arm in arms {
                    self.check_pattern(&arm.pattern, &scrutinee_ty, &mut covered_variants, &mut has_wildcard);
                    let arm_ty = self.infer_block(&arm.body);
                    self.unify(&result_ty, &arm_ty);
                }

                let scrutinee_resolved = self.substitution.apply(&scrutinee_ty);
                if let Ty::Enum(ref enum_name) = scrutinee_resolved {
                    if !has_wildcard {
                        if let Some(variants) = self.enums.get(enum_name).cloned() {
                            for (vname, _) in &variants {
                                if !covered_variants.contains(vname) {
                                    self.errors.push(TypeError {
                                        message: format!("non-exhaustive match: missing variant '{vname}' of enum '{enum_name}'"),
                                        context: self.context.clone(),
                                    });
                                }
                            }
                        }
                    }
                }

                result_ty
            }

            Expr::Spawn(inner) => {
                let inner_ty = self.infer(inner);
                Ty::Future(Box::new(inner_ty))
            }

            Expr::Await(inner) => {
                let inner_ty = self.infer(inner);
                let result_var = self.fresh_var();
                self.unify(&inner_ty, &Ty::Future(Box::new(result_var.clone())));
                result_var
            }

            Expr::Send(chan, val) => {
                let chan_ty = self.infer(chan);
                let val_ty = self.infer(val);
                self.unify(&chan_ty, &Ty::Channel(Box::new(val_ty)));
                Ty::Void
            }

            Expr::Recv(chan) => {
                let chan_ty = self.infer(chan);
                let elem_var = self.fresh_var();
                self.unify(&chan_ty, &Ty::Channel(Box::new(elem_var.clone())));
                elem_var
            }
        }
    }

    fn check_pattern(&mut self, pattern: &Pattern, expected_ty: &Ty, covered: &mut Vec<String>, has_wildcard: &mut bool) {
        match pattern {
            Pattern::Wildcard => {
                *has_wildcard = true;
            }
            Pattern::Literal(expr) => {
                let lit_ty = self.infer(expr);
                self.unify(&lit_ty, expected_ty);
            }
            Pattern::Binding(name) => {
                self.env.insert(name.clone(), expected_ty.clone());
            }
            Pattern::Variant(name, sub_pats) => {
                covered.push(name.clone());
                let expected_resolved = self.substitution.apply(expected_ty);
                if let Ty::Enum(ref enum_name) = expected_resolved {
                    if let Some(variants) = self.enums.get(enum_name).cloned() {
                        if let Some((_, field_tys)) = variants.iter().find(|(vn, _)| vn == name) {
                            for (i, sub_pat) in sub_pats.iter().enumerate() {
                                let field_ty = field_tys.get(i).cloned().unwrap_or(Ty::Unknown);
                                self.check_pattern(sub_pat, &field_ty, covered, has_wildcard);
                            }
                        }
                    }
                }
            }
            Pattern::Tuple(pats) => {
                let expected_resolved = self.substitution.apply(expected_ty);
                if let Ty::Tuple(ref tys) = expected_resolved {
                    for (i, pat) in pats.iter().enumerate() {
                        let ty = tys.get(i).cloned().unwrap_or(Ty::Unknown);
                        self.check_pattern(pat, &ty, covered, has_wildcard);
                    }
                }
            }
        }
    }

    fn infer_block(&mut self, exprs: &[Expr]) -> Ty {
        let mut last = Ty::Void;
        for expr in exprs {
            last = self.infer(expr);
        }
        last
    }

    fn infer_op(&mut self, op: &Op, args: &[Expr]) -> Ty {
        match op {
            Op::Add | Op::Sub | Op::Mul | Op::Div | Op::Modulo => {
                let numeric = self.fresh_var();
                for arg in args {
                    let ty = self.infer(arg);
                    self.unify(&ty, &numeric);
                }
                let resolved = self.substitution.apply(&numeric);
                match resolved {
                    Ty::Int | Ty::Uint | Ty::Float | Ty::Var(_) => resolved,
                    _ => {
                        self.errors.push(TypeError {
                            message: format!("arithmetic requires numeric type, got {resolved}"),
                            context: self.context.clone(),
                        });
                        Ty::Unknown
                    }
                }
            }
            Op::Eq | Op::Neq | Op::Gt | Op::Lt | Op::Gte | Op::Lte => {
                let operand = self.fresh_var();
                for arg in args {
                    let ty = self.infer(arg);
                    self.unify(&ty, &operand);
                }
                Ty::Bool
            }
            Op::And | Op::Or | Op::Not => {
                for arg in args {
                    let ty = self.infer(arg);
                    self.unify(&ty, &Ty::Bool);
                }
                Ty::Bool
            }
        }
    }

    fn infer_call(&mut self, name: &str, args: &[Expr]) -> Ty {
        let arg_tys: Vec<Ty> = args.iter().map(|a| self.infer(a)).collect();

        if let Some(sig) = self.functions.get(name).cloned() {
            for (i, (_, param_ty)) in sig.params.iter().enumerate() {
                if let Some(arg_ty) = arg_tys.get(i) {
                    self.unify(arg_ty, param_ty);
                }
            }
            return sig.returns.clone().unwrap_or(Ty::Void);
        }

        if let Some(ret) = self.infer_builtin_call(name, &arg_tys) {
            return ret;
        }

        if let Some(fn_ty) = self.env.get(name).cloned() {
            let fn_ty = self.substitution.apply(&fn_ty);
            if let Ty::Fn(param_tys, ret_ty) = fn_ty {
                for (i, pt) in param_tys.iter().enumerate() {
                    if let Some(at) = arg_tys.get(i) {
                        self.unify(at, pt);
                    }
                }
                return *ret_ty;
            }
        }

        let ret = self.fresh_var();
        ret
    }

    fn infer_builtin_call(&mut self, name: &str, arg_tys: &[Ty]) -> Option<Ty> {
        let variants: Vec<_> = stdlib::builtins()
            .iter()
            .filter(|b| b.name == name)
            .collect();

        if variants.is_empty() {
            return None;
        }

        if variants.len() == 1 {
            let builtin = variants[0];
            let mut generic_map = HashMap::new();
            let param_tys: Vec<Ty> = builtin
                .params
                .iter()
                .map(|p| self.parse_stdlib_ty(p.ty, &mut generic_map))
                .collect();
            let ret_ty = self.parse_stdlib_ty(builtin.return_ty, &mut generic_map);

            for (i, pt) in param_tys.iter().enumerate() {
                if let Some(at) = arg_tys.get(i) {
                    self.unify_builtin_param(at, pt);
                }
            }
            return Some(ret_ty);
        }

        // Overloaded builtins (e.g. max/min with int/float variants):
        // try each variant and pick the first that unifies without error.
        for builtin in &variants {
            let mut generic_map = HashMap::new();
            let mut test_sub = Substitution {
                map: self.substitution.map.clone(),
            };
            let param_tys: Vec<Ty> = builtin
                .params
                .iter()
                .map(|p| self.parse_stdlib_ty(p.ty, &mut generic_map))
                .collect();
            let ret_ty = self.parse_stdlib_ty(builtin.return_ty, &mut generic_map);

            let mut ok = true;
            for (i, pt) in param_tys.iter().enumerate() {
                if let Some(at) = arg_tys.get(i) {
                    if test_sub.unify(at, pt).is_err() {
                        ok = false;
                        break;
                    }
                }
            }
            if ok {
                self.substitution.map = test_sub.map;
                return Some(ret_ty);
            }
        }

        // Fallback: use first variant, which will produce errors if mismatched.
        let builtin = variants[0];
        let mut generic_map = HashMap::new();
        let param_tys: Vec<Ty> = builtin
            .params
            .iter()
            .map(|p| self.parse_stdlib_ty(p.ty, &mut generic_map))
            .collect();
        let ret_ty = self.parse_stdlib_ty(builtin.return_ty, &mut generic_map);
        for (i, pt) in param_tys.iter().enumerate() {
            if let Some(at) = arg_tys.get(i) {
                self.unify_builtin_param(at, pt);
            }
        }
        Some(ret_ty)
    }

    /// Infer the right-hand side of a pipe, where `left_ty` is piped as the first arg.
    fn infer_pipe_right(&mut self, right: &Expr, left_ty: &Ty) -> Ty {
        match right {
            Expr::Call(name, args) => {
                let mut all_arg_tys = vec![left_ty.clone()];
                for a in args {
                    all_arg_tys.push(self.infer(a));
                }
                self.infer_call_with_arg_tys(name, &all_arg_tys)
            }
            Expr::Ident(name) => self.infer_call_with_arg_tys(name, &[left_ty.clone()]),
            Expr::Pipe(inner_left, inner_right) => {
                let mid_ty = self.infer_pipe_right(inner_left, left_ty);
                self.infer_pipe_right(inner_right, &mid_ty)
            }
            Expr::Op(op, args) => {
                let mut all_args = vec![left_ty.clone()];
                for a in args {
                    all_args.push(self.infer(a));
                }
                self.infer_op_with_tys(op, &all_args)
            }
            _ => self.infer(right),
        }
    }

    fn infer_call_with_arg_tys(&mut self, name: &str, arg_tys: &[Ty]) -> Ty {
        if let Some(sig) = self.functions.get(name).cloned() {
            for (i, (_, param_ty)) in sig.params.iter().enumerate() {
                if let Some(arg_ty) = arg_tys.get(i) {
                    self.unify(arg_ty, param_ty);
                }
            }
            return sig.returns.clone().unwrap_or(Ty::Void);
        }

        if let Some(ret) = self.infer_builtin_call(name, arg_tys) {
            return ret;
        }

        self.fresh_var()
    }

    fn infer_op_with_tys(&mut self, op: &Op, arg_tys: &[Ty]) -> Ty {
        match op {
            Op::Add | Op::Sub | Op::Mul | Op::Div | Op::Modulo => {
                let numeric = self.fresh_var();
                for ty in arg_tys {
                    self.unify(ty, &numeric);
                }
                let resolved = self.substitution.apply(&numeric);
                match resolved {
                    Ty::Int | Ty::Uint | Ty::Float | Ty::Var(_) => resolved,
                    _ => Ty::Unknown,
                }
            }
            Op::Eq | Op::Neq | Op::Gt | Op::Lt | Op::Gte | Op::Lte => {
                let operand = self.fresh_var();
                for ty in arg_tys {
                    self.unify(ty, &operand);
                }
                Ty::Bool
            }
            Op::And | Op::Or | Op::Not => {
                for ty in arg_tys {
                    self.unify(ty, &Ty::Bool);
                }
                Ty::Bool
            }
        }
    }

    /// Finalize a type: apply substitution and convert remaining Var to Unknown.
    fn finalize(&self, ty: &Ty) -> Ty {
        let resolved = self.substitution.apply(ty);
        self.strip_vars(&resolved)
    }

    fn strip_vars(&self, ty: &Ty) -> Ty {
        match ty {
            Ty::Var(_) => Ty::Unknown,
            Ty::List(inner) => Ty::List(Box::new(self.strip_vars(inner))),
            Ty::Map(k, v) => Ty::Map(Box::new(self.strip_vars(k)), Box::new(self.strip_vars(v))),
            Ty::Tuple(ts) => Ty::Tuple(ts.iter().map(|t| self.strip_vars(t)).collect()),
            Ty::Optional(inner) => Ty::Optional(Box::new(self.strip_vars(inner))),
            Ty::Fn(params, ret) => Ty::Fn(
                params.iter().map(|t| self.strip_vars(t)).collect(),
                Box::new(self.strip_vars(ret)),
            ),
            Ty::Future(inner) => Ty::Future(Box::new(self.strip_vars(inner))),
            Ty::Channel(inner) => Ty::Channel(Box::new(self.strip_vars(inner))),
            _ => ty.clone(),
        }
    }
}

// ── Public API (unchanged) ─────────────────────────────────────────────────

pub struct TypeChecker {
    structs: HashMap<String, Vec<(String, Ty)>>,
    enums: HashMap<String, Vec<(String, Vec<Ty>)>>,
    functions: HashMap<String, FnSig>,
}

impl TypeChecker {
    pub fn check(program: &Program) -> Vec<TypeError> {
        let mut checker = Self {
            structs: HashMap::new(),
            enums: HashMap::new(),
            functions: HashMap::new(),
        };

        checker.register_items(program);
        checker.check_items(program)
    }

    fn resolve_type(&self, ty: &Type) -> Ty {
        match ty {
            Type::Named(name) => match name.as_str() {
                "int" => Ty::Int,
                "uint" => Ty::Uint,
                "float" => Ty::Float,
                "bool" => Ty::Bool,
                "str" => Ty::Str,
                "void" => Ty::Void,
                _ => {
                    if self.enums.contains_key(name) {
                        Ty::Enum(name.clone())
                    } else {
                        Ty::UserDefined(name.clone())
                    }
                }
            },
            Type::List(inner) => Ty::List(Box::new(self.resolve_type(inner))),
            Type::Map(k, v) => Ty::Map(
                Box::new(self.resolve_type(k)),
                Box::new(self.resolve_type(v)),
            ),
            Type::Tuple(ts) => Ty::Tuple(ts.iter().map(|t| self.resolve_type(t)).collect()),
            Type::Optional(inner) => Ty::Optional(Box::new(self.resolve_type(inner))),
        }
    }

    fn register_items(&mut self, program: &Program) {
        for item in &program.items {
            match item {
                Item::Struct(s) => {
                    let fields: Vec<_> = s
                        .fields
                        .iter()
                        .map(|p| (p.name.clone(), self.resolve_type(&p.ty)))
                        .collect();
                    self.structs.insert(s.name.clone(), fields);
                }
                Item::Enum(e) => {
                    let variants: Vec<_> = e
                        .variants
                        .iter()
                        .map(|v| {
                            let field_tys: Vec<_> = v.fields.iter().map(|t| self.resolve_type(t)).collect();
                            (v.name.clone(), field_tys)
                        })
                        .collect();
                    self.enums.insert(e.name.clone(), variants);
                }
                Item::Function(f) => {
                    let params: Vec<_> = f
                        .params
                        .iter()
                        .map(|p| (p.name.clone(), self.resolve_type(&p.ty)))
                        .collect();
                    let returns = f.returns.as_ref().map(|p| self.resolve_type(&p.ty));
                    self.functions
                        .insert(f.name.clone(), FnSig { params, returns });
                }
                _ => {}
            }
        }
    }

    fn check_items(&self, program: &Program) -> Vec<TypeError> {
        let mut errors = Vec::new();
        for item in &program.items {
            match item {
                Item::Function(f) => errors.extend(self.check_function(f)),
                Item::Struct(s) => errors.extend(self.check_struct(s)),
                _ => {}
            }
        }
        errors
    }

    fn check_struct(&self, s: &StructDef) -> Vec<TypeError> {
        let mut errors = Vec::new();
        for field in &s.fields {
            let ty = self.resolve_type(&field.ty);
            if let Ty::UserDefined(ref name) = ty {
                if !self.structs.contains_key(name) {
                    errors.push(TypeError {
                        message: format!("unknown type '{name}' for field '{}'", field.name),
                        context: format!("struct {}", s.name),
                    });
                }
            }
        }
        errors
    }

    fn check_function(&self, func: &Function) -> Vec<TypeError> {
        let mut engine = InferenceEngine::new();
        engine.structs = self.structs.clone();
        engine.enums = self.enums.clone();
        engine.functions.clone_from(&self.functions);
        engine.context = format!("fn {}", func.name);

        for param in &func.params {
            let ty = self.resolve_type(&param.ty);
            self.check_type_exists(&ty, &param.name, &engine.context, &mut engine.errors);
            engine.env.insert(param.name.clone(), ty);
        }

        if let Some(ref ret) = func.returns {
            let ty = self.resolve_type(&ret.ty);
            self.check_type_exists(&ty, &ret.name, &engine.context, &mut engine.errors);
            engine.env.insert(ret.name.clone(), ty);
        }

        for inv in &func.invariants {
            let inv_ty = engine.infer(inv);
            let inv_ty = engine.substitution.apply(&inv_ty);
            let inv_final = engine.finalize(&inv_ty);
            if inv_final != Ty::Bool && inv_final != Ty::Unknown {
                engine.errors.push(TypeError {
                    message: format!("invariant must be bool, got {inv_final}"),
                    context: engine.context.clone(),
                });
            }
        }

        let body_ty = engine.infer(&func.body);
        if let Some(ref ret) = func.returns {
            let expected = self.resolve_type(&ret.ty);
            let body_final = engine.finalize(&body_ty);
            if body_final != Ty::Unknown && expected != Ty::Unknown && body_final != expected {
                engine.unify(&body_ty, &expected);
            }
        }

        engine.errors
    }

    fn check_type_exists(&self, ty: &Ty, name: &str, ctx: &str, errors: &mut Vec<TypeError>) {
        if let Ty::UserDefined(type_name) = ty {
            if !self.structs.contains_key(type_name) && !self.enums.contains_key(type_name) {
                errors.push(TypeError {
                    message: format!("unknown type '{type_name}' for '{name}'"),
                    context: ctx.to_string(),
                });
            }
        }
        match ty {
            Ty::List(inner) => self.check_type_exists(inner, name, ctx, errors),
            Ty::Map(k, v) => {
                self.check_type_exists(k, name, ctx, errors);
                self.check_type_exists(v, name, ctx, errors);
            }
            Ty::Optional(inner) => self.check_type_exists(inner, name, ctx, errors),
            _ => {}
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::lexer::Lexer;
    use crate::parser::Parser;

    fn check(input: &str) -> Vec<TypeError> {
        let tokens = Lexer::new(input).tokenize().unwrap();
        let program = Parser::new(tokens).parse_program().unwrap();
        TypeChecker::check(&program)
    }

    // ── Existing tests (must keep passing) ─────────────────────────────

    #[test]
    fn valid_simple_function() {
        let errors = check("fn add_one\n  in x: int\n  out result: int\n  do add x 1");
        assert!(errors.is_empty(), "unexpected errors: {errors:?}");
    }

    #[test]
    fn valid_struct_and_field() {
        let errors = check(
            "struct Account\n  id: uint\n  balance: int\nend\n\
             fn get_balance\n  in acc: Account\n  out result: int\n  do acc.balance",
        );
        assert!(errors.is_empty(), "unexpected errors: {errors:?}");
    }

    #[test]
    fn invariant_must_be_bool() {
        let errors = check("fn bad\n  in x: int\n  inv add x 1\n  do x");
        assert!(
            errors.iter().any(|e| e.message.contains("invariant must be bool")),
            "expected invariant type error, got: {errors:?}"
        );
    }

    #[test]
    fn unknown_type_in_param() {
        let errors = check("fn bad\n  in x: Widget\n  do x");
        assert!(
            errors.iter().any(|e| e.message.contains("unknown type 'Widget'")),
            "expected unknown type error, got: {errors:?}"
        );
    }

    #[test]
    fn valid_invariants_are_bool() {
        let errors = check("fn clamp\n  in val: int lo: int hi: int\n  out result: int\n  inv gte result lo\n  inv lte result hi\n  do max lo min hi val");
        let inv_errors: Vec<_> = errors
            .iter()
            .filter(|e| e.message.contains("invariant"))
            .collect();
        assert!(
            inv_errors.is_empty(),
            "invariants should be bool: {inv_errors:?}"
        );
    }

    #[test]
    fn struct_unknown_field_type() {
        let errors = check("struct Bad\n  x: Nonexistent\nend");
        assert!(
            errors
                .iter()
                .any(|e| e.message.contains("unknown type 'Nonexistent'")),
            "expected unknown type error in struct, got: {errors:?}"
        );
    }

    // ── New HM inference tests ─────────────────────────────────────────

    #[test]
    fn infer_let_without_annotation() {
        let errors = check("fn test\n  do let x = 42");
        assert!(errors.is_empty(), "unexpected errors: {errors:?}");
    }

    #[test]
    fn infer_through_pipe() {
        let errors = check(
            "fn test\n  in nums: [int]\n  out result: int\n  do filter nums gt 0 | len",
        );
        assert!(errors.is_empty(), "pipe chain should propagate types: {errors:?}");
    }

    #[test]
    fn unify_mismatched_types() {
        let errors = check("fn bad\n  do add \"hello\" 1");
        assert!(
            !errors.is_empty(),
            "expected type error for add with str and int"
        );
    }

    #[test]
    fn infer_if_branches_unify() {
        let errors = check(
            "fn test\n  in x: bool\n  out result: int\n  do if x\n    42\n  else\n    \"nope\"\n  end",
        );
        assert!(
            !errors.is_empty(),
            "expected type error: if branches must have same type"
        );
    }

    #[test]
    fn infer_builtin_return_type() {
        let errors = check(
            "fn test\n  in nums: [int]\n  out result: int\n  do len nums",
        );
        assert!(errors.is_empty(), "len should return int: {errors:?}");
    }

    #[test]
    fn generic_builtin_instantiation() {
        let errors = check(
            "fn test\n  in nums: [int]\n  out result: [int]\n  do filter nums gt 0",
        );
        assert!(
            errors.is_empty(),
            "filter [int] should return [int]: {errors:?}"
        );
    }

    #[test]
    fn occurs_check_error() {
        let mut sub = Substitution::new();
        let result = sub.unify(&Ty::Var(0), &Ty::List(Box::new(Ty::Var(0))));
        assert!(result.is_err(), "occurs check should prevent infinite type");
        assert!(
            result
                .unwrap_err()
                .message
                .contains("infinite type"),
            "error should mention infinite type"
        );
    }

    #[test]
    fn type_check_match_arms_same_type() {
        let src = "enum Color\n  Red\n  Blue\nend\n\
                   fn test\n  in x: int\n  do match x\n    0 => 1\n    _ => 2\n  end";
        let errors = check(src);
        assert!(errors.is_empty(), "match arms with same type should pass: {errors:?}");
    }

    #[test]
    fn exhaustiveness_warning_missing_variant() {
        let src = "enum Option\n  Some int\n  None\nend\n\
                   fn test\n  in x: int\n  do match x\n    0 => 1\n    _ => 2\n  end";
        let errors = check(src);
        assert!(errors.is_empty(), "int match with wildcard should pass: {errors:?}");
    }

    #[test]
    fn future_type_inference() {
        let mut sub = Substitution::new();
        let result = sub.unify(&Ty::Future(Box::new(Ty::Int)), &Ty::Future(Box::new(Ty::Int)));
        assert!(result.is_ok(), "Future<int> should unify with Future<int>");
    }

    #[test]
    fn channel_type_inference() {
        let mut sub = Substitution::new();
        let result = sub.unify(
            &Ty::Channel(Box::new(Ty::Int)),
            &Ty::Channel(Box::new(Ty::Int)),
        );
        assert!(result.is_ok(), "Channel<int> should unify with Channel<int>");
    }
}
