use crate::manifest::Manifest;
use std::path::Path;

pub fn run(file: Option<&str>) -> Result<(), String> {
    let entry = match file {
        Some(f) => {
            let p = Path::new(f);
            if !p.exists() {
                return Err(format!("file not found: {f}"));
            }
            f.to_string()
        }
        None => {
            let manifest_path = Path::new("kernl.toml");
            let manifest = Manifest::load(manifest_path)?;
            manifest.package.entry
        }
    };

    println!("compiling {entry}...");

    super::build::run(super::build::Target::Llvm)?;

    println!(
        "note: direct execution not yet supported. \
         compiled output is in the build/ directory."
    );

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn run_missing_file_returns_error() {
        let result = run(Some("/nonexistent/path/to/file.knl"));
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("not found"));
    }
}
