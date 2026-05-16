#[derive(Debug, Clone)]
pub struct CompileTarget {
    pub triple: String,
    pub arch: Arch,
    pub os: Os,
    pub description: String,
}

#[derive(Debug, Clone, PartialEq)]
pub enum Arch {
    X86_64,
    AArch64,
    Arm,
    RiscV64,
    RiscV32,
    Wasm32,
}

#[derive(Debug, Clone, PartialEq)]
pub enum Os {
    Linux,
    MacOS,
    Windows,
    Freestanding,
    Wasi,
}

impl CompileTarget {
    pub fn host() -> Self {
        #[cfg(target_arch = "x86_64")]
        let arch = Arch::X86_64;
        #[cfg(target_arch = "aarch64")]
        let arch = Arch::AArch64;
        #[cfg(not(any(target_arch = "x86_64", target_arch = "aarch64")))]
        let arch = Arch::X86_64;

        #[cfg(target_os = "linux")]
        let os = Os::Linux;
        #[cfg(target_os = "macos")]
        let os = Os::MacOS;
        #[cfg(target_os = "windows")]
        let os = Os::Windows;
        #[cfg(not(any(target_os = "linux", target_os = "macos", target_os = "windows")))]
        let os = Os::Linux;

        Self::from_arch_os(arch, os)
    }

    pub fn from_triple(triple: &str) -> Option<Self> {
        match triple {
            "x86_64-unknown-linux-gnu" | "x86_64-linux-gnu" => {
                Some(Self::from_arch_os(Arch::X86_64, Os::Linux))
            }
            "aarch64-unknown-linux-gnu" | "aarch64-linux-gnu" => {
                Some(Self::from_arch_os(Arch::AArch64, Os::Linux))
            }
            "aarch64-apple-darwin" => Some(Self::from_arch_os(Arch::AArch64, Os::MacOS)),
            "x86_64-apple-darwin" => Some(Self::from_arch_os(Arch::X86_64, Os::MacOS)),
            "riscv64-unknown-linux-gnu" | "riscv64gc-unknown-linux-gnu" => {
                Some(Self::from_arch_os(Arch::RiscV64, Os::Linux))
            }
            "riscv32-unknown-linux-gnu" | "riscv32gc-unknown-linux-gnu" => {
                Some(Self::from_arch_os(Arch::RiscV32, Os::Linux))
            }
            "arm-unknown-linux-gnueabihf" => Some(Self::from_arch_os(Arch::Arm, Os::Linux)),
            "wasm32-wasi" => Some(Self::from_arch_os(Arch::Wasm32, Os::Wasi)),
            "riscv64-unknown-none-elf" => {
                Some(Self::from_arch_os(Arch::RiscV64, Os::Freestanding))
            }
            "aarch64-unknown-none" => Some(Self::from_arch_os(Arch::AArch64, Os::Freestanding)),
            _ => None,
        }
    }

    fn from_arch_os(arch: Arch, os: Os) -> Self {
        let triple = match (&arch, &os) {
            (Arch::X86_64, Os::Linux) => "x86_64-unknown-linux-gnu",
            (Arch::AArch64, Os::Linux) => "aarch64-unknown-linux-gnu",
            (Arch::AArch64, Os::MacOS) => "aarch64-apple-darwin",
            (Arch::X86_64, Os::MacOS) => "x86_64-apple-darwin",
            (Arch::RiscV64, Os::Linux) => "riscv64gc-unknown-linux-gnu",
            (Arch::RiscV32, Os::Linux) => "riscv32gc-unknown-linux-gnu",
            (Arch::Arm, Os::Linux) => "arm-unknown-linux-gnueabihf",
            (Arch::Wasm32, Os::Wasi) => "wasm32-wasi",
            (Arch::RiscV64, Os::Freestanding) => "riscv64-unknown-none-elf",
            (Arch::AArch64, Os::Freestanding) => "aarch64-unknown-none",
            _ => "unknown-unknown-unknown",
        };

        Self {
            triple: triple.to_string(),
            arch,
            os,
            description: format!("target: {triple}"),
        }
    }

    pub fn llvm_triple(&self) -> &str {
        &self.triple
    }

    pub fn data_layout(&self) -> &str {
        match self.arch {
            Arch::X86_64 => {
                "e-m:e-p270:32:32-p271:32:32-p272:64:64-i64:64-f80:128-n8:16:32:64-S128"
            }
            Arch::AArch64 => "e-m:e-i8:8:32-i16:16:32-i64:64-i128:128-n32:64-S128",
            Arch::RiscV64 => "e-m:e-p:64:64-i64:64-i128:128-n64-S128",
            Arch::RiscV32 => "e-m:e-p:32:32-i64:64-n32-S128",
            Arch::Arm => "e-m:e-p:32:32-Fi8-i64:64-v128:64:128-a:0:32-n32-S64",
            Arch::Wasm32 => "e-m:e-p:32:32-i64:64-n32:64-S128",
        }
    }

    pub fn all_targets() -> Vec<Self> {
        vec![
            Self::from_arch_os(Arch::X86_64, Os::Linux),
            Self::from_arch_os(Arch::X86_64, Os::MacOS),
            Self::from_arch_os(Arch::AArch64, Os::Linux),
            Self::from_arch_os(Arch::AArch64, Os::MacOS),
            Self::from_arch_os(Arch::RiscV64, Os::Linux),
            Self::from_arch_os(Arch::RiscV32, Os::Linux),
            Self::from_arch_os(Arch::Arm, Os::Linux),
            Self::from_arch_os(Arch::Wasm32, Os::Wasi),
            Self::from_arch_os(Arch::RiscV64, Os::Freestanding),
            Self::from_arch_os(Arch::AArch64, Os::Freestanding),
        ]
    }
}

impl std::fmt::Display for CompileTarget {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.triple)
    }
}

impl std::fmt::Display for Arch {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let s = match self {
            Arch::X86_64 => "x86_64",
            Arch::AArch64 => "aarch64",
            Arch::Arm => "arm",
            Arch::RiscV64 => "riscv64",
            Arch::RiscV32 => "riscv32",
            Arch::Wasm32 => "wasm32",
        };
        write!(f, "{s}")
    }
}

impl std::fmt::Display for Os {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let s = match self {
            Os::Linux => "linux",
            Os::MacOS => "macos",
            Os::Windows => "windows",
            Os::Freestanding => "freestanding",
            Os::Wasi => "wasi",
        };
        write!(f, "{s}")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn host_returns_valid_target() {
        let host = CompileTarget::host();
        assert!(!host.triple.is_empty());
        assert!(!host.description.is_empty());
    }

    #[test]
    fn from_triple_known_triples() {
        let triples = [
            "x86_64-unknown-linux-gnu",
            "aarch64-unknown-linux-gnu",
            "aarch64-apple-darwin",
            "x86_64-apple-darwin",
            "riscv64gc-unknown-linux-gnu",
            "riscv32gc-unknown-linux-gnu",
            "arm-unknown-linux-gnueabihf",
            "wasm32-wasi",
            "riscv64-unknown-none-elf",
            "aarch64-unknown-none",
        ];
        for triple in triples {
            assert!(
                CompileTarget::from_triple(triple).is_some(),
                "expected Some for triple '{triple}'"
            );
        }
    }

    #[test]
    fn from_triple_alias_triples() {
        assert!(CompileTarget::from_triple("x86_64-linux-gnu").is_some());
        assert!(CompileTarget::from_triple("aarch64-linux-gnu").is_some());
        assert!(CompileTarget::from_triple("riscv64-unknown-linux-gnu").is_some());
        assert!(CompileTarget::from_triple("riscv32-unknown-linux-gnu").is_some());
    }

    #[test]
    fn from_triple_returns_none_for_unknown() {
        assert!(CompileTarget::from_triple("sparc-unknown-linux-gnu").is_none());
        assert!(CompileTarget::from_triple("").is_none());
        assert!(CompileTarget::from_triple("foobar").is_none());
    }

    #[test]
    fn all_targets_returns_10() {
        assert_eq!(CompileTarget::all_targets().len(), 10);
    }

    #[test]
    fn data_layout_non_empty_for_each_arch() {
        let archs = [
            Arch::X86_64,
            Arch::AArch64,
            Arch::RiscV64,
            Arch::RiscV32,
            Arch::Arm,
            Arch::Wasm32,
        ];
        for arch in archs {
            let target = CompileTarget::from_arch_os(arch, Os::Linux);
            assert!(!target.data_layout().is_empty());
        }
    }

    #[test]
    fn llvm_triple_matches_stored_triple() {
        let target = CompileTarget::from_triple("aarch64-apple-darwin").unwrap();
        assert_eq!(target.llvm_triple(), "aarch64-apple-darwin");
    }

    #[test]
    fn display_formats() {
        let target = CompileTarget::from_triple("x86_64-unknown-linux-gnu").unwrap();
        assert_eq!(format!("{target}"), "x86_64-unknown-linux-gnu");
        assert_eq!(format!("{}", target.arch), "x86_64");
        assert_eq!(format!("{}", target.os), "linux");
    }

    #[test]
    fn host_arch_matches_cfg() {
        let host = CompileTarget::host();
        #[cfg(target_arch = "aarch64")]
        assert_eq!(host.arch, Arch::AArch64);
        #[cfg(target_arch = "x86_64")]
        assert_eq!(host.arch, Arch::X86_64);
    }

    #[test]
    fn host_os_matches_cfg() {
        let host = CompileTarget::host();
        #[cfg(target_os = "macos")]
        assert_eq!(host.os, Os::MacOS);
        #[cfg(target_os = "linux")]
        assert_eq!(host.os, Os::Linux);
    }
}
