use crate::parser::ast::*;
use super::CodegenError;

pub struct WasmEmitter {
    output: String,
    indent: usize,
    local_count: usize,
    /// Imported function signatures for unresolved calls.
    imports: Vec<String>,
}

impl WasmEmitter {
    pub fn emit(program: &Program) -> Result<String, CodegenError> {
        let mut emitter = Self {
            output: String::new(),
            indent: 0,
            local_count: 0,
            imports: Vec::new(),
        };

        emitter.push("(module");
        emitter.indent += 1;

        for item in &program.items {
            if let Item::Function(f) = item {
                emitter.emit_function(f)?;
            }
        }

        for imp in emitter.imports.clone() {
            emitter.output = format!("{}\n{}", imp, emitter.output.split_once('\n').map(|(first, rest)| format!("{first}\n{rest}")).unwrap_or(emitter.output.clone()));
        }

        emitter.indent -= 1;
        emitter.push(")");

        Ok(emitter.output.clone())
    }

    fn emit_function(&mut self, func: &Function) -> Result<(), CodegenError> {
        self.local_count = 0;

        let params: Vec<String> = func.params.iter()
            .map(|p| format!("(param ${} {})", p.name, type_to_wasm(&p.ty)))
            .collect();

        let result = func.returns.as_ref()
            .map(|p| format!(" (result {})", type_to_wasm(&p.ty)))
            .unwrap_or_default();

        self.push(&format!("(func ${} {}{}", func.name, params.join(" "), result));
        self.indent += 1;

        self.emit_expr(&func.body, func)?;

        self.indent -= 1;
        self.push(")");

        self.push(&format!("(export \"{}\" (func ${}))", func.name, func.name));
        self.push("");

        Ok(())
    }

    fn emit_expr(&mut self, expr: &Expr, func: &Function) -> Result<(), CodegenError> {
        match expr {
            Expr::IntLit(n) => {
                self.push(&format!("i64.const {n}"));
            }
            Expr::FloatLit(n) => {
                self.push(&format!("f64.const {n}"));
            }
            Expr::BoolLit(b) => {
                self.push(&format!("i32.const {}", if *b { 1 } else { 0 }));
            }
            Expr::StrLit(_) => {
                self.push("i32.const 0 ;; string placeholder");
            }

            Expr::Ident(name) => {
                self.push(&format!("local.get ${name}"));
            }

            Expr::Op(op, args) => {
                for arg in args {
                    self.emit_expr(arg, func)?;
                }
                self.push(op_to_wasm(op, args));
            }

            Expr::Call(name, args) => {
                for arg in args {
                    self.emit_expr(arg, func)?;
                }
                self.push(&format!("call ${name}"));
            }

            Expr::Pipe(left, right) => {
                self.emit_expr(left, func)?;
                match right.as_ref() {
                    Expr::Call(name, extra_args) => {
                        for arg in extra_args {
                            self.emit_expr(arg, func)?;
                        }
                        self.push(&format!("call ${name}"));
                    }
                    _ => {
                        self.emit_expr(right, func)?;
                    }
                }
            }

            Expr::Field(base, _field) => {
                self.emit_expr(base, func)?;
                self.push(";; field access (struct support pending)");
            }

            Expr::Let { value, name, .. } => {
                self.emit_expr(value, func)?;
                self.push(&format!("local.set ${name}"));
            }

            Expr::If { condition, then_body, else_body, .. } => {
                self.emit_expr(condition, func)?;
                self.push("(if (result i64)");
                self.indent += 1;
                self.push("(then");
                self.indent += 1;
                for e in then_body {
                    self.emit_expr(e, func)?;
                }
                self.indent -= 1;
                self.push(")");
                self.push("(else");
                self.indent += 1;
                if let Some(body) = else_body {
                    for e in body {
                        self.emit_expr(e, func)?;
                    }
                } else {
                    self.push("i64.const 0");
                }
                self.indent -= 1;
                self.push(")");
                self.indent -= 1;
                self.push(")");
            }

            Expr::Each { iter, body, .. } => {
                self.push(";; each loop (runtime support pending)");
                self.emit_expr(iter, func)?;
                for e in body {
                    self.emit_expr(e, func)?;
                }
            }

            Expr::While { condition, body } => {
                self.push("(block $break");
                self.indent += 1;
                self.push("(loop $continue");
                self.indent += 1;

                self.emit_expr(condition, func)?;
                self.push("i32.eqz");
                self.push("br_if $break");

                for e in body {
                    self.emit_expr(e, func)?;
                }
                self.push("br $continue");

                self.indent -= 1;
                self.push(")");
                self.indent -= 1;
                self.push(")");
            }

            Expr::Block(exprs) => {
                for e in exprs {
                    self.emit_expr(e, func)?;
                }
            }

            Expr::Temporal(inner, _) => {
                self.emit_expr(inner, func)?;
            }
        }
        Ok(())
    }

    fn push(&mut self, line: &str) {
        let indent = "  ".repeat(self.indent);
        self.output.push_str(&indent);
        self.output.push_str(line);
        self.output.push('\n');
    }
}

fn type_to_wasm(ty: &Type) -> &'static str {
    match ty {
        Type::Named(name) => match name.as_str() {
            "int" | "uint" => "i64",
            "float" => "f64",
            "bool" => "i32",
            _ => "i64",
        },
        Type::List(_) | Type::Map(_, _) | Type::Optional(_) => "i64",
        Type::Tuple(_) => "i64",
    }
}

fn op_to_wasm(op: &Op, args: &[Expr]) -> &'static str {
    let _is_float = args.first().is_some_and(|a| matches!(a, Expr::FloatLit(_)));

    match op {
        Op::Add => "i64.add",
        Op::Sub => "i64.sub",
        Op::Mul => "i64.mul",
        Op::Div => "i64.div_s",
        Op::Modulo => "i64.rem_s",
        Op::Eq  => "i64.eq",
        Op::Neq => "i64.ne",
        Op::Gt  => "i64.gt_s",
        Op::Lt  => "i64.lt_s",
        Op::Gte => "i64.ge_s",
        Op::Lte => "i64.le_s",
        Op::And => "i32.and",
        Op::Or  => "i32.or",
        Op::Not => "i32.eqz",
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::lexer::Lexer;
    use crate::parser::Parser;

    fn emit(input: &str) -> String {
        let tokens = Lexer::new(input).tokenize().unwrap();
        let program = Parser::new(tokens).parse_program().unwrap();
        WasmEmitter::emit(&program).unwrap()
    }

    #[test]
    fn simple_function() {
        let wat = emit("fn add_one\n  in x: int\n  out result: int\n  do add x 1");
        assert!(wat.contains("(func $add_one"));
        assert!(wat.contains("(param $x i64)"));
        assert!(wat.contains("(result i64)"));
        assert!(wat.contains("i64.add"));
        assert!(wat.contains("(export \"add_one\""));
    }

    #[test]
    fn comparison() {
        let wat = emit("fn is_pos\n  in x: int\n  out r: bool\n  do gt x 0");
        assert!(wat.contains("i64.gt_s"));
    }

    #[test]
    fn pipe_chain() {
        let wat = emit("fn test\n  in nums: [int]\n  do filter nums gt 0 | reduce add");
        assert!(wat.contains("call $filter") || wat.contains("call $reduce"));
    }

    #[test]
    fn module_wrapping() {
        let wat = emit("fn noop\n  do 0");
        assert!(wat.starts_with("(module"));
        assert!(wat.trim().ends_with(')'));
    }
}
