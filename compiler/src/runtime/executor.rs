use crate::parser::ast::*;
use crate::runtime::{FluidRuntime, ResolverConfig, ResolverRequest, RuntimeError};
use std::collections::HashMap;

#[derive(Debug, Clone)]
pub enum Value {
    Int(i64),
    Float(f64),
    Bool(bool),
    Str(String),
    List(Vec<Value>),
    Void,
}

impl std::fmt::Display for Value {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Value::Int(n) => write!(f, "{n}"),
            Value::Float(n) => write!(f, "{n}"),
            Value::Bool(b) => write!(f, "{b}"),
            Value::Str(s) => write!(f, "{s}"),
            Value::List(items) => {
                let strs: Vec<String> = items.iter().map(|v| v.to_string()).collect();
                write!(f, "[{}]", strs.join(", "))
            }
            Value::Void => write!(f, "void"),
        }
    }
}

pub struct Executor {
    functions: HashMap<String, Function>,
    structs: HashMap<String, Vec<(String, Type)>>,
    resolver: FluidRuntime,
    output: Vec<String>,
}

impl Executor {
    pub fn new(config: ResolverConfig) -> Self {
        Self {
            functions: HashMap::new(),
            structs: HashMap::new(),
            resolver: FluidRuntime::new(config),
            output: Vec::new(),
        }
    }

    pub fn load(&mut self, program: &Program) {
        for item in &program.items {
            match item {
                Item::Function(f) => {
                    self.functions.insert(f.name.clone(), f.clone());
                }
                Item::Struct(s) => {
                    let fields: Vec<_> = s
                        .fields
                        .iter()
                        .map(|p| (p.name.clone(), p.ty.clone()))
                        .collect();
                    self.structs.insert(s.name.clone(), fields);
                }
                _ => {}
            }
        }
    }

    pub fn call(&mut self, name: &str, args: Vec<Value>) -> Result<Value, RuntimeError> {
        let func = self
            .functions
            .get(name)
            .cloned()
            .ok_or_else(|| RuntimeError {
                message: format!("undefined function '{name}'"),
            })?;

        if func.mode == FnMode::Fluid {
            return self.resolve_fluid(&func, &args);
        }

        let mut env: HashMap<String, Value> = HashMap::new();
        for (param, arg) in func.params.iter().zip(args.iter()) {
            env.insert(param.name.clone(), arg.clone());
        }

        self.eval_expr(&func.body, &mut env)
    }

    fn resolve_fluid(&mut self, func: &Function, args: &[Value]) -> Result<Value, RuntimeError> {
        let intent = func.intent.as_deref().unwrap_or(&func.name);
        let threshold = func.confidence.unwrap_or(0.8);

        let params: Vec<(String, String)> = func
            .params
            .iter()
            .zip(args.iter())
            .map(|(p, v)| (p.name.clone(), v.to_string()))
            .collect();

        let request = ResolverRequest {
            intent: intent.to_string(),
            params,
            confidence_threshold: threshold,
        };

        let response = self.resolver.resolve(&request)?;

        if response.used_fallback {
            if let Some(ref fallback_expr) = func.fallback {
                let mut env: HashMap<String, Value> = HashMap::new();
                for (param, arg) in func.params.iter().zip(args.iter()) {
                    env.insert(param.name.clone(), arg.clone());
                }
                return self.eval_expr(fallback_expr, &mut env);
            }
        }

        Ok(Value::Str(response.result))
    }

    fn eval_expr(
        &mut self,
        expr: &Expr,
        env: &mut HashMap<String, Value>,
    ) -> Result<Value, RuntimeError> {
        match expr {
            Expr::IntLit(n) => Ok(Value::Int(*n)),
            Expr::FloatLit(n) => Ok(Value::Float(*n)),
            Expr::StrLit(s) => Ok(Value::Str(s.clone())),
            Expr::BoolLit(b) => Ok(Value::Bool(*b)),

            Expr::Ident(name) => env.get(name).cloned().ok_or_else(|| RuntimeError {
                message: format!("undefined variable '{name}'"),
            }),

            Expr::Op(op, args) => self.eval_op(op, args, env),

            Expr::Call(name, call_args) => {
                let mut vals = Vec::new();
                for arg in call_args {
                    vals.push(self.eval_expr(arg, env)?);
                }
                self.call_builtin_or_fn(name, vals)
            }

            Expr::Pipe(left, right) => {
                let left_val = self.eval_expr(left, env)?;
                match right.as_ref() {
                    Expr::Call(name, extra_args) => {
                        let mut vals = vec![left_val];
                        for arg in extra_args {
                            vals.push(self.eval_expr(arg, env)?);
                        }
                        self.call_builtin_or_fn(name, vals)
                    }
                    _ => self.eval_expr(right, env),
                }
            }

            Expr::Let { name, value, .. } => {
                let val = self.eval_expr(value, env)?;
                env.insert(name.clone(), val.clone());
                Ok(val)
            }

            Expr::If {
                condition,
                then_body,
                elif_branches,
                else_body,
            } => {
                let cond_val = self.eval_expr(condition, env)?;
                if self.is_truthy(&cond_val) {
                    return self.eval_block(then_body, env);
                }
                for (elif_cond, elif_body) in elif_branches {
                    let elif_val = self.eval_expr(elif_cond, env)?;
                    if self.is_truthy(&elif_val) {
                        return self.eval_block(elif_body, env);
                    }
                }
                if let Some(body) = else_body {
                    self.eval_block(body, env)
                } else {
                    Ok(Value::Void)
                }
            }

            Expr::Each {
                binding,
                iter,
                body,
            } => {
                let iter_val = self.eval_expr(iter, env)?;
                if let Value::List(items) = iter_val {
                    for item in items {
                        env.insert(binding.clone(), item);
                        self.eval_block(body, env)?;
                    }
                }
                Ok(Value::Void)
            }

            Expr::While { condition, body } => {
                loop {
                    let cond = self.eval_expr(condition, env)?;
                    if !self.is_truthy(&cond) {
                        break;
                    }
                    self.eval_block(body, env)?;
                }
                Ok(Value::Void)
            }

            Expr::Block(exprs) => self.eval_block(exprs, env),

            Expr::Field(_, _) | Expr::Temporal(_, _) => Ok(Value::Void),
        }
    }

    fn is_truthy(&self, val: &Value) -> bool {
        match val {
            Value::Bool(b) => *b,
            Value::Int(n) => *n != 0,
            _ => false,
        }
    }

    fn eval_block(
        &mut self,
        exprs: &[Expr],
        env: &mut HashMap<String, Value>,
    ) -> Result<Value, RuntimeError> {
        let mut last = Value::Void;
        for e in exprs {
            last = self.eval_expr(e, env)?;
        }
        Ok(last)
    }

    fn eval_op(
        &mut self,
        op: &Op,
        args: &[Expr],
        env: &mut HashMap<String, Value>,
    ) -> Result<Value, RuntimeError> {
        let vals: Vec<Value> = args
            .iter()
            .map(|a| self.eval_expr(a, env))
            .collect::<Result<_, _>>()?;

        match (op, vals.as_slice()) {
            (Op::Add, [Value::Int(a), Value::Int(b)]) => Ok(Value::Int(a + b)),
            (Op::Sub, [Value::Int(a), Value::Int(b)]) => Ok(Value::Int(a - b)),
            (Op::Mul, [Value::Int(a), Value::Int(b)]) => Ok(Value::Int(a * b)),
            (Op::Div, [Value::Int(a), Value::Int(b)]) if *b != 0 => Ok(Value::Int(a / b)),
            (Op::Modulo, [Value::Int(a), Value::Int(b)]) if *b != 0 => Ok(Value::Int(a % b)),

            (Op::Add, [Value::Float(a), Value::Float(b)]) => Ok(Value::Float(a + b)),
            (Op::Sub, [Value::Float(a), Value::Float(b)]) => Ok(Value::Float(a - b)),
            (Op::Mul, [Value::Float(a), Value::Float(b)]) => Ok(Value::Float(a * b)),
            (Op::Div, [Value::Float(a), Value::Float(b)]) => Ok(Value::Float(a / b)),

            (Op::Eq, [a, b]) => Ok(Value::Bool(format!("{a}") == format!("{b}"))),
            (Op::Neq, [a, b]) => Ok(Value::Bool(format!("{a}") != format!("{b}"))),
            (Op::Gt, [Value::Int(a), Value::Int(b)]) => Ok(Value::Bool(a > b)),
            (Op::Lt, [Value::Int(a), Value::Int(b)]) => Ok(Value::Bool(a < b)),
            (Op::Gte, [Value::Int(a), Value::Int(b)]) => Ok(Value::Bool(a >= b)),
            (Op::Lte, [Value::Int(a), Value::Int(b)]) => Ok(Value::Bool(a <= b)),

            (Op::And, [Value::Bool(a), Value::Bool(b)]) => Ok(Value::Bool(*a && *b)),
            (Op::Or, [Value::Bool(a), Value::Bool(b)]) => Ok(Value::Bool(*a || *b)),
            (Op::Not, [Value::Bool(a)]) => Ok(Value::Bool(!a)),

            _ => Ok(Value::Void),
        }
    }

    fn call_builtin_or_fn(&mut self, name: &str, args: Vec<Value>) -> Result<Value, RuntimeError> {
        match name {
            "print" => {
                if let Some(v) = args.first() {
                    self.output.push(v.to_string());
                    println!("{v}");
                }
                Ok(Value::Void)
            }
            "len" => match args.first() {
                Some(Value::List(l)) => Ok(Value::Int(l.len() as i64)),
                Some(Value::Str(s)) => Ok(Value::Int(s.len() as i64)),
                _ => Ok(Value::Int(0)),
            },
            "abs" => match args.first() {
                Some(Value::Int(n)) => Ok(Value::Int(n.abs())),
                Some(Value::Float(n)) => Ok(Value::Float(n.abs())),
                _ => Ok(Value::Int(0)),
            },
            "max" => match (args.first(), args.get(1)) {
                (Some(Value::Int(a)), Some(Value::Int(b))) => Ok(Value::Int(*a.max(b))),
                _ => Ok(Value::Int(0)),
            },
            "min" => match (args.first(), args.get(1)) {
                (Some(Value::Int(a)), Some(Value::Int(b))) => Ok(Value::Int(*a.min(b))),
                _ => Ok(Value::Int(0)),
            },
            "range" => match (args.first(), args.get(1)) {
                (Some(Value::Int(a)), Some(Value::Int(b))) => {
                    Ok(Value::List((*a..*b).map(Value::Int).collect()))
                }
                _ => Ok(Value::List(vec![])),
            },
            "sqrt" => match args.first() {
                Some(Value::Float(n)) => Ok(Value::Float(n.sqrt())),
                Some(Value::Int(n)) => Ok(Value::Float((*n as f64).sqrt())),
                _ => Ok(Value::Float(0.0)),
            },
            _ => self.call(name, args),
        }
    }

    pub fn get_output(&self) -> &[String] {
        &self.output
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    fn stub_config() -> ResolverConfig {
        ResolverConfig {
            endpoint: "http://localhost:8080/v1/resolve".into(),
            model: "gpt-4".into(),
            timeout_ms: 5000,
            max_retries: 3,
        }
    }

    fn make_program(items: Vec<Item>) -> Program {
        Program { items }
    }

    fn strict_fn(name: &str, params: Vec<Param>, body: Expr) -> Function {
        Function {
            name: name.into(),
            params,
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
        }
    }

    fn int_param(name: &str) -> Param {
        Param {
            name: name.into(),
            ty: Type::Named("int".into()),
        }
    }

    #[test]
    fn eval_integer_arithmetic() {
        let mut exec = Executor::new(stub_config());
        let body = Expr::Op(Op::Add, vec![Expr::Ident("a".into()), Expr::Ident("b".into())]);
        let func = strict_fn("add", vec![int_param("a"), int_param("b")], body);
        let prog = make_program(vec![Item::Function(func)]);
        exec.load(&prog);

        let result = exec.call("add", vec![Value::Int(1), Value::Int(2)]).unwrap();
        assert!(matches!(result, Value::Int(3)));
    }

    #[test]
    fn eval_comparison() {
        let mut exec = Executor::new(stub_config());
        let body = Expr::Op(Op::Gt, vec![Expr::Ident("a".into()), Expr::Ident("b".into())]);
        let func = strict_fn("cmp", vec![int_param("a"), int_param("b")], body);
        let prog = make_program(vec![Item::Function(func)]);
        exec.load(&prog);

        let result = exec.call("cmp", vec![Value::Int(5), Value::Int(3)]).unwrap();
        assert!(matches!(result, Value::Bool(true)));
    }

    #[test]
    fn eval_function_call() {
        let mut exec = Executor::new(stub_config());

        let add_one_body = Expr::Op(
            Op::Add,
            vec![Expr::Ident("x".into()), Expr::IntLit(1)],
        );
        let add_one = strict_fn("add_one", vec![int_param("x")], add_one_body);

        let main_body = Expr::Call("add_one".into(), vec![Expr::IntLit(10)]);
        let main_fn = strict_fn("main", vec![], main_body);

        let prog = make_program(vec![Item::Function(add_one), Item::Function(main_fn)]);
        exec.load(&prog);

        let result = exec.call("main", vec![]).unwrap();
        assert!(matches!(result, Value::Int(11)));
    }

    #[test]
    fn eval_if_expression() {
        let mut exec = Executor::new(stub_config());

        let body = Expr::If {
            condition: Box::new(Expr::Op(
                Op::Gt,
                vec![Expr::Ident("x".into()), Expr::IntLit(0)],
            )),
            then_body: vec![Expr::StrLit("positive".into())],
            elif_branches: vec![],
            else_body: Some(vec![Expr::StrLit("non-positive".into())]),
        };
        let func = strict_fn("check", vec![int_param("x")], body);
        let prog = make_program(vec![Item::Function(func)]);
        exec.load(&prog);

        let pos = exec.call("check", vec![Value::Int(5)]).unwrap();
        assert!(matches!(pos, Value::Str(ref s) if s == "positive"));

        let neg = exec.call("check", vec![Value::Int(-1)]).unwrap();
        assert!(matches!(neg, Value::Str(ref s) if s == "non-positive"));
    }

    #[test]
    fn eval_pipe_chain() {
        let mut exec = Executor::new(stub_config());

        let negate_body = Expr::Op(
            Op::Sub,
            vec![Expr::IntLit(0), Expr::Ident("x".into())],
        );
        let negate = strict_fn("negate", vec![int_param("x")], negate_body);

        let main_body = Expr::Pipe(
            Box::new(Expr::IntLit(5)),
            Box::new(Expr::Call("negate".into(), vec![])),
        );
        let main_fn = strict_fn("main", vec![], main_body);

        let prog = make_program(vec![Item::Function(negate), Item::Function(main_fn)]);
        exec.load(&prog);

        let result = exec.call("main", vec![]).unwrap();
        assert!(matches!(result, Value::Int(-5)));
    }

    #[test]
    fn eval_let_binding() {
        let mut exec = Executor::new(stub_config());

        let body = Expr::Block(vec![
            Expr::Let {
                name: "x".into(),
                ty: Some(Type::Named("int".into())),
                mutable: false,
                value: Box::new(Expr::IntLit(42)),
            },
            Expr::Ident("x".into()),
        ]);
        let func = strict_fn("main", vec![], body);
        let prog = make_program(vec![Item::Function(func)]);
        exec.load(&prog);

        let result = exec.call("main", vec![]).unwrap();
        assert!(matches!(result, Value::Int(42)));
    }

    #[test]
    fn eval_builtins() {
        let mut exec = Executor::new(stub_config());

        // max
        let max_body = Expr::Call("max".into(), vec![Expr::Ident("a".into()), Expr::Ident("b".into())]);
        let max_fn = strict_fn("test_max", vec![int_param("a"), int_param("b")], max_body);

        // min
        let min_body = Expr::Call("min".into(), vec![Expr::Ident("a".into()), Expr::Ident("b".into())]);
        let min_fn = strict_fn("test_min", vec![int_param("a"), int_param("b")], min_body);

        // abs
        let abs_body = Expr::Call("abs".into(), vec![Expr::Ident("x".into())]);
        let abs_fn = strict_fn("test_abs", vec![int_param("x")], abs_body);

        // len
        let len_body = Expr::Call(
            "len".into(),
            vec![Expr::Call("range".into(), vec![Expr::IntLit(0), Expr::IntLit(5)])],
        );
        let len_fn = strict_fn("test_len", vec![], len_body);

        // range
        let range_body = Expr::Call("range".into(), vec![Expr::IntLit(0), Expr::IntLit(3)]);
        let range_fn = strict_fn("test_range", vec![], range_body);

        let prog = make_program(vec![
            Item::Function(max_fn),
            Item::Function(min_fn),
            Item::Function(abs_fn),
            Item::Function(len_fn),
            Item::Function(range_fn),
        ]);
        exec.load(&prog);

        let max_r = exec.call("test_max", vec![Value::Int(3), Value::Int(7)]).unwrap();
        assert!(matches!(max_r, Value::Int(7)));

        let min_r = exec.call("test_min", vec![Value::Int(3), Value::Int(7)]).unwrap();
        assert!(matches!(min_r, Value::Int(3)));

        let abs_r = exec.call("test_abs", vec![Value::Int(-4)]).unwrap();
        assert!(matches!(abs_r, Value::Int(4)));

        let len_r = exec.call("test_len", vec![]).unwrap();
        assert!(matches!(len_r, Value::Int(5)));

        let range_r = exec.call("test_range", vec![]).unwrap();
        assert!(matches!(range_r, Value::List(ref v) if v.len() == 3));
    }

    #[test]
    fn eval_fluid_uses_resolver() {
        let mut exec = Executor::new(stub_config());

        let func = Function {
            name: "classify".into(),
            params: vec![Param {
                name: "text".into(),
                ty: Type::Named("str".into()),
            }],
            returns: None,
            invariants: vec![],
            requires: vec![],
            ensures: vec![],
            mode: FnMode::Fluid,
            intent: Some("classify the input text".into()),
            confidence: Some(0.85),
            fallback: None,
            guarantee: None,
            body: Expr::StrLit("placeholder".into()),
        };

        let prog = make_program(vec![Item::Function(func)]);
        exec.load(&prog);

        let result = exec
            .call("classify", vec![Value::Str("hello".into())])
            .unwrap();
        // Stub resolver returns "stub_result(...)" for high-confidence requests
        assert!(matches!(result, Value::Str(ref s) if s.contains("stub_result")));
    }

    #[test]
    fn eval_output_collection() {
        let mut exec = Executor::new(stub_config());

        let body = Expr::Block(vec![
            Expr::Call("print".into(), vec![Expr::StrLit("hello".into())]),
            Expr::Call("print".into(), vec![Expr::StrLit("world".into())]),
        ]);
        let func = strict_fn("main", vec![], body);
        let prog = make_program(vec![Item::Function(func)]);
        exec.load(&prog);

        exec.call("main", vec![]).unwrap();
        let output = exec.get_output();
        assert_eq!(output, &["hello", "world"]);
    }
}
