use crate::manifest::{Manifest, Package};
use std::collections::HashMap;
use std::fs;
use std::path::Path;

const HELLO_WORLD: &str = "fn main\n  do print \"hello kernl\"\n";

const GITIGNORE: &str = "/target\n/build\n";

pub fn run(name: Option<&str>) -> Result<(), String> {
    let project_dir = match name {
        Some(n) => {
            let dir = Path::new(n);
            fs::create_dir_all(dir)
                .map_err(|e| format!("failed to create directory {}: {e}", dir.display()))?;
            dir.to_path_buf()
        }
        None => std::env::current_dir()
            .map_err(|e| format!("failed to get current directory: {e}"))?,
    };

    let project_name = project_dir
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("my-project");

    let manifest_path = project_dir.join("kernl.toml");
    if manifest_path.exists() {
        return Err(format!(
            "kernl.toml already exists in {}",
            project_dir.display()
        ));
    }

    let manifest = Manifest {
        package: Package {
            name: project_name.to_string(),
            version: "0.1.0".into(),
            description: None,
            authors: vec![],
            license: None,
            entry: "src/main.knl".into(),
        },
        dependencies: HashMap::new(),
    };

    manifest.save(&manifest_path)?;

    let src_dir = project_dir.join("src");
    fs::create_dir_all(&src_dir)
        .map_err(|e| format!("failed to create src directory: {e}"))?;

    let main_knl = src_dir.join("main.knl");
    fs::write(&main_knl, HELLO_WORLD)
        .map_err(|e| format!("failed to write {}: {e}", main_knl.display()))?;

    let gitignore_path = project_dir.join(".gitignore");
    fs::write(&gitignore_path, GITIGNORE)
        .map_err(|e| format!("failed to write .gitignore: {e}"))?;

    println!("created kernl project `{project_name}` in {}", project_dir.display());
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    #[test]
    fn init_creates_project_structure() {
        let tmp = tempdir("init-create");
        let project_dir = tmp.join("test-project");

        run(Some(project_dir.to_str().unwrap())).unwrap();

        assert!(project_dir.join("kernl.toml").exists());
        assert!(project_dir.join("src/main.knl").exists());
        assert!(project_dir.join(".gitignore").exists());

        let manifest_content = fs::read_to_string(project_dir.join("kernl.toml")).unwrap();
        let manifest = Manifest::from_str(&manifest_content).unwrap();
        assert_eq!(manifest.package.name, "test-project");
        assert_eq!(manifest.package.version, "0.1.0");
        assert_eq!(manifest.package.entry, "src/main.knl");

        let main_content = fs::read_to_string(project_dir.join("src/main.knl")).unwrap();
        assert!(main_content.contains("hello kernl"));

        let gitignore = fs::read_to_string(project_dir.join(".gitignore")).unwrap();
        assert!(gitignore.contains("/target"));
        assert!(gitignore.contains("/build"));
    }

    #[test]
    fn init_refuses_existing_project() {
        let tmp = tempdir("init-refuse");
        let project_dir = tmp.join("existing");
        fs::create_dir_all(&project_dir).unwrap();
        fs::write(project_dir.join("kernl.toml"), "[package]\nname = \"x\"\nversion = \"0.1.0\"\n").unwrap();

        let result = run(Some(project_dir.to_str().unwrap()));
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("already exists"));
    }

    fn tempdir(label: &str) -> std::path::PathBuf {
        let dir = std::env::temp_dir().join(format!(
            "kernl-test-{}-{label}",
            std::process::id()
        ));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        dir
    }
}
