use crate::manifest::Manifest;
use std::fs;
use std::path::Path;
use std::process::Command;

#[derive(Debug, Clone, Copy)]
pub enum Target {
    Llvm,
    Wasm,
    Debug,
}

impl Target {
    pub fn from_str(s: &str) -> Result<Self, String> {
        match s {
            "llvm" => Ok(Self::Llvm),
            "wasm" => Ok(Self::Wasm),
            "debug" => Ok(Self::Debug),
            other => Err(format!("unknown target: {other} (expected llvm, wasm, or debug)")),
        }
    }

    fn flag(&self) -> &str {
        match self {
            Self::Llvm => "llvm",
            Self::Wasm => "wasm",
            Self::Debug => "debug",
        }
    }

    fn extension(&self) -> &str {
        match self {
            Self::Llvm => "ll",
            Self::Wasm => "wat",
            Self::Debug => "txt",
        }
    }
}

pub fn run(target: Target) -> Result<(), String> {
    let manifest_path = Path::new("kernl.toml");
    let manifest = Manifest::load(manifest_path)?;

    let entry = Path::new(&manifest.package.entry);
    if !entry.exists() {
        return Err(format!("entry file not found: {}", entry.display()));
    }

    let compiler = find_compiler()?;

    let build_dir = Path::new("build");
    fs::create_dir_all(build_dir)
        .map_err(|e| format!("failed to create build directory: {e}"))?;

    let stem = entry
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("output");
    let output_file = build_dir.join(format!("{stem}.{}", target.extension()));

    println!(
        "compiling {} -> {} (target: {})",
        entry.display(),
        output_file.display(),
        target.flag()
    );

    let result = Command::new(&compiler)
        .arg(entry.to_str().unwrap())
        .arg("--target")
        .arg(target.flag())
        .output()
        .map_err(|e| format!("failed to run compiler at {}: {e}", compiler))?;

    if !result.status.success() {
        let stderr = String::from_utf8_lossy(&result.stderr);
        return Err(format!("compilation failed:\n{stderr}"));
    }

    let stdout = String::from_utf8_lossy(&result.stdout);
    fs::write(&output_file, stdout.as_bytes())
        .map_err(|e| format!("failed to write {}: {e}", output_file.display()))?;

    println!("wrote {}", output_file.display());
    Ok(())
}

fn find_compiler() -> Result<String, String> {
    if which_exists("kernlc") {
        return Ok("kernlc".into());
    }

    let fallback = "../compiler/target/debug/kernlc";
    if Path::new(fallback).exists() {
        return Ok(fallback.into());
    }

    Err(
        "could not find kernlc compiler. \
         install it or build the compiler with `cargo build` in the compiler/ directory"
            .into(),
    )
}

fn which_exists(name: &str) -> bool {
    Command::new("which")
        .arg(name)
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn target_from_str_valid() {
        assert!(matches!(Target::from_str("llvm"), Ok(Target::Llvm)));
        assert!(matches!(Target::from_str("wasm"), Ok(Target::Wasm)));
        assert!(matches!(Target::from_str("debug"), Ok(Target::Debug)));
    }

    #[test]
    fn target_from_str_invalid() {
        assert!(Target::from_str("javascript").is_err());
    }

    #[test]
    fn target_extension() {
        assert_eq!(Target::Llvm.extension(), "ll");
        assert_eq!(Target::Wasm.extension(), "wat");
        assert_eq!(Target::Debug.extension(), "txt");
    }
}
