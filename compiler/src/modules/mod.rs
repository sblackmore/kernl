use crate::parser::ast::{Program, Item};
use std::collections::HashMap;
use std::fmt;
use std::path::PathBuf;

// ---------------------------------------------------------------------------
// Error
// ---------------------------------------------------------------------------

#[derive(Debug)]
pub struct ModuleError {
    pub message: String,
    pub path: Option<PathBuf>,
}

impl fmt::Display for ModuleError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if let Some(ref p) = self.path {
            write!(f, "module error: {} ({})", self.message, p.display())
        } else {
            write!(f, "module error: {}", self.message)
        }
    }
}

impl std::error::Error for ModuleError {}

// ---------------------------------------------------------------------------
// Module
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
pub struct Module {
    pub name: String,
    pub path: PathBuf,
    pub source: String,
    pub dependencies: Vec<String>,
}

// ---------------------------------------------------------------------------
// ModuleResolver
// ---------------------------------------------------------------------------

pub struct ModuleResolver {
    root: PathBuf,
    loaded: HashMap<String, Module>,
}

impl ModuleResolver {
    pub fn new(root: PathBuf) -> Self {
        Self {
            root,
            loaded: HashMap::new(),
        }
    }

    /// Resolve a `use` path to a file on disk.
    ///
    /// `use io.print`       → `<root>/io.knl` or `<root>/io/mod.knl`
    /// `use math.trig.sin`  → `<root>/math/trig.knl` or `<root>/math/trig/sin.knl`
    pub fn resolve(&self, use_path: &[String]) -> Result<PathBuf, ModuleError> {
        if use_path.is_empty() {
            return Err(ModuleError {
                message: "empty use path".into(),
                path: None,
            });
        }

        // Strategy 1: join all-but-last as directories, last segment as `.knl` file
        //   use io.print  → <root>/io.knl
        //   use math.trig.sin → <root>/math/trig.knl
        let (dirs, _leaf) = use_path.split_at(use_path.len() - 1);

        if !dirs.is_empty() {
            let mut candidate = self.root.clone();
            for d in &dirs[..dirs.len() - 1] {
                candidate.push(d);
            }
            candidate.push(format!("{}.knl", dirs[dirs.len() - 1]));
            if candidate.exists() {
                return Ok(candidate);
            }
        }

        // Strategy 2: join all segments as directories, try `mod.knl` or last segment `.knl`
        //   use io.print → <root>/io/mod.knl
        //   use math.trig.sin → <root>/math/trig/sin.knl
        if !dirs.is_empty() {
            let mut candidate = self.root.clone();
            for d in dirs {
                candidate.push(d);
            }
            candidate.push("mod.knl");
            if candidate.exists() {
                return Ok(candidate);
            }
        }

        // Strategy 3: full path with last segment as file
        {
            let mut candidate = self.root.clone();
            for segment in use_path {
                candidate.push(segment);
            }
            candidate.set_extension("knl");
            if candidate.exists() {
                return Ok(candidate);
            }
        }

        // Strategy 4: single-segment → `<root>/<name>.knl`
        if use_path.len() == 1 {
            let candidate = self.root.join(format!("{}.knl", use_path[0]));
            if candidate.exists() {
                return Ok(candidate);
            }
        }

        let full: String = use_path.join(".");
        Err(ModuleError {
            message: format!("module not found: {full}"),
            path: Some(self.root.clone()),
        })
    }

    /// Read a module file, extract `use` declarations as dependencies.
    pub fn load_module(&mut self, name: &str) -> Result<Module, ModuleError> {
        if let Some(m) = self.loaded.get(name) {
            return Ok(m.clone());
        }

        let parts: Vec<String> = name.split('.').map(String::from).collect();
        let path = self.resolve(&parts)?;

        let source = std::fs::read_to_string(&path).map_err(|e| ModuleError {
            message: format!("cannot read {}: {e}", path.display()),
            path: Some(path.clone()),
        })?;

        let dependencies = extract_use_names(&source);

        let module = Module {
            name: name.to_string(),
            path,
            source,
            dependencies,
        };

        self.loaded.insert(name.to_string(), module.clone());
        Ok(module)
    }

    /// Walk all `Use` items in a program, resolve and load each module.
    pub fn resolve_all(&mut self, program: &Program) -> Result<Vec<Module>, ModuleError> {
        let mut modules = Vec::new();

        for item in &program.items {
            if let Item::Use(use_decl) = item {
                let name = use_decl.path.join(".");
                let module = self.load_module(&name)?;
                modules.push(module);
            }
        }

        Ok(modules)
    }
}

// ---------------------------------------------------------------------------
// Dependency graph — topological sort with cycle detection
// ---------------------------------------------------------------------------

pub fn build_graph(modules: &[Module]) -> Result<Vec<String>, ModuleError> {
    let mut in_degree: HashMap<&str, usize> = HashMap::new();
    let mut adjacency: HashMap<&str, Vec<&str>> = HashMap::new();

    for m in modules {
        in_degree.entry(&m.name).or_insert(0);
        adjacency.entry(&m.name).or_default();
        for dep in &m.dependencies {
            in_degree.entry(dep.as_str()).or_insert(0);
            adjacency.entry(dep.as_str()).or_default().push(&m.name);
            *in_degree.entry(m.name.as_str()).or_insert(0) += 1;
        }
    }

    let mut queue: Vec<&str> = in_degree
        .iter()
        .filter(|&(_, &deg)| deg == 0)
        .map(|(&name, _)| name)
        .collect();
    queue.sort(); // deterministic ordering

    let mut order: Vec<String> = Vec::new();

    while let Some(node) = queue.pop() {
        order.push(node.to_string());
        if let Some(deps) = adjacency.get(node) {
            for &dep in deps {
                if let Some(deg) = in_degree.get_mut(dep) {
                    *deg -= 1;
                    if *deg == 0 {
                        queue.push(dep);
                        queue.sort();
                    }
                }
            }
        }
    }

    if order.len() < in_degree.len() {
        let remaining: Vec<String> = in_degree
            .iter()
            .filter(|&(_, &deg)| deg > 0)
            .map(|(&name, _)| name.to_string())
            .collect();
        return Err(ModuleError {
            message: format!("circular dependency among: {}", remaining.join(", ")),
            path: None,
        });
    }

    Ok(order)
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Minimal extraction of `use foo.bar` lines from raw source text.
fn extract_use_names(source: &str) -> Vec<String> {
    source
        .lines()
        .filter_map(|line| {
            let trimmed = line.trim();
            if trimmed.starts_with("use ") {
                Some(trimmed[4..].trim().to_string())
            } else {
                None
            }
        })
        .collect()
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::sync::atomic::{AtomicU64, Ordering};

    fn tmp_dir() -> PathBuf {
        static COUNTER: AtomicU64 = AtomicU64::new(0);
        let id = COUNTER.fetch_add(1, Ordering::SeqCst);
        let dir = std::env::temp_dir().join(format!(
            "kernl_mod_test_{}_{id}",
            std::process::id()
        ));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn resolve_flat_file() {
        let root = tmp_dir();
        fs::write(root.join("io.knl"), "fn print\n  in s: str\n  do 0").unwrap();

        let resolver = ModuleResolver::new(root.clone());
        let path = resolver
            .resolve(&["io".into(), "print".into()])
            .unwrap();
        assert_eq!(path, root.join("io.knl"));

        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn resolve_nested_file() {
        let root = tmp_dir();
        let nested = root.join("math").join("trig");
        fs::create_dir_all(&nested).unwrap();
        fs::write(nested.join("sin.knl"), "fn sin\n  in x: float\n  do 0").unwrap();

        let resolver = ModuleResolver::new(root.clone());
        let path = resolver
            .resolve(&["math".into(), "trig".into(), "sin".into()])
            .unwrap();
        assert_eq!(path, nested.join("sin.knl"));

        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn resolve_mod_file() {
        let root = tmp_dir();
        let io_dir = root.join("io");
        fs::create_dir_all(&io_dir).unwrap();
        fs::write(io_dir.join("mod.knl"), "fn print\n  in s: str\n  do 0").unwrap();

        let resolver = ModuleResolver::new(root.clone());
        let path = resolver
            .resolve(&["io".into(), "print".into()])
            .unwrap();
        assert_eq!(path, io_dir.join("mod.knl"));

        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn resolve_missing_module() {
        let root = tmp_dir();
        let resolver = ModuleResolver::new(root.clone());
        let result = resolver.resolve(&["nope".into()]);
        assert!(result.is_err());
        assert!(result.unwrap_err().message.contains("not found"));

        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn circular_dependency_detected() {
        let modules = vec![
            Module {
                name: "a".into(),
                path: PathBuf::from("a.knl"),
                source: String::new(),
                dependencies: vec!["b".into()],
            },
            Module {
                name: "b".into(),
                path: PathBuf::from("b.knl"),
                source: String::new(),
                dependencies: vec!["a".into()],
            },
        ];

        let result = build_graph(&modules);
        assert!(result.is_err());
        assert!(result.unwrap_err().message.contains("circular"));
    }

    #[test]
    fn topological_sort_ordering() {
        let modules = vec![
            Module {
                name: "app".into(),
                path: PathBuf::from("app.knl"),
                source: String::new(),
                dependencies: vec!["io".into(), "math".into()],
            },
            Module {
                name: "io".into(),
                path: PathBuf::from("io.knl"),
                source: String::new(),
                dependencies: vec![],
            },
            Module {
                name: "math".into(),
                path: PathBuf::from("math.knl"),
                source: String::new(),
                dependencies: vec![],
            },
        ];

        let order = build_graph(&modules).unwrap();
        let app_pos = order.iter().position(|n| n == "app").unwrap();
        let io_pos = order.iter().position(|n| n == "io").unwrap();
        let math_pos = order.iter().position(|n| n == "math").unwrap();

        // app depends on io and math, so app comes first (it has in-degree 0),
        // then its deps get resolved. Actually the topo sort emits sources first.
        // With our implementation, nodes with 0 in-degree come first.
        // io and math have 0 in-degree; app has in-degree 2.
        assert!(io_pos < app_pos, "io should come before app");
        assert!(math_pos < app_pos, "math should come before app");
    }
}
