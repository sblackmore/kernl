use crate::manifest::Manifest;
use std::path::Path;

pub fn run() -> Result<(), String> {
    let manifest_path = Path::new("kernl.toml");
    let manifest = Manifest::load(manifest_path)?;

    let pkg = &manifest.package;

    println!("package: {}", pkg.name);
    println!("version: {}", pkg.version);

    if let Some(desc) = &pkg.description {
        println!("description: {desc}");
    }

    if !pkg.authors.is_empty() {
        println!("authors: {}", pkg.authors.join(", "));
    }

    if let Some(license) = &pkg.license {
        println!("license: {license}");
    }

    println!("entry: {}", pkg.entry);

    if manifest.dependencies.is_empty() {
        println!("dependencies: (none)");
    } else {
        println!("dependencies:");
        for (name, dep) in &manifest.dependencies {
            match dep {
                crate::manifest::Dependency::Version(v) => {
                    println!("  {name} = \"{v}\"");
                }
                crate::manifest::Dependency::Detailed(d) => {
                    let mut parts = vec![];
                    if let Some(v) = &d.version {
                        parts.push(format!("version = \"{v}\""));
                    }
                    if let Some(p) = &d.path {
                        parts.push(format!("path = \"{p}\""));
                    }
                    if let Some(g) = &d.git {
                        parts.push(format!("git = \"{g}\""));
                    }
                    println!("  {name} {{ {} }}", parts.join(", "));
                }
            }
        }
    }

    Ok(())
}
