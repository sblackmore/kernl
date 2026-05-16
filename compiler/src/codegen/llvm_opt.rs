use std::process::Command;
use std::fs;

/// Optimization passes that can be applied to LLVM IR.
#[derive(Debug, Clone)]
pub enum Pass {
    InstCombine,
    SCCP,
    DeadCodeElim,
    SimplifyCFG,
    Reassociate,
    GVN,
    LICM,
    LoopUnroll,

    InlineSmallFns,
    TailCallElim,
    MemToReg,

    GlobalDCE,
    StripDeadPrototypes,

    Custom(String),
}

impl Pass {
    pub fn to_opt_flag(&self) -> String {
        match self {
            Pass::InstCombine => "-passes=instcombine".into(),
            Pass::SCCP => "-passes=sccp".into(),
            Pass::DeadCodeElim => "-passes=adce".into(),
            Pass::SimplifyCFG => "-passes=simplifycfg".into(),
            Pass::Reassociate => "-passes=reassociate".into(),
            Pass::GVN => "-passes=gvn".into(),
            Pass::LICM => "-passes=licm".into(),
            Pass::LoopUnroll => "-passes=loop-unroll".into(),
            Pass::InlineSmallFns => "-passes=inline".into(),
            Pass::TailCallElim => "-passes=tailcallelim".into(),
            Pass::MemToReg => "-passes=mem2reg".into(),
            Pass::GlobalDCE => "-passes=globaldce".into(),
            Pass::StripDeadPrototypes => "-passes=strip-dead-prototypes".into(),
            Pass::Custom(s) => format!("-passes={s}"),
        }
    }

    /// Parse a pass name from a CLI string.
    pub fn from_name(name: &str) -> Self {
        match name {
            "instcombine" => Pass::InstCombine,
            "sccp" => Pass::SCCP,
            "adce" => Pass::DeadCodeElim,
            "simplifycfg" => Pass::SimplifyCFG,
            "reassociate" => Pass::Reassociate,
            "gvn" => Pass::GVN,
            "licm" => Pass::LICM,
            "loop-unroll" => Pass::LoopUnroll,
            "inline" => Pass::InlineSmallFns,
            "tailcallelim" => Pass::TailCallElim,
            "mem2reg" => Pass::MemToReg,
            "globaldce" => Pass::GlobalDCE,
            "strip-dead-prototypes" => Pass::StripDeadPrototypes,
            other => Pass::Custom(other.to_string()),
        }
    }
}

pub fn pipeline_o0() -> Vec<Pass> {
    vec![]
}

pub fn pipeline_o1() -> Vec<Pass> {
    vec![Pass::MemToReg, Pass::InstCombine, Pass::SimplifyCFG, Pass::DeadCodeElim]
}

pub fn pipeline_o2() -> Vec<Pass> {
    vec![
        Pass::MemToReg, Pass::InstCombine, Pass::SCCP, Pass::Reassociate,
        Pass::GVN, Pass::SimplifyCFG, Pass::DeadCodeElim,
        Pass::InlineSmallFns, Pass::TailCallElim, Pass::GlobalDCE,
    ]
}

pub fn pipeline_o3() -> Vec<Pass> {
    let mut passes = pipeline_o2();
    passes.extend([Pass::LICM, Pass::LoopUnroll, Pass::StripDeadPrototypes]);
    passes
}

/// Check if the LLVM `opt` tool is available on PATH.
pub fn has_opt() -> bool {
    Command::new("opt")
        .arg("--version")
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false)
}

/// Run LLVM `opt` on IR text with specified passes.
pub fn optimize_ir(ir: &str, passes: &[Pass]) -> Result<String, OptError> {
    if passes.is_empty() {
        return Ok(ir.to_string());
    }

    let tmp_in = std::env::temp_dir().join("kernl_opt_in.ll");
    let tmp_out = std::env::temp_dir().join("kernl_opt_out.ll");
    fs::write(&tmp_in, ir).map_err(|e| OptError { message: format!("write failed: {e}") })?;

    let combined = passes.iter()
        .map(|p| {
            p.to_opt_flag()
                .strip_prefix("-passes=")
                .unwrap_or("")
                .to_string()
        })
        .collect::<Vec<_>>()
        .join(",");

    let status = Command::new("opt")
        .arg(&tmp_in)
        .arg("-S")
        .arg(format!("-passes={combined}"))
        .arg("-o").arg(&tmp_out)
        .status()
        .map_err(|e| OptError { message: format!("opt not found: {e}") })?;

    if !status.success() {
        let _ = fs::remove_file(&tmp_in);
        return Err(OptError { message: "opt returned non-zero".into() });
    }

    let result = fs::read_to_string(&tmp_out)
        .map_err(|e| OptError { message: format!("read failed: {e}") })?;

    let _ = fs::remove_file(&tmp_in);
    let _ = fs::remove_file(&tmp_out);

    Ok(result)
}

#[derive(Debug)]
pub struct OptError {
    pub message: String,
}

impl std::fmt::Display for OptError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "opt error: {}", self.message)
    }
}

impl std::error::Error for OptError {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pipeline_o0_is_empty() {
        assert!(pipeline_o0().is_empty());
    }

    #[test]
    fn pipeline_o1_returns_expected_passes() {
        let passes = pipeline_o1();
        assert_eq!(passes.len(), 4);
        assert!(matches!(passes[0], Pass::MemToReg));
        assert!(matches!(passes[1], Pass::InstCombine));
        assert!(matches!(passes[2], Pass::SimplifyCFG));
        assert!(matches!(passes[3], Pass::DeadCodeElim));
    }

    #[test]
    fn pipeline_o2_includes_inline() {
        let passes = pipeline_o2();
        assert!(passes.iter().any(|p| matches!(p, Pass::InlineSmallFns)));
    }

    #[test]
    fn pipeline_o3_includes_loop_passes() {
        let passes = pipeline_o3();
        assert!(passes.iter().any(|p| matches!(p, Pass::LICM)));
        assert!(passes.iter().any(|p| matches!(p, Pass::LoopUnroll)));
        assert!(passes.iter().any(|p| matches!(p, Pass::StripDeadPrototypes)));
    }

    #[test]
    fn has_opt_returns_bool() {
        let _result: bool = has_opt();
    }

    #[test]
    fn to_opt_flag_instcombine() {
        assert_eq!(Pass::InstCombine.to_opt_flag(), "-passes=instcombine");
    }

    #[test]
    fn to_opt_flag_sccp() {
        assert_eq!(Pass::SCCP.to_opt_flag(), "-passes=sccp");
    }

    #[test]
    fn to_opt_flag_mem2reg() {
        assert_eq!(Pass::MemToReg.to_opt_flag(), "-passes=mem2reg");
    }

    #[test]
    fn to_opt_flag_custom() {
        let pass = Pass::Custom("my-pass".into());
        assert_eq!(pass.to_opt_flag(), "-passes=my-pass");
    }

    #[test]
    fn to_opt_flag_all_passes_correct() {
        assert_eq!(Pass::DeadCodeElim.to_opt_flag(), "-passes=adce");
        assert_eq!(Pass::SimplifyCFG.to_opt_flag(), "-passes=simplifycfg");
        assert_eq!(Pass::Reassociate.to_opt_flag(), "-passes=reassociate");
        assert_eq!(Pass::GVN.to_opt_flag(), "-passes=gvn");
        assert_eq!(Pass::LICM.to_opt_flag(), "-passes=licm");
        assert_eq!(Pass::LoopUnroll.to_opt_flag(), "-passes=loop-unroll");
        assert_eq!(Pass::InlineSmallFns.to_opt_flag(), "-passes=inline");
        assert_eq!(Pass::TailCallElim.to_opt_flag(), "-passes=tailcallelim");
        assert_eq!(Pass::GlobalDCE.to_opt_flag(), "-passes=globaldce");
        assert_eq!(Pass::StripDeadPrototypes.to_opt_flag(), "-passes=strip-dead-prototypes");
    }

    #[test]
    fn optimize_ir_empty_passes_returns_input() {
        let ir = "; ModuleID = 'test'\n";
        let result = optimize_ir(ir, &[]).unwrap();
        assert_eq!(result, ir);
    }

    #[test]
    fn from_name_known_passes() {
        assert!(matches!(Pass::from_name("instcombine"), Pass::InstCombine));
        assert!(matches!(Pass::from_name("mem2reg"), Pass::MemToReg));
        assert!(matches!(Pass::from_name("gvn"), Pass::GVN));
    }

    #[test]
    fn from_name_unknown_is_custom() {
        match Pass::from_name("my-custom-pass") {
            Pass::Custom(s) => assert_eq!(s, "my-custom-pass"),
            _ => panic!("expected Custom"),
        }
    }
}
