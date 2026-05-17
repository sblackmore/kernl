pub mod cli;
pub mod lexer;
pub mod parser;
pub mod stdlib;
pub mod semantic;
pub mod typeck;
pub mod verify;
pub mod smt;
pub mod codegen;
pub mod runtime;
pub mod modules;
pub mod driver;
pub mod repl;
pub mod incremental;
pub mod proof;
pub mod profiler;
pub mod debugger;

use lexer::Lexer;
use parser::Parser;
use semantic::SemanticAnalyzer;
use typeck::TypeChecker;
use verify::Verifier;
use codegen::{Codegen, Target};
use codegen::optimize;

pub struct CompileResult {
    pub output: String,
    pub warnings: Vec<String>,
    pub type_errors: Vec<String>,
    pub semantic_errors: Vec<String>,
}

pub fn compile(source: &str, target: Target) -> Result<CompileResult, CompileError> {
    let tokens = Lexer::new(source).tokenize().map_err(|e| CompileError {
        phase: "lex",
        message: e.to_string(),
    })?;

    let mut program = Parser::new(tokens).parse_program().map_err(|e| CompileError {
        phase: "parse",
        message: e.to_string(),
    })?;

    let semantic_errors: Vec<String> = SemanticAnalyzer::check(&program)
        .iter()
        .map(|e| e.to_string())
        .collect();

    let type_errors: Vec<String> = TypeChecker::check(&program)
        .iter()
        .map(|e| e.to_string())
        .collect();

    let verify_errors = Verifier::check(&program);
    let warnings: Vec<String> = verify_errors.iter().map(|e| e.to_string()).collect();

    optimize::fold_constants(&mut program);
    optimize::dead_code_elimination(&mut program);

    let output = Codegen::new(target).emit(&program).map_err(|e| CompileError {
        phase: "codegen",
        message: e.to_string(),
    })?;

    Ok(CompileResult { output, warnings, type_errors, semantic_errors })
}

#[derive(Debug)]
pub struct CompileError {
    pub phase: &'static str,
    pub message: String,
}

impl std::fmt::Display for CompileError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "[{}] {}", self.phase, self.message)
    }
}

impl std::error::Error for CompileError {}
