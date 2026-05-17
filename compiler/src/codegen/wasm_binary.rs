use wasm_encoder::{
    CodeSection, ExportKind, ExportSection, FunctionSection, Ieee64,
    InstructionSink, Module, TypeSection, ValType, BlockType,
};
use wasm_encoder::Function as WasmFunction;
use crate::parser::ast::*;
use super::CodegenError;

pub struct WasmBinaryEmitter;

impl WasmBinaryEmitter {
    pub fn emit(program: &Program) -> Result<Vec<u8>, CodegenError> {
        let mut module = Module::new();

        let functions: Vec<&Function> = program
            .items
            .iter()
            .filter_map(|item| {
                if let Item::Function(f) = item {
                    Some(f)
                } else {
                    None
                }
            })
            .collect();

        // Type section: one signature per function
        let mut types = TypeSection::new();
        for func in &functions {
            let params: Vec<ValType> = func.params.iter().map(|p| type_to_valtype(&p.ty)).collect();
            let results: Vec<ValType> = func
                .returns
                .as_ref()
                .map(|p| vec![type_to_valtype(&p.ty)])
                .unwrap_or_default();
            types.ty().function(params, results);
        }
        module.section(&types);

        // Function section: map each function to its type index
        let mut func_section = FunctionSection::new();
        for (i, _) in functions.iter().enumerate() {
            func_section.function(i as u32);
        }
        module.section(&func_section);

        // Export section: export all functions
        let mut exports = ExportSection::new();
        for (i, func) in functions.iter().enumerate() {
            exports.export(&func.name, ExportKind::Func, i as u32);
        }
        module.section(&exports);

        // Code section: function bodies
        let mut codes = CodeSection::new();
        for (func_idx, func) in functions.iter().enumerate() {
            let mut emitter = FunctionEmitter::new(&functions, func_idx as u32);
            emitter.collect_locals(func);

            let locals: Vec<(u32, ValType)> = emitter
                .locals
                .iter()
                .map(|(_, vt)| (1u32, *vt))
                .collect();

            let mut f = WasmFunction::new(locals);
            let mut insn = f.instructions();
            emitter.emit_expr(&func.body, &mut insn)?;
            insn.end();
            drop(insn);
            codes.function(&f);
        }
        module.section(&codes);

        Ok(module.finish())
    }
}

struct FunctionEmitter<'a> {
    functions: &'a [&'a Function],
    current_func_idx: u32,
    /// Locals declared via `let` bindings (name -> local index)
    local_map: std::collections::HashMap<String, u32>,
    /// Locals to declare (beyond params)
    locals: Vec<(String, ValType)>,
    /// Number of params for the current function
    param_count: u32,
}

impl<'a> FunctionEmitter<'a> {
    fn new(functions: &'a [&'a Function], current_func_idx: u32) -> Self {
        Self {
            functions,
            current_func_idx,
            local_map: std::collections::HashMap::new(),
            locals: Vec::new(),
            param_count: 0,
        }
    }

    fn collect_locals(&mut self, func: &Function) {
        self.param_count = func.params.len() as u32;
        for (i, p) in func.params.iter().enumerate() {
            self.local_map.insert(p.name.clone(), i as u32);
        }
        self.collect_locals_from_expr(&func.body);
    }

    fn collect_locals_from_expr(&mut self, expr: &Expr) {
        match expr {
            Expr::Let { name, ty, value, .. } => {
                let vt = ty
                    .as_ref()
                    .map(|t| type_to_valtype(t))
                    .unwrap_or(Self::infer_valtype(value));
                let idx = self.param_count + self.locals.len() as u32;
                self.local_map.insert(name.clone(), idx);
                self.locals.push((name.clone(), vt));
                self.collect_locals_from_expr(value);
            }
            Expr::Block(exprs) => {
                for e in exprs {
                    self.collect_locals_from_expr(e);
                }
            }
            Expr::If { then_body, else_body, .. } => {
                for e in then_body {
                    self.collect_locals_from_expr(e);
                }
                if let Some(body) = else_body {
                    for e in body {
                        self.collect_locals_from_expr(e);
                    }
                }
            }
            Expr::While { body, .. } => {
                for e in body {
                    self.collect_locals_from_expr(e);
                }
            }
            _ => {}
        }
    }

    fn infer_valtype(expr: &Expr) -> ValType {
        match expr {
            Expr::FloatLit(_) => ValType::F64,
            Expr::BoolLit(_) => ValType::I32,
            _ => ValType::I64,
        }
    }

    fn emit_expr<'b>(
        &self,
        expr: &Expr,
        insn: &mut InstructionSink<'b>,
    ) -> Result<(), CodegenError> {
        match expr {
            Expr::IntLit(n) => {
                insn.i64_const(*n);
            }
            Expr::FloatLit(n) => {
                insn.f64_const(Ieee64::from(*n));
            }
            Expr::BoolLit(b) => {
                insn.i32_const(if *b { 1 } else { 0 });
            }
            Expr::StrLit(_) => {
                insn.i32_const(0);
            }
            Expr::Ident(name) => {
                let idx = self.local_map.get(name).copied().unwrap_or(0);
                insn.local_get(idx);
            }
            Expr::Op(op, args) => {
                for arg in args {
                    self.emit_expr(arg, insn)?;
                }
                self.emit_op(op, insn);
            }
            Expr::Call(name, args) => {
                for arg in args {
                    self.emit_expr(arg, insn)?;
                }
                let func_idx = self.resolve_func_index(name);
                insn.call(func_idx);
            }
            Expr::Pipe(left, right) => {
                self.emit_expr(left, insn)?;
                match right.as_ref() {
                    Expr::Call(name, extra_args) => {
                        for arg in extra_args {
                            self.emit_expr(arg, insn)?;
                        }
                        let func_idx = self.resolve_func_index(name);
                        insn.call(func_idx);
                    }
                    _ => {
                        self.emit_expr(right, insn)?;
                    }
                }
            }
            Expr::Let { name, value, .. } => {
                self.emit_expr(value, insn)?;
                let idx = self.local_map.get(name).copied().unwrap_or(0);
                insn.local_set(idx);
            }
            Expr::If { condition, then_body, else_body, .. } => {
                self.emit_expr(condition, insn)?;
                insn.if_(BlockType::Result(ValType::I64));
                for e in then_body {
                    self.emit_expr(e, insn)?;
                }
                insn.else_();
                if let Some(body) = else_body {
                    for e in body {
                        self.emit_expr(e, insn)?;
                    }
                } else {
                    insn.i64_const(0);
                }
                insn.end();
            }
            Expr::While { condition, body } => {
                insn.block(BlockType::Empty);
                insn.loop_(BlockType::Empty);
                self.emit_expr(condition, insn)?;
                insn.i32_eqz();
                insn.br_if(1);
                for e in body {
                    self.emit_expr(e, insn)?;
                }
                insn.br(0);
                insn.end();
                insn.end();
            }
            Expr::Block(exprs) => {
                for e in exprs {
                    self.emit_expr(e, insn)?;
                }
            }
            Expr::Field(base, _) => {
                self.emit_expr(base, insn)?;
            }
            Expr::Each { iter, body, .. } => {
                self.emit_expr(iter, insn)?;
                for e in body {
                    self.emit_expr(e, insn)?;
                }
            }
            Expr::Temporal(inner, _) => {
                self.emit_expr(inner, insn)?;
            }

            Expr::EnumVariant(_enum_name, _variant_name, args) => {
                if args.is_empty() {
                    insn.i64_const(0);
                } else {
                    self.emit_expr(&args[0], insn)?;
                }
            }

            Expr::Match { scrutinee, arms } => {
                self.emit_expr(scrutinee, insn)?;
                insn.drop();
                if let Some(first_arm) = arms.first() {
                    for e in &first_arm.body {
                        self.emit_expr(e, insn)?;
                    }
                } else {
                    insn.i64_const(0);
                }
            }

            Expr::Spawn(inner) => {
                self.emit_expr(inner, insn)?;
            }

            Expr::Await(inner) => {
                self.emit_expr(inner, insn)?;
            }

            Expr::Send(_, val) => {
                self.emit_expr(val, insn)?;
            }

            Expr::Recv(_) => {
                insn.i64_const(0);
            }
        }
        Ok(())
    }

    fn emit_op(&self, op: &Op, insn: &mut InstructionSink<'_>) {
        match op {
            Op::Add => { insn.i64_add(); }
            Op::Sub => { insn.i64_sub(); }
            Op::Mul => { insn.i64_mul(); }
            Op::Div => { insn.i64_div_s(); }
            Op::Modulo => { insn.i64_rem_s(); }
            Op::Eq => { insn.i64_eq(); }
            Op::Neq => { insn.i64_ne(); }
            Op::Gt => { insn.i64_gt_s(); }
            Op::Lt => { insn.i64_lt_s(); }
            Op::Gte => { insn.i64_ge_s(); }
            Op::Lte => { insn.i64_le_s(); }
            Op::And => { insn.i32_and(); }
            Op::Or => { insn.i32_or(); }
            Op::Not => { insn.i32_eqz(); }
        }
    }

    fn resolve_func_index(&self, name: &str) -> u32 {
        self.functions
            .iter()
            .position(|f| f.name == name)
            .map(|i| i as u32)
            .unwrap_or(self.current_func_idx)
    }
}

fn type_to_valtype(ty: &Type) -> ValType {
    match ty {
        Type::Named(name) => match name.as_str() {
            "int" | "uint" => ValType::I64,
            "float" => ValType::F64,
            "bool" => ValType::I32,
            _ => ValType::I64,
        },
        Type::List(_) | Type::Map(_, _) | Type::Optional(_) => ValType::I64,
        Type::Tuple(_) => ValType::I64,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::lexer::Lexer;
    use crate::parser::Parser;

    fn emit(input: &str) -> Vec<u8> {
        let tokens = Lexer::new(input).tokenize().unwrap();
        let program = Parser::new(tokens).parse_program().unwrap();
        WasmBinaryEmitter::emit(&program).unwrap()
    }

    #[test]
    fn wasm_magic_bytes() {
        let bytes = emit("fn noop\n  do 0");
        assert!(bytes.len() >= 8);
        assert_eq!(&bytes[0..4], &[0x00, 0x61, 0x73, 0x6D]);
    }

    #[test]
    fn simple_function_valid_structure() {
        let bytes = emit("fn add_one\n  in x: int\n  out result: int\n  do add x 1");
        // Starts with WASM magic + version
        assert_eq!(&bytes[0..4], &[0x00, 0x61, 0x73, 0x6D]);
        assert_eq!(&bytes[4..8], &[0x01, 0x00, 0x00, 0x00]);
        assert!(bytes.len() > 8);
    }

    #[test]
    fn multiple_functions_exported() {
        let src = "fn double\n  in x: int\n  out r: int\n  do mul x 2\n\nfn triple\n  in x: int\n  out r: int\n  do mul x 3";
        let bytes = emit(src);
        assert_eq!(&bytes[0..4], &[0x00, 0x61, 0x73, 0x6D]);
        // Both function names should appear in the binary (export section)
        let bytes_str = String::from_utf8_lossy(&bytes);
        assert!(bytes_str.contains("double"));
        assert!(bytes_str.contains("triple"));
    }
}
