use crate::parser::ast::*;
use crate::driver::targets::CompileTarget;
use super::CodegenError;
use super::optimize;
use super::debug_info::DebugInfoEmitter;

pub struct LlvmEmitter {
    output: String,
    reg: usize,
    /// External functions that need to be declared.
    externs: Vec<String>,
    debug: bool,
    debug_info: Option<DebugInfoEmitter>,
    current_dbg_scope: Option<usize>,
    current_line: usize,
}

impl LlvmEmitter {
    pub fn emit(program: &Program) -> Result<String, CodegenError> {
        let mut emitter = Self {
            output: String::new(),
            reg: 0,
            externs: Vec::new(),
            debug: false,
            debug_info: None,
            current_dbg_scope: None,
            current_line: 0,
        };

        emitter.push("; ModuleID = 'kernl'");
        emitter.push("source_filename = \"kernl\"");
        emitter.push("");

        for item in &program.items {
            match item {
                Item::Function(f) => emitter.emit_function(f)?,
                Item::Struct(s) => emitter.emit_struct(s),
                Item::Enum(e) => emitter.emit_enum(e),
                _ => {}
            }
        }

        let mut result = String::new();
        for ext in &emitter.externs {
            result.push_str(ext);
            result.push('\n');
        }
        if !emitter.externs.is_empty() {
            result.push('\n');
        }
        result.push_str(&emitter.output);

        Ok(result)
    }

    pub fn emit_for_target(program: &Program, target: &CompileTarget) -> Result<String, CodegenError> {
        let mut emitter = Self {
            output: String::new(),
            reg: 0,
            externs: Vec::new(),
            debug: false,
            debug_info: None,
            current_dbg_scope: None,
            current_line: 0,
        };

        emitter.push("; ModuleID = 'kernl'");
        emitter.push("source_filename = \"kernl\"");
        emitter.push(&format!("target datalayout = \"{}\"", target.data_layout()));
        emitter.push(&format!("target triple = \"{}\"", target.llvm_triple()));
        emitter.push("");

        for item in &program.items {
            match item {
                Item::Function(f) => emitter.emit_function(f)?,
                Item::Struct(s) => emitter.emit_struct(s),
                Item::Enum(e) => emitter.emit_enum(e),
                _ => {}
            }
        }

        let mut result = String::new();
        for ext in &emitter.externs {
            result.push_str(ext);
            result.push('\n');
        }
        if !emitter.externs.is_empty() {
            result.push('\n');
        }
        result.push_str(&emitter.output);

        Ok(result)
    }

    pub fn emit_with_debug(program: &Program, file_name: &str, directory: &str) -> Result<String, CodegenError> {
        let mut dbg = DebugInfoEmitter::new(file_name, directory);
        dbg.emit_compile_unit();

        let mut emitter = Self {
            output: String::new(),
            reg: 0,
            externs: Vec::new(),
            debug: true,
            debug_info: Some(dbg),
            current_dbg_scope: None,
            current_line: 0,
        };

        emitter.push("; ModuleID = 'kernl'");
        emitter.push("source_filename = \"kernl\"");
        emitter.push("");

        let mut func_line = 1;
        for item in &program.items {
            match item {
                Item::Function(f) => {
                    emitter.current_line = func_line;
                    emitter.emit_function(f)?;
                    func_line += 10;
                }
                Item::Struct(s) => {
                    emitter.emit_struct(s);
                    func_line += 3;
                }
                Item::Enum(e) => {
                    emitter.emit_enum(e);
                    func_line += e.variants.len() + 2;
                }
                _ => { func_line += 1; }
            }
        }

        let mut result = String::new();
        for ext in &emitter.externs {
            result.push_str(ext);
            result.push('\n');
        }
        if !emitter.externs.is_empty() {
            result.push('\n');
        }
        result.push_str(&emitter.output);

        if let Some(di) = &emitter.debug_info {
            result.push_str(&di.finish());
        }

        Ok(result)
    }

    fn emit_struct(&mut self, s: &StructDef) {
        let fields: Vec<String> = s.fields.iter()
            .map(|f| type_to_llvm(&f.ty))
            .collect();
        self.push(&format!("%{} = type {{ {} }}", s.name, fields.join(", ")));
        self.push("");
    }

    fn emit_enum(&mut self, e: &EnumDef) {
        self.push(&format!("; enum {} — tagged union: {{i8, i64}}", e.name));
        self.push(&format!("%{} = type {{ i8, i64 }}", e.name));
        self.push("");
    }

    fn emit_function(&mut self, func: &Function) -> Result<(), CodegenError> {
        self.reg = 0;
        let ret_ty = func.returns.as_ref()
            .map(|p| type_to_llvm(&p.ty))
            .unwrap_or_else(|| "void".to_string());

        let params: Vec<String> = func.params.iter()
            .map(|p| format!("{} %{}", type_to_llvm(&p.ty), p.name))
            .collect();

        if self.debug {
            let file_id = self.debug_info.as_ref().map(|d| d.file_id()).unwrap_or(1);
            let sp_id = self.debug_info.as_mut()
                .map(|d| d.emit_subprogram(&func.name, self.current_line, file_id))
                .unwrap_or(0);
            self.current_dbg_scope = Some(sp_id);
            self.push(&format!(
                "define {} @{}({}) !dbg !{sp_id} {{",
                ret_ty, func.name, params.join(", ")
            ));
        } else {
            self.push(&format!("define {} @{}({}) {{", ret_ty, func.name, params.join(", ")));
        }
        self.push("entry:");

        let result_reg = self.emit_expr(&func.body, func)?;

        if ret_ty == "void" {
            self.push("  ret void");
        } else {
            self.push(&format!("  ret {} {}", ret_ty, result_reg));
        }

        self.push("}");
        self.push("");
        self.current_dbg_scope = None;
        Ok(())
    }

    fn emit_expr(&mut self, expr: &Expr, func: &Function) -> Result<String, CodegenError> {
        match expr {
            Expr::IntLit(n) => Ok(n.to_string()),
            Expr::FloatLit(n) => Ok(format!("{n:e}")),
            Expr::BoolLit(b) => Ok(if *b { "1".into() } else { "0".into() }),
            Expr::StrLit(_) => Ok("zeroinitializer".into()),

            Expr::Ident(name) => {
                if func.params.iter().any(|p| p.name == *name) {
                    Ok(format!("%{name}"))
                } else if func.returns.as_ref().is_some_and(|r| r.name == *name) {
                    Ok(format!("%{name}"))
                } else {
                    Ok(format!("%{name}"))
                }
            }

            Expr::Op(op, args) => self.emit_op(op, args, func),

            Expr::Call(name, args) => {
                let mut arg_regs = Vec::new();
                for arg in args {
                    arg_regs.push(self.emit_expr(arg, func)?);
                }

                if let Some(intrinsic) = optimize::is_llvm_intrinsic(name) {
                    return self.emit_intrinsic_call(intrinsic, name, &arg_regs);
                }

                if optimize::is_inline_builtin(name) && arg_regs.len() == 2 {
                    return self.emit_inline_builtin(name, &arg_regs);
                }

                let ret_ty = self.lookup_return_type(name, func);
                let arg_strs: Vec<String> = arg_regs.iter()
                    .map(|r| format!("i64 {r}"))
                    .collect();

                let decl = format!("declare {} @{}({})",
                    ret_ty,
                    name,
                    arg_strs.iter().map(|a| {
                        a.split_whitespace().next().unwrap_or("i64").to_string()
                    }).collect::<Vec<_>>().join(", ")
                );
                if !self.externs.iter().any(|e| e.contains(&format!("@{name}("))) {
                    self.externs.push(decl);
                }

                let reg = self.next_reg();
                self.push_dbg(&format!("  {} = call {} @{}({})",
                    reg, ret_ty, name, arg_strs.join(", ")));
                Ok(reg)
            }

            Expr::Pipe(left, right) => {
                let left_reg = self.emit_expr(left, func)?;
                match right.as_ref() {
                    Expr::Call(name, extra_args) => {
                        let mut all_arg_regs = vec![left_reg];
                        for arg in extra_args {
                            all_arg_regs.push(self.emit_expr(arg, func)?);
                        }

                        let ret_ty = self.lookup_return_type(name, func);
                        let arg_strs: Vec<String> = all_arg_regs.iter()
                            .map(|r| format!("i64 {r}"))
                            .collect();

                        let decl = format!("declare {} @{}({})",
                            ret_ty, name,
                            vec!["i64"; all_arg_regs.len()].join(", ")
                        );
                        if !self.externs.iter().any(|e| e.contains(&format!("@{name}("))) {
                            self.externs.push(decl);
                        }

                        let reg = self.next_reg();
                        self.push_dbg(&format!("  {} = call {} @{}({})",
                            reg, ret_ty, name, arg_strs.join(", ")));
                        Ok(reg)
                    }
                    _ => self.emit_expr(right, func),
                }
            }

            Expr::Field(base, field_name) => {
                let base_reg = self.emit_expr(base, func)?;
                let reg = self.next_reg();
                self.push(&format!("  {} = extractvalue %struct {}, ; .{}", reg, base_reg, field_name));
                Ok(reg)
            }

            Expr::Let { name, value, .. } => {
                let val_reg = self.emit_expr(value, func)?;
                self.push_dbg(&format!("  %{name} = add i64 {val_reg}, 0"));
                Ok(format!("%{name}"))
            }

            Expr::If { condition, then_body, else_body, .. } => {
                let cond_reg = self.emit_expr(condition, func)?;
                let then_label = format!("then_{}", self.reg);
                let else_label = format!("else_{}", self.reg);
                let merge_label = format!("merge_{}", self.reg);

                self.push(&format!("  br i1 {cond_reg}, label %{then_label}, label %{else_label}"));

                self.push(&format!("{then_label}:"));
                let mut then_reg = "0".to_string();
                for expr in then_body {
                    then_reg = self.emit_expr(expr, func)?;
                }
                self.push(&format!("  br label %{merge_label}"));

                self.push(&format!("{else_label}:"));
                let mut else_reg = "0".to_string();
                if let Some(body) = else_body {
                    for expr in body {
                        else_reg = self.emit_expr(expr, func)?;
                    }
                }
                self.push(&format!("  br label %{merge_label}"));

                self.push(&format!("{merge_label}:"));
                let result = self.next_reg();
                self.push(&format!("  {result} = phi i64 [{then_reg}, %{then_label}], [{else_reg}, %{else_label}]"));
                Ok(result)
            }

            Expr::Block(exprs) => {
                let mut last = "0".to_string();
                for e in exprs {
                    last = self.emit_expr(e, func)?;
                }
                Ok(last)
            }

            Expr::EnumVariant(_enum_name, _variant_name, args) => {
                let tag_val = "0";
                let payload = if args.is_empty() {
                    "0".to_string()
                } else {
                    self.emit_expr(&args[0], func)?
                };
                let r1 = self.next_reg();
                self.push(&format!("  {r1} = insertvalue {{i8, i64}} undef, i8 {tag_val}, 0"));
                let r2 = self.next_reg();
                self.push(&format!("  {r2} = insertvalue {{i8, i64}} {r1}, i64 {payload}, 1"));
                Ok(r2)
            }

            Expr::Match { scrutinee, arms } => {
                let scrut_reg = self.emit_expr(scrutinee, func)?;
                let tag_reg = self.next_reg();
                self.push(&format!("  {tag_reg} = extractvalue {{i8, i64}} {scrut_reg}, 0"));
                let merge_label = format!("match_merge_{}", self.reg);
                let mut arm_labels = Vec::new();
                let default_label = format!("match_default_{}", self.reg);

                for (i, _arm) in arms.iter().enumerate() {
                    arm_labels.push(format!("match_arm_{}_{}", self.reg, i));
                }

                let mut switch_cases = String::new();
                for (i, label) in arm_labels.iter().enumerate() {
                    switch_cases.push_str(&format!(" i8 {i}, label %{label}"));
                }
                self.push(&format!("  switch i8 {tag_reg}, label %{default_label} [{switch_cases} ]"));

                let mut phi_entries = Vec::new();
                for (i, arm) in arms.iter().enumerate() {
                    let label = &arm_labels[i];
                    self.push(&format!("{label}:"));
                    let mut last_reg = "0".to_string();
                    for expr in &arm.body {
                        last_reg = self.emit_expr(expr, func)?;
                    }
                    phi_entries.push((last_reg, label.clone()));
                    self.push(&format!("  br label %{merge_label}"));
                }

                self.push(&format!("{default_label}:"));
                self.push(&format!("  br label %{merge_label}"));
                phi_entries.push(("0".to_string(), default_label));

                self.push(&format!("{merge_label}:"));
                let result = self.next_reg();
                let phi_args: Vec<String> = phi_entries.iter()
                    .map(|(reg, label)| format!("[{reg}, %{label}]"))
                    .collect();
                self.push(&format!("  {result} = phi i64 {}", phi_args.join(", ")));
                Ok(result)
            }

            Expr::Spawn(inner) => self.emit_expr(inner, func),
            Expr::Await(inner) => self.emit_expr(inner, func),
            Expr::Send(_, val) => self.emit_expr(val, func),
            Expr::Recv(_) => Ok("0".into()),

            _ => Ok("0".into()),
        }
    }

    fn emit_op(&mut self, op: &Op, args: &[Expr], func: &Function) -> Result<String, CodegenError> {
        if args.len() == 2 {
            let lhs = self.emit_expr(&args[0], func)?;
            let rhs = self.emit_expr(&args[1], func)?;
            let reg = self.next_reg();
            let inst = match op {
                Op::Add => "add i64",
                Op::Sub => "sub i64",
                Op::Mul => "mul i64",
                Op::Div => "sdiv i64",
                Op::Modulo => "srem i64",
                Op::Eq  => "icmp eq i64",
                Op::Neq => "icmp ne i64",
                Op::Gt  => "icmp sgt i64",
                Op::Lt  => "icmp slt i64",
                Op::Gte => "icmp sge i64",
                Op::Lte => "icmp sle i64",
                Op::And => "and i1",
                Op::Or  => "or i1",
                Op::Not => unreachable!(),
            };
            self.push_dbg(&format!("  {reg} = {inst} {lhs}, {rhs}"));
            Ok(reg)
        } else if args.len() == 1 && *op == Op::Not {
            let operand = self.emit_expr(&args[0], func)?;
            let reg = self.next_reg();
            self.push_dbg(&format!("  {reg} = xor i1 {operand}, 1"));
            Ok(reg)
        } else {
            Ok("0".into())
        }
    }

    fn emit_intrinsic_call(&mut self, intrinsic: &str, builtin_name: &str, arg_regs: &[String]) -> Result<String, CodegenError> {
        let reg = self.next_reg();
        match builtin_name {
            "abs" => {
                let a = &arg_regs[0];
                let decl = format!("declare i64 @{intrinsic}(i64, i1)");
                if !self.externs.iter().any(|e| e.contains(&format!("@{intrinsic}("))) {
                    self.externs.push(decl);
                }
                self.push_dbg(&format!("  {reg} = call i64 @{intrinsic}(i64 {a}, i1 1)"));
            }
            "sqrt" => {
                let a = &arg_regs[0];
                let decl = format!("declare double @{intrinsic}(double)");
                if !self.externs.iter().any(|e| e.contains(&format!("@{intrinsic}("))) {
                    self.externs.push(decl);
                }
                self.push_dbg(&format!("  {reg} = call double @{intrinsic}(double {a})"));
            }
            _ => {
                self.push_dbg(&format!("  {reg} = add i64 0, 0 ; unsupported intrinsic"));
            }
        }
        Ok(reg)
    }

    fn emit_inline_builtin(&mut self, name: &str, arg_regs: &[String]) -> Result<String, CodegenError> {
        let a = &arg_regs[0];
        let b = &arg_regs[1];
        let cmp_reg = self.next_reg();
        let cmp = match name {
            "max" => "sgt",
            "min" => "slt",
            _ => unreachable!(),
        };
        self.push_dbg(&format!("  {cmp_reg} = icmp {cmp} i64 {a}, {b}"));
        let sel_reg = self.next_reg();
        self.push_dbg(&format!("  {sel_reg} = select i1 {cmp_reg}, i64 {a}, i64 {b}"));
        Ok(sel_reg)
    }

    fn lookup_return_type(&self, _name: &str, _func: &Function) -> String {
        "i64".to_string()
    }

    fn next_reg(&mut self) -> String {
        self.reg += 1;
        format!("%{}", self.reg)
    }

    fn push(&mut self, line: &str) {
        self.output.push_str(line);
        self.output.push('\n');
    }

    /// Push an instruction line with debug location attached (if debug is enabled).
    fn push_dbg(&mut self, line: &str) {
        if self.debug {
            if let Some(scope_id) = self.current_dbg_scope {
                self.current_line += 1;
                let loc_id = self.debug_info.as_mut()
                    .map(|d| d.emit_location(self.current_line, 1, scope_id))
                    .unwrap_or(0);
                self.output.push_str(line);
                self.output.push_str(&format!(", !dbg !{loc_id}"));
                self.output.push('\n');
                return;
            }
        }
        self.output.push_str(line);
        self.output.push('\n');
    }
}

fn type_to_llvm(ty: &Type) -> String {
    match ty {
        Type::Named(name) => match name.as_str() {
            "int" | "uint" => "i64".into(),
            "float" => "double".into(),
            "bool" => "i1".into(),
            "str" => "i8*".into(),
            "void" => "void".into(),
            other => format!("%{other}"),
        },
        Type::List(_) => "{ i64*, i64 }".into(),
        Type::Map(_, _) => "i8*".into(),
        Type::Tuple(ts) => {
            let inner: Vec<String> = ts.iter().map(type_to_llvm).collect();
            format!("{{ {} }}", inner.join(", "))
        }
        Type::Optional(inner) => {
            format!("{{ i1, {} }}", type_to_llvm(inner))
        }
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
        LlvmEmitter::emit(&program).unwrap()
    }

    fn emit_debug(input: &str) -> String {
        let tokens = Lexer::new(input).tokenize().unwrap();
        let program = Parser::new(tokens).parse_program().unwrap();
        LlvmEmitter::emit_with_debug(&program, "test.knl", "/tmp").unwrap()
    }

    #[test]
    fn simple_add() {
        let ir = emit("fn add_one\n  in x: int\n  out result: int\n  do add x 1");
        assert!(ir.contains("define i64 @add_one(i64 %x)"));
        assert!(ir.contains("add i64"));
        assert!(ir.contains("ret i64"));
    }

    #[test]
    fn struct_definition() {
        let ir = emit("struct Point\n  x: int\n  y: int\nend");
        assert!(ir.contains("%Point = type { i64, i64 }"));
    }

    #[test]
    fn comparison_op() {
        let ir = emit("fn is_positive\n  in x: int\n  out result: bool\n  do gt x 0");
        assert!(ir.contains("icmp sgt i64"));
    }

    #[test]
    fn pipe_emits_calls() {
        let ir = emit("fn test\n  in nums: [int]\n  do filter nums gt 0 | reduce add");
        assert!(ir.contains("call"));
        assert!(ir.contains("declare"));
    }

    #[test]
    fn emit_with_debug_contains_dicompileunit() {
        let ir = emit_debug("fn add_one\n  in x: int\n  out result: int\n  do add x 1");
        assert!(ir.contains("!DICompileUnit"));
    }

    #[test]
    fn emit_with_debug_function_has_dbg_attachment() {
        let ir = emit_debug("fn add_one\n  in x: int\n  out result: int\n  do add x 1");
        assert!(ir.contains("!dbg"));
        assert!(ir.contains("define i64 @add_one(i64 %x) !dbg"));
    }

    #[test]
    fn emit_with_debug_has_disubprogram() {
        let ir = emit_debug("fn add_one\n  in x: int\n  out result: int\n  do add x 1");
        assert!(ir.contains("!DISubprogram(name: \"add_one\""));
    }

    #[test]
    fn emit_with_debug_has_dilocation() {
        let ir = emit_debug("fn add_one\n  in x: int\n  out result: int\n  do add x 1");
        assert!(ir.contains("!DILocation"));
    }

    #[test]
    fn basic_compilation_without_debug() {
        let ir = emit("fn add_one\n  in x: int\n  out result: int\n  do add x 1");
        assert!(!ir.contains("!dbg"));
        assert!(!ir.contains("!DICompileUnit"));
        assert!(ir.contains("define i64 @add_one"));
    }

    #[test]
    fn enum_as_tagged_union() {
        let ir = emit("enum Option\n  Some int\n  None\nend");
        assert!(ir.contains("%Option = type { i8, i64 }"));
    }
}
