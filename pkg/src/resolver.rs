use std::collections::HashMap;
use std::path::PathBuf;

use crate::manifest::{Dependency, Manifest};
use crate::registry::{Registry, RegistryErrorKind};

#[derive(Debug)]
pub struct ResolverError {
    pub message: String,
}

impl std::fmt::Display for ResolverError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.message)
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum DepSource {
    Registry,
    Path,
    Git,
}

#[derive(Debug, Clone)]
pub struct ResolvedDep {
    pub name: String,
    pub version: String,
    pub path: PathBuf,
    pub source: DepSource,
}

pub struct DependencyResolver {
    registry: Registry,
    cache_dir: PathBuf,
}

impl DependencyResolver {
    pub fn new() -> Self {
        let cache_dir = dirs_cache();
        Self {
            registry: Registry::new(),
            cache_dir,
        }
    }

    pub fn with_registry(registry: Registry) -> Self {
        let cache_dir = dirs_cache();
        Self {
            registry,
            cache_dir,
        }
    }

    /// Resolve all dependencies from the manifest.
    ///
    /// For each dependency:
    ///   - Path dependency: use the local path directly
    ///   - Version dependency: check cache, then try registry
    ///   - Git dependency: would clone the repo (stub for now)
    pub fn resolve(&self, manifest: &Manifest) -> Result<Vec<ResolvedDep>, ResolverError> {
        let mut resolved = Vec::new();

        for (name, dep) in &manifest.dependencies {
            let r = match dep {
                Dependency::Version(version) => self.resolve_version(name, version)?,
                Dependency::Detailed(detail) => {
                    if let Some(ref path) = detail.path {
                        self.resolve_path(name, path)?
                    } else if let Some(ref _git_url) = detail.git {
                        self.resolve_git(name, _git_url)?
                    } else if let Some(ref version) = detail.version {
                        self.resolve_version(name, version)?
                    } else {
                        return Err(ResolverError {
                            message: format!(
                                "dependency '{name}' has no version, path, or git source"
                            ),
                        });
                    }
                }
            };
            resolved.push(r);
        }

        Ok(resolved)
    }

    /// Install resolved dependencies into .kernl/deps/
    pub fn install(&self, deps: &[ResolvedDep]) -> Result<(), ResolverError> {
        let deps_dir = PathBuf::from(".kernl/deps");
        std::fs::create_dir_all(&deps_dir).map_err(|e| ResolverError {
            message: format!("failed to create .kernl/deps: {e}"),
        })?;

        for dep in deps {
            let dest = deps_dir.join(&dep.name);

            match dep.source {
                DepSource::Path => {
                    if !dep.path.exists() {
                        return Err(ResolverError {
                            message: format!(
                                "path dependency '{}' not found at {}",
                                dep.name,
                                dep.path.display()
                            ),
                        });
                    }
                    copy_dir(&dep.path, &dest)?;
                }
                DepSource::Registry => {
                    return Err(ResolverError {
                        message: format!(
                            "cannot install '{}' from registry — registry not yet available",
                            dep.name
                        ),
                    });
                }
                DepSource::Git => {
                    return Err(ResolverError {
                        message: format!(
                            "cannot install '{}' from git — git dependencies not yet supported",
                            dep.name
                        ),
                    });
                }
            }
        }

        Ok(())
    }

    fn resolve_version(&self, name: &str, version: &str) -> Result<ResolvedDep, ResolverError> {
        let cached = self.cache_dir.join(name).join(version);
        if cached.exists() {
            return Ok(ResolvedDep {
                name: name.into(),
                version: version.into(),
                path: cached,
                source: DepSource::Registry,
            });
        }

        match self.registry.get_package(name, version) {
            Ok(_info) => Ok(ResolvedDep {
                name: name.into(),
                version: version.into(),
                path: cached,
                source: DepSource::Registry,
            }),
            Err(e) if e.kind == RegistryErrorKind::NotAvailable => Err(ResolverError {
                message: format!(
                    "cannot resolve '{name}@{version}': {e}"
                ),
            }),
            Err(e) => Err(ResolverError {
                message: format!("failed to fetch '{name}@{version}': {e}"),
            }),
        }
    }

    fn resolve_path(&self, name: &str, path: &str) -> Result<ResolvedDep, ResolverError> {
        let path = PathBuf::from(path);
        if !path.exists() {
            return Err(ResolverError {
                message: format!(
                    "path dependency '{name}' not found at {}",
                    path.display()
                ),
            });
        }
        Ok(ResolvedDep {
            name: name.into(),
            version: "local".into(),
            path,
            source: DepSource::Path,
        })
    }

    fn resolve_git(&self, name: &str, url: &str) -> Result<ResolvedDep, ResolverError> {
        Err(ResolverError {
            message: format!(
                "git dependencies not yet supported ('{name}' from {url})"
            ),
        })
    }
}

fn dirs_cache() -> PathBuf {
    if let Some(home) = std::env::var_os("HOME") {
        PathBuf::from(home).join(".kernl").join("cache")
    } else {
        PathBuf::from(".kernl").join("cache")
    }
}

fn copy_dir(src: &std::path::Path, dst: &std::path::Path) -> Result<(), ResolverError> {
    if dst.exists() {
        std::fs::remove_dir_all(dst).map_err(|e| ResolverError {
            message: format!("failed to clean {}: {e}", dst.display()),
        })?;
    }

    std::fs::create_dir_all(dst).map_err(|e| ResolverError {
        message: format!("failed to create {}: {e}", dst.display()),
    })?;

    for entry in std::fs::read_dir(src).map_err(|e| ResolverError {
        message: format!("failed to read {}: {e}", src.display()),
    })? {
        let entry = entry.map_err(|e| ResolverError {
            message: format!("directory entry error: {e}"),
        })?;
        let src_path = entry.path();
        let dst_path = dst.join(entry.file_name());

        if src_path.is_dir() {
            copy_dir(&src_path, &dst_path)?;
        } else {
            std::fs::copy(&src_path, &dst_path).map_err(|e| ResolverError {
                message: format!(
                    "failed to copy {} -> {}: {e}",
                    src_path.display(),
                    dst_path.display()
                ),
            })?;
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::manifest::{DetailedDep, Manifest, Package};
    use std::fs;

    fn tempdir(label: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!(
            "kernl-resolver-test-{}-{label}",
            std::process::id()
        ));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn resolve_path_dependency() {
        let tmp = tempdir("path-dep");
        let dep_dir = tmp.join("my-lib");
        fs::create_dir_all(&dep_dir).unwrap();
        fs::write(dep_dir.join("main.knl"), "# lib").unwrap();

        let manifest = Manifest {
            package: Package {
                name: "test".into(),
                version: "0.1.0".into(),
                description: None,
                authors: vec![],
                license: None,
                entry: "src/main.knl".into(),
            },
            dependencies: HashMap::from([(
                "my-lib".into(),
                Dependency::Detailed(DetailedDep {
                    version: None,
                    path: Some(dep_dir.to_str().unwrap().into()),
                    git: None,
                }),
            )]),
        };

        let resolver = DependencyResolver::new();
        let deps = resolver.resolve(&manifest).unwrap();
        assert_eq!(deps.len(), 1);
        assert_eq!(deps[0].name, "my-lib");
        assert_eq!(deps[0].source, DepSource::Path);
    }

    #[test]
    fn resolve_no_dependencies_succeeds() {
        let manifest = Manifest {
            package: Package {
                name: "empty".into(),
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

    #[test]
    fn resolve_missing_path_errors() {
        let manifest = Manifest {
            package: Package {
                name: "test".into(),
                version: "0.1.0".into(),
                description: None,
                authors: vec![],
                license: None,
                entry: "src/main.knl".into(),
            },
            dependencies: HashMap::from([(
                "missing".into(),
                Dependency::Detailed(DetailedDep {
                    version: None,
                    path: Some("/nonexistent/path/to/dep".into()),
                    git: None,
                }),
            )]),
        };

        let resolver = DependencyResolver::new();
        let err = resolver.resolve(&manifest).unwrap_err();
        assert!(err.message.contains("not found"));
    }

    #[test]
    fn resolve_version_dep_returns_not_available() {
        let manifest = Manifest {
            package: Package {
                name: "test".into(),
                version: "0.1.0".into(),
                description: None,
                authors: vec![],
                license: None,
                entry: "src/main.knl".into(),
            },
            dependencies: HashMap::from([("math".into(), Dependency::Version("0.1.0".into()))]),
        };

        let resolver = DependencyResolver::new();
        let err = resolver.resolve(&manifest).unwrap_err();
        assert!(err.message.contains("not available") || err.message.contains("math"));
    }
}
