use std::path::Path;

use crate::manifest::Manifest;
use crate::registry::{Registry, RegistryErrorKind};

pub fn run() -> Result<(), String> {
    let manifest_path = Path::new("kernl.toml");
    let manifest = Manifest::load(manifest_path)?;

    println!(
        "packaging {} v{}...",
        manifest.package.name, manifest.package.version
    );

    let tarball = create_tarball(&manifest)?;
    println!("created tarball ({} bytes)", tarball.len());

    let registry = Registry::new();
    match registry.publish(&tarball, &manifest) {
        Ok(()) => {
            println!(
                "published {} v{} to {}",
                manifest.package.name,
                manifest.package.version,
                registry.base_url()
            );
            Ok(())
        }
        Err(e) if e.kind == RegistryErrorKind::NotAvailable => {
            eprintln!(
                "note: the kernl package registry is not yet available.\n\
                 publishing will be supported once the registry launches.\n\
                 see https://kernl-lang.org/registry for updates."
            );
            Ok(())
        }
        Err(e) => Err(format!("publish failed: {e}")),
    }
}

fn create_tarball(manifest: &Manifest) -> Result<Vec<u8>, String> {
    let mut files: Vec<String> = Vec::new();

    files.push("kernl.toml".into());

    let entry = Path::new(&manifest.package.entry);
    if entry.exists() {
        files.push(manifest.package.entry.clone());
    }

    let src = Path::new("src");
    if src.is_dir() {
        collect_knl_files(src, &mut files)?;
    }

    if Path::new("README.md").exists() {
        files.push("README.md".into());
    }
    if Path::new("LICENSE").exists() {
        files.push("LICENSE".into());
    }

    // Stub: in production this would create a real .tar.gz
    // For now, collect file contents into a simple concatenation
    let mut data = Vec::new();
    for file in &files {
        let path = Path::new(file);
        if path.exists() {
            let content = std::fs::read(path)
                .map_err(|e| format!("failed to read {file}: {e}"))?;
            data.extend_from_slice(file.as_bytes());
            data.push(b'\0');
            data.extend_from_slice(&(content.len() as u32).to_le_bytes());
            data.extend_from_slice(&content);
        }
    }

    println!("files to publish:");
    for file in &files {
        println!("  {file}");
    }

    Ok(data)
}

fn collect_knl_files(dir: &Path, files: &mut Vec<String>) -> Result<(), String> {
    let entries = std::fs::read_dir(dir)
        .map_err(|e| format!("failed to read {}: {e}", dir.display()))?;

    for entry in entries {
        let entry = entry.map_err(|e| format!("directory entry error: {e}"))?;
        let path = entry.path();

        if path.is_dir() {
            collect_knl_files(&path, files)?;
        } else if path.extension().is_some_and(|ext| ext == "knl") {
            if let Some(s) = path.to_str() {
                files.push(s.into());
            }
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    #[test]
    fn publish_not_available_is_not_error() {
        // publish requires kernl.toml to exist, so we just test
        // that the module compiles and the registry stub works
        let registry = crate::registry::Registry::new();
        let manifest = crate::manifest::Manifest {
            package: crate::manifest::Package {
                name: "test".into(),
                version: "0.1.0".into(),
                description: None,
                authors: vec![],
                license: None,
                entry: "src/main.knl".into(),
            },
            dependencies: std::collections::HashMap::new(),
        };
        let err = registry.publish(b"fake-tarball", &manifest).unwrap_err();
        assert_eq!(
            err.kind,
            crate::registry::RegistryErrorKind::NotAvailable
        );
    }
}
