use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Debug, Serialize, Deserialize)]
pub struct Manifest {
    pub package: Package,
    #[serde(default)]
    pub dependencies: HashMap<String, Dependency>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Package {
    pub name: String,
    pub version: String,
    #[serde(default)]
    pub description: Option<String>,
    #[serde(default)]
    pub authors: Vec<String>,
    #[serde(default)]
    pub license: Option<String>,
    #[serde(default = "default_entry")]
    pub entry: String,
}

fn default_entry() -> String {
    "src/main.knl".into()
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(untagged)]
pub enum Dependency {
    Version(String),
    Detailed(DetailedDep),
}

#[derive(Debug, Serialize, Deserialize)]
pub struct DetailedDep {
    pub version: Option<String>,
    pub path: Option<String>,
    pub git: Option<String>,
}

impl Manifest {
    pub fn from_str(s: &str) -> Result<Self, toml::de::Error> {
        toml::from_str(s)
    }

    pub fn to_string_pretty(&self) -> Result<String, toml::ser::Error> {
        toml::to_string_pretty(self)
    }

    pub fn load(path: &std::path::Path) -> Result<Self, String> {
        let content = std::fs::read_to_string(path)
            .map_err(|e| format!("failed to read {}: {e}", path.display()))?;
        Self::from_str(&content)
            .map_err(|e| format!("failed to parse {}: {e}", path.display()))
    }

    pub fn save(&self, path: &std::path::Path) -> Result<(), String> {
        let content = self
            .to_string_pretty()
            .map_err(|e| format!("failed to serialize manifest: {e}"))?;
        std::fs::write(path, content)
            .map_err(|e| format!("failed to write {}: {e}", path.display()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_minimal_manifest() {
        let toml_str = r#"
[package]
name = "my-project"
version = "0.1.0"
"#;
        let manifest = Manifest::from_str(toml_str).unwrap();
        assert_eq!(manifest.package.name, "my-project");
        assert_eq!(manifest.package.version, "0.1.0");
        assert_eq!(manifest.package.entry, "src/main.knl");
        assert!(manifest.dependencies.is_empty());
    }

    #[test]
    fn parse_full_manifest() {
        let toml_str = r#"
[package]
name = "my-project"
version = "1.0.0"
description = "A cool project"
authors = ["Alice", "Bob"]
license = "MIT"
entry = "src/app.knl"

[dependencies]
math = "0.2.0"
utils = { version = "1.0.0", path = "../utils" }
remote = { git = "https://github.com/example/remote" }
"#;
        let manifest = Manifest::from_str(toml_str).unwrap();
        assert_eq!(manifest.package.name, "my-project");
        assert_eq!(manifest.package.description.as_deref(), Some("A cool project"));
        assert_eq!(manifest.package.authors.len(), 2);
        assert_eq!(manifest.package.entry, "src/app.knl");
        assert_eq!(manifest.dependencies.len(), 3);
    }

    #[test]
    fn roundtrip_manifest() {
        let manifest = Manifest {
            package: Package {
                name: "test".into(),
                version: "0.1.0".into(),
                description: Some("A test".into()),
                authors: vec!["Dev".into()],
                license: Some("MIT".into()),
                entry: "src/main.knl".into(),
            },
            dependencies: HashMap::new(),
        };
        let serialized = manifest.to_string_pretty().unwrap();
        let deserialized = Manifest::from_str(&serialized).unwrap();
        assert_eq!(deserialized.package.name, "test");
        assert_eq!(deserialized.package.version, "0.1.0");
    }
}
