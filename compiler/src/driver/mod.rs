pub mod targets;

use std::fmt;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use targets::CompileTarget;

#[derive(Debug, Clone, Copy)]
pub enum OptLevel {
    O0,
    O1,
    O2,
    O3,
}

impl OptLevel {
    fn as_llc_flag(self) -> &'static str {
        match self {
            OptLevel::O0 => "-O0",
            OptLevel::O1 => "-O1",
            OptLevel::O2 => "-O2",
            OptLevel::O3 => "-O3",
        }
    }
}

impl fmt::Display for OptLevel {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            OptLevel::O0 => write!(f, "O0"),
            OptLevel::O1 => write!(f, "O1"),
            OptLevel::O2 => write!(f, "O2"),
            OptLevel::O3 => write!(f, "O3"),
        }
    }
}

pub struct DriverConfig {
    pub opt_level: OptLevel,
    pub output: Option<PathBuf>,
    pub runtime_path: Option<PathBuf>,
    pub keep_intermediates: bool,
    pub target: Option<CompileTarget>,
}

impl Default for DriverConfig {
    fn default() -> Self {
        Self {
            opt_level: OptLevel::O2,
            output: None,
            runtime_path: None,
            keep_intermediates: false,
            target: None,
        }
    }
}

#[derive(Debug)]
pub struct DriverError {
    pub message: String,
}

impl fmt::Display for DriverError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "driver error: {}", self.message)
    }
}

impl std::error::Error for DriverError {}

impl From<std::io::Error> for DriverError {
    fn from(e: std::io::Error) -> Self {
        DriverError {
            message: format!("IO error: {e}"),
        }
    }
}

pub struct Driver {
    runtime_path: Option<PathBuf>,
    opt_level: OptLevel,
    keep_intermediates: bool,
    target: Option<CompileTarget>,
}

impl Driver {
    pub fn new(config: DriverConfig) -> Self {
        Self {
            runtime_path: config.runtime_path,
            opt_level: config.opt_level,
            keep_intermediates: config.keep_intermediates,
            target: config.target,
        }
    }

    /// Full pipeline: LLVM IR text -> .ll file -> .o file -> native binary
    pub fn compile_to_native(&self, ir: &str, output: &Path) -> Result<(), DriverError> {
        let ll_path = output.with_extension("ll");
        fs::write(&ll_path, ir)?;

        let obj_path = output.with_extension("o");
        self.ll_to_obj(&ll_path, &obj_path)?;

        self.link(&obj_path, output)?;

        if !self.keep_intermediates {
            let _ = fs::remove_file(&ll_path);
            let _ = fs::remove_file(&obj_path);
        }

        Ok(())
    }

    fn ll_to_obj(&self, ll: &Path, obj: &Path) -> Result<(), DriverError> {
        let opt_flag = self.opt_level.as_llc_flag();

        let mut cmd = Command::new("llc");
        cmd.args(["-filetype=obj", "-relocation-model=pic"])
            .arg(opt_flag);

        if let Some(ref target) = self.target {
            cmd.arg(format!("--mtriple={}", target.llvm_triple()));
        }

        let status = cmd.arg(ll)
            .arg("-o")
            .arg(obj)
            .status();

        match status {
            Ok(s) if s.success() => return Ok(()),
            _ => {}
        }

        let status = Command::new("clang")
            .args(["-c", "-x", "ir"])
            .arg(opt_flag)
            .arg(ll)
            .arg("-o")
            .arg(obj)
            .status()
            .map_err(|e| DriverError {
                message: format!("neither llc nor clang found: {e}"),
            })?;

        if !status.success() {
            return Err(DriverError {
                message: "compilation to object file failed".into(),
            });
        }
        Ok(())
    }

    fn link(&self, obj: &Path, output: &Path) -> Result<(), DriverError> {
        let mut cmd = Command::new("cc");
        cmd.arg(obj);
        cmd.arg("-o").arg(output);
        cmd.arg("-lm");

        if let Some(ref rt) = self.runtime_path {
            cmd.arg(format!("-L{}", rt.display()));
            cmd.arg("-lkernl_rt");
        }

        let status = cmd.status().map_err(|e| DriverError {
            message: format!("linker not found: {e}"),
        })?;

        if !status.success() {
            return Err(DriverError {
                message: "linking failed".into(),
            });
        }
        Ok(())
    }
}

/// Check if `llc` is available on PATH.
pub fn has_llc() -> bool {
    Command::new("llc")
        .arg("--version")
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false)
}

/// Check if `clang` is available on PATH.
pub fn has_clang() -> bool {
    Command::new("clang")
        .arg("--version")
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_has_llc_returns_bool() {
        let _result: bool = has_llc();
    }

    #[test]
    fn test_has_clang_returns_bool() {
        let _result: bool = has_clang();
    }

    #[test]
    fn test_driver_config_defaults() {
        let config = DriverConfig::default();
        assert!(config.output.is_none());
        assert!(config.runtime_path.is_none());
        assert!(!config.keep_intermediates);
        assert!(matches!(config.opt_level, OptLevel::O2));
    }

    #[test]
    fn test_write_ir_to_file() {
        let dir = std::env::temp_dir().join("kernl_driver_test");
        let _ = fs::create_dir_all(&dir);
        let ll_path = dir.join("test.ll");

        let ir = "; ModuleID = 'test'\ndeclare void @main()\n";
        fs::write(&ll_path, ir).unwrap();

        let contents = fs::read_to_string(&ll_path).unwrap();
        assert_eq!(contents, ir);

        let _ = fs::remove_file(&ll_path);
        let _ = fs::remove_dir(&dir);
    }

    #[test]
    fn test_opt_level_display() {
        assert_eq!(OptLevel::O0.to_string(), "O0");
        assert_eq!(OptLevel::O1.to_string(), "O1");
        assert_eq!(OptLevel::O2.to_string(), "O2");
        assert_eq!(OptLevel::O3.to_string(), "O3");
    }

    #[test]
    fn test_driver_error_display() {
        let err = DriverError {
            message: "test error".into(),
        };
        assert_eq!(err.to_string(), "driver error: test error");
    }
}
