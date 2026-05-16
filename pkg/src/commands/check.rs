use crate::manifest::Manifest;
use std::path::Path;
use std::process::Command;

pub fn run() -> Result<(), String> {
    let manifest_path = Path::new("kernl.toml");
    let manifest = Manifest::load(manifest_path)?;

    let entry = Path::new(&manifest.package.entry);
    if !entry.exists() {
        return Err(format!("entry file not found: {}", entry.display()));
    }

    let compiler = find_compiler()?;

    println!("checking {}...", entry.display());

    let result = Command::new(&compiler)
        .arg(entry.to_str().unwrap())
        .arg("--target")
        .arg("debug")
        .output()
        .map_err(|e| format!("failed to run compiler at {}: {e}", compiler))?;

    let stderr = String::from_utf8_lossy(&result.stderr);
    if !stderr.is_empty() {
        eprint!("{stderr}");
    }

    if result.status.success() {
        println!("no errors found");
        Ok(())
    } else {
        Err("check failed with errors".into())
    }
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
