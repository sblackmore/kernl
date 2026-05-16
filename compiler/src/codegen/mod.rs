pub mod debug_info;
pub mod llvm;
pub mod optimize;
pub mod wasm;
pub mod wasm_binary;
pub mod llvm_opt;

use crate::parser::ast::Program;

#[derive(Debug, Clone)]
pub enum Target {
    LlvmIr,
    Wasm,
    WasmBinary,
    Debug,
}

pub struct Codegen {
    target: Target,
}

impl Codegen {
    pub fn new(target: Target) -> Self {
        Self { target }
    }

    pub fn emit(&self, program: &Program) -> Result<String, CodegenError> {
        match self.target {
            Target::Debug => Ok(format!("{program:#?}")),
            Target::LlvmIr => llvm::LlvmEmitter::emit(program),
            Target::Wasm => wasm::WasmEmitter::emit(program),
            Target::WasmBinary => {
                let bytes = wasm_binary::WasmBinaryEmitter::emit(program)?;
                Ok(hex_dump(&bytes))
            }
        }
    }

    pub fn emit_bytes(&self, program: &Program) -> Result<Vec<u8>, CodegenError> {
        match self.target {
            Target::WasmBinary => wasm_binary::WasmBinaryEmitter::emit(program),
            _ => {
                let text = self.emit(program)?;
                Ok(text.into_bytes())
            }
        }
    }
}

fn hex_dump(bytes: &[u8]) -> String {
    bytes
        .chunks(16)
        .enumerate()
        .map(|(i, chunk)| {
            let hex: Vec<String> = chunk.iter().map(|b| format!("{b:02x}")).collect();
            format!("{:08x}  {}", i * 16, hex.join(" "))
        })
        .collect::<Vec<_>>()
        .join("\n")
}

#[derive(Debug)]
pub struct CodegenError {
    pub message: String,
}

impl std::fmt::Display for CodegenError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "codegen error: {}", self.message)
    }
}

impl std::error::Error for CodegenError {}
