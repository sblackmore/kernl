use std::env;
use std::fs;
use std::io::{IsTerminal, Read};
use std::path::{Path, PathBuf};
use std::process;

use crate::codegen::{Codegen, Target};
use crate::codegen::llvm::LlvmEmitter;
use crate::codegen::llvm_opt::{self, Pass};
use crate::debugger::Debugger;
use crate::driver::{Driver, DriverConfig, OptLevel};
use crate::driver::targets::CompileTarget;
use crate::lexer::Lexer;
use crate::parser::Parser;
use crate::parser::ast::{Item, Param, Program, Type};
use crate::profiler::{instrument::instrument_llvm_ir, Profiler};
use crate::runtime::executor::{Executor, Value};
use crate::runtime::ResolverConfig;
use crate::semantic::SemanticAnalyzer;
use crate::smt::{SmtSolver, SmtEncoder, VerifyResult};
use crate::typeck::TypeChecker;
use crate::verify::Verifier;
use crate::codegen::optimize;

fn exe_stem(args: &[String]) -> &str {
    args.first()
        .map(|p| {
            Path::new(p)
                .file_stem()
                .and_then(|s| s.to_str())
                .unwrap_or("kernl")
        })
        .unwrap_or("kernl")
}

fn print_full_usage() {
    println!("usage: kernlc <file.knl> [--target debug|llvm|wasm|wasm-bin|native] [--verify] [--run]");
    println!("       kernl  <file.knl>   # same as kernlc --run; pipes stdin into main when non-tty");
    println!("       kernlc --repl [--target debug|llvm|wasm]");
    println!();
    println!("general:");
    println!("  --help / -h               show this message");
    println!("  --version / -V            print version");
    println!();
    println!("targets:");
    println!("  debug     dump parsed AST (default)");
    println!("  llvm      emit LLVM IR (.ll)");
    println!("  wasm      emit WebAssembly Text (.wat)");
    println!("  wasm-bin  emit WebAssembly Binary (.wasm)");
    println!("  native    compile to native binary via LLVM");
    println!();
    println!("modes:");
    println!("  --repl                      launch interactive REPL");
    println!("  --run                       interpret the program via executor");
    println!("  --compile                   kernl only: emit AST/LLVM/etc. (skip implicit --run)");
    println!("  --invoke-stdin              with --run: read stdin into main's single `str` parameter");
    println!("  --no-stdin                  kernl: do not auto-bind piped stdin");
    println!("  --profile                   print profiling report after execution (with --run)");
    println!("  --debug                     enable interactive debugger (with --run)");
    println!();
    println!("run options:");
    println!("  --resolver-endpoint <url>   LLM API endpoint for fluid mode");
    println!("  --resolver-model <model>    LLM model name for fluid mode");
    println!();
    println!("verification:");
    println!("  --verify  formally verify invariants via SMT solver (Z3)");
    println!();
    println!("proof export:");
    println!("  --export-lean  emit Lean 4 proof skeletons (stdout)");
    println!("  --export-coq   emit Coq proof skeletons (stdout)");
    println!();
    println!("instrumentation:");
    println!("  --instrument-llvm  insert __kernl_profile_enter/exit calls into LLVM IR");
    println!("                     (link runtime/kernl_profile.o via libkernl_rt.a)");
    println!();
    println!("native options:");
    println!("  --runtime-path <dir>  path to dir containing libkernl_rt.a");
    println!("  -O <0|1|2|3>          optimization level (default: 2)");
    println!("  -o <file>             output file path");
    println!("  --keep-intermediates  keep .ll and .o files");
    println!("  --debug-info          emit DWARF debug metadata in LLVM IR");
    println!("  --opt-passes <list>   comma-separated LLVM opt passes (e.g. mem2reg,instcombine)");
    println!();
    println!("cross-compilation:");
    println!("  --cross <triple>      cross-compile to target triple");
    println!("  --list-targets        list all supported compilation targets");
}

pub fn run(script_default_run: bool) {
    let args: Vec<String> = env::args().collect();

    if args.iter().any(|a| a == "--help" || a == "-h") {
        print_full_usage();
        return;
    }
    if args.iter().any(|a| a == "--version" || a == "-V") {
        println!("{} {}", exe_stem(&args), env!("CARGO_PKG_VERSION"));
        return;
    }

    let is_repl = args.iter().any(|a| a == "--repl");

    if is_repl {
        let target_str = args.iter()
            .position(|a| a == "--target")
            .and_then(|i| args.get(i + 1))
            .map(|s| s.as_str())
            .unwrap_or("debug");

        let target = match target_str {
            "llvm" => Target::LlvmIr,
            "wasm" => Target::Wasm,
            _ => Target::Debug,
        };

        let mut repl = crate::repl::Repl::new(target);
        repl.run();
        return;
    }

    if args.iter().any(|a| a == "--list-targets") {
        eprintln!("supported cross-compilation targets:");
        for t in CompileTarget::all_targets() {
            eprintln!("  {:<40} ({}, {})", t.triple, t.arch, t.os);
        }
        return;
    }

    if args.len() < 2 {
        print_full_usage();
        eprintln!();
        eprintln!("error: missing <file.knl>");
        process::exit(1);
    }

    let path = &args[1];
    let do_verify = args.iter().any(|a| a == "--verify");
    let compile_only = args.iter().any(|a| a == "--compile");
    let mut do_run = args.iter().any(|a| a == "--run");
    if script_default_run && !compile_only {
        do_run = true;
    }
    let do_export_lean = args.iter().any(|a| a == "--export-lean");
    let do_export_coq = args.iter().any(|a| a == "--export-coq");
    let instrument_llvm = args.iter().any(|a| a == "--instrument-llvm");

    let target_str = args.iter()
        .position(|a| a == "--target")
        .and_then(|i| args.get(i + 1))
        .map(|s| s.as_str())
        .unwrap_or("debug");

    let is_native = target_str == "native";

    let target = match target_str {
        "llvm" | "native" => Target::LlvmIr,
        "wasm" => Target::Wasm,
        "wasm-bin" => Target::WasmBinary,
        _ => Target::Debug,
    };

    let source = match fs::read_to_string(path) {
        Ok(s) => s,
        Err(e) => {
            eprintln!("error reading {path}: {e}");
            process::exit(1);
        }
    };

    if do_verify {
        run_verify(&source, path);
        return;
    }

    if do_export_lean || do_export_coq {
        run_proof_export(&source, do_export_lean);
        return;
    }

    if do_run {
        run_program(&source, &args, script_default_run);
        return;
    }

    let debug_info = args.iter().any(|a| a == "--debug-info");

    if is_native {
        emit_native(&source, path, &args, debug_info, instrument_llvm);
    } else if matches!(target, Target::WasmBinary) {
        emit_wasm_binary(&source, path);
    } else if matches!(target, Target::LlvmIr) && debug_info {
        emit_llvm_with_debug(&source, path, instrument_llvm);
    } else {
        match crate::compile(&source, target) {
            Ok(mut result) => {
                for e in &result.semantic_errors {
                    eprintln!("semantic: {e}");
                }
                for e in &result.type_errors {
                    eprintln!("type error: {e}");
                }
                for w in &result.warnings {
                    eprintln!("warning: {w}");
                }
                if instrument_llvm && target_str == "llvm" {
                    result.output = instrument_llvm_ir(&result.output);
                }
                println!("{}", result.output);

                if !result.type_errors.is_empty() || !result.semantic_errors.is_empty() {
                    process::exit(2);
                }
            }
            Err(e) => {
                eprintln!("error: {e}");
                process::exit(1);
            }
        }
    }
}

fn get_flag_value(args: &[String], flag: &str) -> Option<String> {
    args.iter()
        .position(|a| a == flag)
        .and_then(|i| args.get(i + 1))
        .cloned()
}

fn emit_llvm_with_debug(source: &str, input_path: &str, instrument_llvm: bool) {
    let tokens = match Lexer::new(source).tokenize() {
        Ok(t) => t,
        Err(e) => { eprintln!("error: [lex] {e}"); process::exit(1); }
    };

    let mut program = match Parser::new(tokens).parse_program() {
        Ok(p) => p,
        Err(e) => { eprintln!("error: [parse] {e}"); process::exit(1); }
    };

    let semantic_errors: Vec<String> = SemanticAnalyzer::check(&program).iter().map(|e| e.to_string()).collect();
    let type_errors: Vec<String> = TypeChecker::check(&program).iter().map(|e| e.to_string()).collect();

    for e in &semantic_errors { eprintln!("semantic: {e}"); }
    for e in &type_errors { eprintln!("type error: {e}"); }
    if !type_errors.is_empty() || !semantic_errors.is_empty() { process::exit(2); }

    optimize::fold_constants(&mut program);
    optimize::dead_code_elimination(&mut program);

    let file_name = Path::new(input_path).file_name().unwrap_or_default().to_string_lossy().to_string();
    let directory = Path::new(input_path).parent().unwrap_or(Path::new(".")).to_string_lossy().to_string();

    match LlvmEmitter::emit_with_debug(&program, &file_name, &directory) {
        Ok(mut ir) => {
            if instrument_llvm {
                ir = instrument_llvm_ir(&ir);
            }
            println!("{ir}");
        }
        Err(e) => { eprintln!("error: [codegen] {e}"); process::exit(1); }
    }
}

fn emit_native(source: &str, input_path: &str, args: &[String], debug_info: bool, instrument_llvm: bool) {
    let opt_level = match get_flag_value(args, "-O").as_deref() {
        Some("0") => OptLevel::O0,
        Some("1") => OptLevel::O1,
        Some("3") => OptLevel::O3,
        _ => OptLevel::O2,
    };

    let runtime_path = get_flag_value(args, "--runtime-path").map(PathBuf::from);
    let keep_intermediates = args.iter().any(|a| a == "--keep-intermediates");
    let custom_passes = get_flag_value(args, "--opt-passes");

    let cross_target = get_flag_value(args, "--cross").and_then(|triple| {
        match CompileTarget::from_triple(&triple) {
            Some(t) => Some(t),
            None => {
                eprintln!("error: unknown cross-compilation target '{triple}'");
                eprintln!("run 'kernlc --list-targets' to see supported targets");
                process::exit(1);
            }
        }
    });

    let output_path = match get_flag_value(args, "-o") {
        Some(p) => PathBuf::from(p),
        None => {
            let p = Path::new(input_path);
            p.with_extension("")
        }
    };

    let tokens = match Lexer::new(source).tokenize() {
        Ok(t) => t,
        Err(e) => { eprintln!("error: [lex] {e}"); process::exit(1); }
    };

    let mut program = match Parser::new(tokens).parse_program() {
        Ok(p) => p,
        Err(e) => { eprintln!("error: [parse] {e}"); process::exit(1); }
    };

    let semantic_errors: Vec<String> = SemanticAnalyzer::check(&program).iter().map(|e| e.to_string()).collect();
    let type_errors: Vec<String> = TypeChecker::check(&program).iter().map(|e| e.to_string()).collect();
    let verify_errors = Verifier::check(&program);

    for e in &semantic_errors { eprintln!("semantic: {e}"); }
    for e in &type_errors { eprintln!("type error: {e}"); }
    for w in &verify_errors { eprintln!("warning: {w}"); }

    if !type_errors.is_empty() || !semantic_errors.is_empty() { process::exit(2); }

    optimize::fold_constants(&mut program);
    optimize::dead_code_elimination(&mut program);

    let ir = if let Some(ref ct) = cross_target {
        LlvmEmitter::emit_for_target(&program, ct)
    } else if debug_info {
        let file_name = Path::new(input_path).file_name().unwrap_or_default().to_string_lossy().to_string();
        let directory = Path::new(input_path).parent().unwrap_or(Path::new(".")).to_string_lossy().to_string();
        LlvmEmitter::emit_with_debug(&program, &file_name, &directory)
    } else {
        LlvmEmitter::emit(&program)
    };

    let ir = match ir {
        Ok(ir) => ir,
        Err(e) => { eprintln!("error: [codegen] {e}"); process::exit(1); }
    };

    let passes = if let Some(ref pass_str) = custom_passes {
        pass_str.split(',').map(|s| Pass::from_name(s.trim())).collect()
    } else {
        match opt_level {
            OptLevel::O0 => llvm_opt::pipeline_o0(),
            OptLevel::O1 => llvm_opt::pipeline_o1(),
            OptLevel::O2 => llvm_opt::pipeline_o2(),
            OptLevel::O3 => llvm_opt::pipeline_o3(),
        }
    };

    let mut ir = if !passes.is_empty() && llvm_opt::has_opt() {
        match llvm_opt::optimize_ir(&ir, &passes) {
            Ok(optimized) => optimized,
            Err(e) => {
                eprintln!("warning: opt failed ({e}), using unoptimized IR");
                ir
            }
        }
    } else {
        ir
    };

    if instrument_llvm {
        ir = instrument_llvm_ir(&ir);
    }

    let config = DriverConfig {
        opt_level,
        output: Some(output_path.clone()),
        runtime_path,
        keep_intermediates,
        target: cross_target,
    };

    let driver = Driver::new(config);
    if let Err(e) = driver.compile_to_native(&ir, &output_path) {
        eprintln!("error: {e}");
        process::exit(1);
    }

    eprintln!("wrote native binary to {}", output_path.display());
}

fn emit_wasm_binary(source: &str, input_path: &str) {
    let tokens = match Lexer::new(source).tokenize() {
        Ok(t) => t,
        Err(e) => {
            eprintln!("error: [lex] {e}");
            process::exit(1);
        }
    };

    let mut program = match Parser::new(tokens).parse_program() {
        Ok(p) => p,
        Err(e) => {
            eprintln!("error: [parse] {e}");
            process::exit(1);
        }
    };

    let semantic_errors: Vec<String> = SemanticAnalyzer::check(&program)
        .iter()
        .map(|e| e.to_string())
        .collect();
    let type_errors: Vec<String> = TypeChecker::check(&program)
        .iter()
        .map(|e| e.to_string())
        .collect();
    let verify_errors = Verifier::check(&program);

    for e in &semantic_errors {
        eprintln!("semantic: {e}");
    }
    for e in &type_errors {
        eprintln!("type error: {e}");
    }
    for w in &verify_errors {
        eprintln!("warning: {w}");
    }

    if !type_errors.is_empty() || !semantic_errors.is_empty() {
        process::exit(2);
    }

    optimize::fold_constants(&mut program);
    optimize::dead_code_elimination(&mut program);

    let codegen = Codegen::new(Target::WasmBinary);
    let bytes = match codegen.emit_bytes(&program) {
        Ok(b) => b,
        Err(e) => {
            eprintln!("error: [codegen] {e}");
            process::exit(1);
        }
    };

    let output_path = Path::new(input_path).with_extension("wasm");

    match fs::write(&output_path, &bytes) {
        Ok(_) => {
            eprintln!("wrote {} bytes to {}", bytes.len(), output_path.display());
        }
        Err(e) => {
            eprintln!("error writing {}: {e}", output_path.display());
            process::exit(1);
        }
    }
}

fn run_verify(source: &str, path: &str) {
    let tokens = match Lexer::new(source).tokenize() {
        Ok(t) => t,
        Err(e) => {
            eprintln!("error: [lex] {e}");
            process::exit(1);
        }
    };

    let program = match Parser::new(tokens).parse_program() {
        Ok(p) => p,
        Err(e) => {
            eprintln!("error: [parse] {e}");
            process::exit(1);
        }
    };

    let type_errors: Vec<String> = TypeChecker::check(&program)
        .iter()
        .map(|e| e.to_string())
        .collect();

    if !type_errors.is_empty() {
        for e in &type_errors {
            eprintln!("type error: {e}");
        }
        process::exit(2);
    }

    let results = SmtSolver::verify_program(&program);

    if results.is_empty() {
        eprintln!("no invariants to verify in {path}");
        return;
    }

    let mut any_violated = false;
    let mut any_solver_missing = false;

    for (func_name, checks) in &results {
        eprintln!("verifying fn {func_name}...");
        for (idx, result) in checks {
            let func = program.items.iter().find_map(|item| {
                if let crate::parser::ast::Item::Function(f) = item {
                    if f.name == *func_name { Some(f) } else { None }
                } else {
                    None
                }
            });
            let desc = func
                .and_then(|f| f.invariants.get(*idx))
                .map(|inv| SmtEncoder::describe_invariant(inv))
                .unwrap_or_else(|| format!("invariant {idx}"));

            match result {
                VerifyResult::Verified => {
                    eprintln!("  inv {idx} ({desc}): \u{2713} verified (UNSAT)");
                }
                VerifyResult::Violated(ce) => {
                    eprintln!("  inv {idx} ({desc}): \u{2717} VIOLATED (SAT) — {ce}");
                    any_violated = true;
                }
                VerifyResult::Unknown(msg) => {
                    eprintln!("  inv {idx} ({desc}): ? unknown — {msg}");
                }
                VerifyResult::SolverNotFound => {
                    eprintln!("  inv {idx} ({desc}): ? z3 not found (install z3 for formal verification)");
                    any_solver_missing = true;
                }
            }
        }
    }

    if any_solver_missing {
        let smt_dir = Path::new(path).parent().unwrap_or(Path::new(".")).join("build/smt");
        let _ = fs::create_dir_all(&smt_dir);
        for (func_name, _) in &results {
            let func = program.items.iter().find_map(|item| {
                if let crate::parser::ast::Item::Function(f) = item {
                    if f.name == *func_name { Some(f) } else { None }
                } else {
                    None
                }
            });
            if let Some(f) = func {
                let smt_result = SmtEncoder::encode_function(f);
                let out_path = smt_dir.join(format!("{func_name}.smt2"));
                let _ = fs::write(&out_path, &smt_result.script);
            }
        }
        eprintln!("SMT-LIB2 scripts written to {} for manual verification", smt_dir.display());
    } else if !any_violated {
        eprintln!("all invariants verified");
    }

    if any_violated {
        process::exit(3);
    }
}

fn run_program(source: &str, args: &[String], script_default_run: bool) {
    let do_profile = args.iter().any(|a| a == "--profile");
    let do_debug = args.iter().any(|a| a == "--debug");

    let tokens = match Lexer::new(source).tokenize() {
        Ok(t) => t,
        Err(e) => {
            eprintln!("error: [lex] {e}");
            process::exit(1);
        }
    };

    let program = match Parser::new(tokens).parse_program() {
        Ok(p) => p,
        Err(e) => {
            eprintln!("error: [parse] {e}");
            process::exit(1);
        }
    };

    let endpoint = get_flag_value(args, "--resolver-endpoint")
        .unwrap_or_else(|| "http://localhost:8080/v1/chat/completions".into());
    let model = get_flag_value(args, "--resolver-model")
        .unwrap_or_else(|| "gpt-4".into());

    let config = ResolverConfig {
        endpoint,
        model,
        timeout_ms: 30000,
        max_retries: 3,
    };

    let mut executor = Executor::new(config);
    executor.load(&program);

    let mut profiler = Profiler::new();
    if do_profile {
        profiler.enable();
    }

    let mut debugger = if do_debug {
        let mut dbg = Debugger::new();
        eprintln!("kernl debugger active. Type 'h' for help at the prompt.");
        eprintln!("Enter breakpoint function names (one per line, empty line to start):");
        let stdin = std::io::stdin();
        loop {
            let mut line = String::new();
            if stdin.read_line(&mut line).unwrap_or(0) == 0 || line.trim().is_empty() {
                break;
            }
            let bp_name = line.trim().to_string();
            let id = dbg.add_breakpoint(&bp_name);
            eprintln!("breakpoint #{id} set on '{bp_name}'");
        }
        Some(dbg)
    } else {
        None
    };

    let entry = if executor_has_function(&program, "main") {
        "main"
    } else {
        match program.items.first() {
            Some(crate::parser::ast::Item::Function(f)) => f.name.as_str(),
            _ => {
                eprintln!("error: no function to run");
                process::exit(1);
            }
        }
    };

    profiler.enter(entry);
    if let Some(ref mut dbg) = debugger {
        let locals = std::collections::HashMap::new();
        dbg.enter_function(entry, &locals);
        if dbg.should_break(entry) {
            eprintln!("break at function '{entry}'");
            use crate::debugger::DebugAction;
            loop {
                match dbg.prompt() {
                    DebugAction::Continue => break,
                    DebugAction::Backtrace => dbg.print_backtrace(),
                    DebugAction::Locals => dbg.print_locals(),
                    DebugAction::Print(var) => dbg.print_variable(&var),
                    DebugAction::ListBreakpoints => dbg.list_breakpoints(),
                    DebugAction::Quit => {
                        eprintln!("debugger quit");
                        process::exit(0);
                    }
                    _ => break,
                }
            }
        }
    }

    let mut do_invoke_stdin = args.iter().any(|a| a == "--invoke-stdin");
    if script_default_run && !args.iter().any(|a| a == "--no-stdin") {
        if main_accepts_single_str(&program) && !std::io::stdin().is_terminal() {
            do_invoke_stdin = true;
        }
    }
    let main_args = main_call_args(&program, do_invoke_stdin);

    match executor.call(entry, main_args) {
        Ok(result) => {
            if !matches!(result, Value::Void) {
                println!("{result}");
            }
        }
        Err(e) => {
            eprintln!("runtime error: {}", e.message);
            process::exit(1);
        }
    }

    profiler.exit(entry);
    if let Some(ref mut dbg) = debugger {
        dbg.exit_function();
    }

    if do_profile {
        eprintln!("\n--- kernl profile report ---");
        eprint!("{}", profiler.report());
    }
}

fn run_proof_export(source: &str, lean: bool) {
    let tokens = match Lexer::new(source).tokenize() {
        Ok(t) => t,
        Err(e) => {
            eprintln!("error: [lex] {e}");
            process::exit(1);
        }
    };

    let program = match Parser::new(tokens).parse_program() {
        Ok(p) => p,
        Err(e) => {
            eprintln!("error: [parse] {e}");
            process::exit(1);
        }
    };

    if lean {
        println!("{}", crate::proof::LeanExporter::export_program(&program));
    } else {
        println!("{}", crate::proof::CoqExporter::export_program(&program));
    }
}

fn main_accepts_single_str(program: &Program) -> bool {
    let Some(main_fn) = program.items.iter().find_map(|item| {
        if let Item::Function(f) = item {
            (f.name == "main").then_some(f)
        } else {
            None
        }
    }) else {
        return false;
    };
    main_fn.params.len() == 1 && param_is_str(&main_fn.params[0])
}

fn executor_has_function(program: &crate::parser::ast::Program, name: &str) -> bool {
    program.items.iter().any(|item| {
        matches!(item, crate::parser::ast::Item::Function(f) if f.name == name)
    })
}

fn param_is_str(p: &Param) -> bool {
    matches!(&p.ty, Type::Named(n) if n == "str")
}

/// When `main` declares exactly one `str` parameter, bind it from stdin if `--invoke-stdin`,
/// otherwise use an empty string (local `kernlc … --run` smoke tests).
fn main_call_args(program: &Program, do_invoke_stdin: bool) -> Vec<Value> {
    let Some(main_fn) = program.items.iter().find_map(|item| {
        if let Item::Function(f) = item {
            (f.name == "main").then_some(f)
        } else {
            None
        }
    }) else {
        return vec![];
    };
    if main_fn.params.len() != 1 || !param_is_str(&main_fn.params[0]) {
        return vec![];
    }
    let payload = if do_invoke_stdin {
        let mut buf = String::new();
        let _ = std::io::stdin().read_to_string(&mut buf);
        buf.trim().to_string()
    } else {
        String::new()
    };
    vec![Value::Str(payload)]
}
