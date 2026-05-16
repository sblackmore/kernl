use crate::manifest::{Dependency, Manifest};
use std::path::Path;

pub fn run(name: &str, version: Option<&str>) -> Result<(), String> {
    let manifest_path = Path::new("kernl.toml");
    let mut manifest = Manifest::load(manifest_path)?;

    let dep = Dependency::Version(version.unwrap_or("*").to_string());
    manifest
        .dependencies
        .insert(name.to_string(), dep);

    manifest.save(manifest_path)?;

    let ver_display = version.unwrap_or("*");
    println!("added dependency `{name}` = \"{ver_display}\"");
    Ok(())
}

#[cfg(test)]
mod tests {
    use crate::manifest::Manifest;
    use std::fs;

    #[test]
    fn add_dependency_modifies_manifest() {
        let tmp = tempdir("add-modify");
        let manifest_path = tmp.join("kernl.toml");
        let initial = r#"[package]
name = "test-project"
version = "0.1.0"
"#;
        fs::write(&manifest_path, initial).unwrap();

        let mut manifest = Manifest::load(&manifest_path).unwrap();
        manifest.dependencies.insert(
            "math".into(),
            crate::manifest::Dependency::Version("0.2.0".into()),
        );
        manifest.save(&manifest_path).unwrap();

        let reloaded = Manifest::load(&manifest_path).unwrap();
        assert!(reloaded.dependencies.contains_key("math"));
    }

    #[test]
    fn add_dependency_with_wildcard() {
        let tmp = tempdir("add-wildcard");
        let manifest_path = tmp.join("kernl.toml");
        let initial = r#"[package]
name = "test-project"
version = "0.1.0"
"#;
        fs::write(&manifest_path, initial).unwrap();

        let mut manifest = Manifest::load(&manifest_path).unwrap();
        manifest.dependencies.insert(
            "utils".into(),
            crate::manifest::Dependency::Version("*".into()),
        );
        manifest.save(&manifest_path).unwrap();

        let reloaded = Manifest::load(&manifest_path).unwrap();
        assert!(reloaded.dependencies.contains_key("utils"));
    }

    fn tempdir(label: &str) -> std::path::PathBuf {
        let dir = std::env::temp_dir().join(format!(
            "kernl-add-test-{}-{label}",
            std::process::id()
        ));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        dir
    }
}
