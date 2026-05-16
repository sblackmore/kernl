use std::path::Path;

use crate::manifest::Manifest;
use crate::resolver::DependencyResolver;

pub fn run() -> Result<(), String> {
    let manifest_path = Path::new("kernl.toml");
    let manifest = Manifest::load(manifest_path)?;

    println!(
        "resolving dependencies for {} v{}...",
        manifest.package.name, manifest.package.version
    );

    let resolver = DependencyResolver::new();

    let deps = resolver
        .resolve(&manifest)
        .map_err(|e| format!("dependency resolution failed: {e}"))?;

    if deps.is_empty() {
        println!("no dependencies to install");
        return Ok(());
    }

    println!("resolved {} dependency(ies):", deps.len());
    for dep in &deps {
        let source = match dep.source {
            crate::resolver::DepSource::Registry => "registry",
            crate::resolver::DepSource::Path => "path",
            crate::resolver::DepSource::Git => "git",
        };
        println!(
            "  {} v{} ({})",
            dep.name, dep.version, source
        );
    }

    resolver
        .install(&deps)
        .map_err(|e| format!("installation failed: {e}"))?;

    println!("installed {} dependency(ies) to .kernl/deps/", deps.len());
    Ok(())
}

#[cfg(test)]
mod tests {
    use crate::manifest::{Manifest, Package};
    use crate::resolver::DependencyResolver;
    use std::collections::HashMap;

    #[test]
    fn install_with_no_dependencies_succeeds() {
        let manifest = Manifest {
            package: Package {
                name: "empty-project".into(),
                version: "0.1.0".into(),
                description: None,
                authors: vec![],
                license: None,
                entry: "src/main.knl".into(),
            },
            dependencies: HashMap::new(),
        };

        let resolver = DependencyResolver::new();
        let deps = resolver.resolve(&manifest).unwrap();
        assert!(deps.is_empty());
    }
}
