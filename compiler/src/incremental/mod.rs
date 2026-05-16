use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::time::SystemTime;

fn content_hash(content: &str) -> String {
    let mut hash: u64 = 0xcbf29ce484222325; // FNV-1a offset basis
    for byte in content.bytes() {
        hash ^= byte as u64;
        hash = hash.wrapping_mul(0x100000001b3); // FNV-1a prime
    }
    format!("{hash:016x}")
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct CacheEntry {
    pub file_path: String,
    pub content_hash: String,
    pub modified_at: u64,
    pub ast_json: String,
    pub llvm_ir: Option<String>,
    pub wat: Option<String>,
    pub type_errors: Vec<String>,
    pub semantic_errors: Vec<String>,
}

#[derive(Debug, serde::Serialize, serde::Deserialize)]
pub struct CompilationCache {
    pub version: String,
    pub entries: HashMap<String, CacheEntry>,
}

impl CompilationCache {
    pub fn new() -> Self {
        Self {
            version: env!("CARGO_PKG_VERSION").into(),
            entries: HashMap::new(),
        }
    }

    pub fn load(cache_dir: &Path) -> Self {
        let path = cache_dir.join("cache.json");
        if let Ok(data) = fs::read_to_string(&path) {
            if let Ok(cache) = serde_json::from_str::<CompilationCache>(&data) {
                if cache.version == env!("CARGO_PKG_VERSION") {
                    return cache;
                }
            }
        }
        Self::new()
    }

    pub fn save(&self, cache_dir: &Path) -> Result<(), std::io::Error> {
        fs::create_dir_all(cache_dir)?;
        let path = cache_dir.join("cache.json");
        let json = serde_json::to_string_pretty(self)?;
        fs::write(path, json)
    }

    pub fn is_stale(&self, file_path: &str, content: &str) -> bool {
        match self.entries.get(file_path) {
            Some(entry) => entry.content_hash != content_hash(content),
            None => true,
        }
    }

    pub fn update(&mut self, file_path: &str, _content: &str, entry: CacheEntry) {
        self.entries.insert(file_path.to_string(), entry);
    }

    pub fn prune(&mut self) {
        self.entries.retain(|path, _| Path::new(path).exists());
    }

    pub fn get(&self, file_path: &str, content: &str) -> Option<&CacheEntry> {
        let entry = self.entries.get(file_path)?;
        if entry.content_hash == content_hash(content) {
            Some(entry)
        } else {
            None
        }
    }

    pub fn stats(&self) -> CacheStats {
        CacheStats {
            total_entries: self.entries.len(),
            total_size_bytes: serde_json::to_string(self).map(|s| s.len()).unwrap_or(0),
        }
    }
}

pub struct CacheStats {
    pub total_entries: usize,
    pub total_size_bytes: usize,
}

pub struct IncrementalCompiler {
    cache: CompilationCache,
    cache_dir: PathBuf,
}

impl IncrementalCompiler {
    pub fn new(project_root: &Path) -> Self {
        let cache_dir = project_root.join(".kernl");
        let cache = CompilationCache::load(&cache_dir);
        Self { cache, cache_dir }
    }

    pub fn compile_file(
        &mut self,
        file_path: &Path,
        target: &crate::codegen::Target,
    ) -> Result<CompileFileResult, String> {
        let content = fs::read_to_string(file_path)
            .map_err(|e| format!("cannot read {}: {e}", file_path.display()))?;

        let path_str = file_path.to_string_lossy().to_string();

        if let Some(cached) = self.cache.get(&path_str, &content) {
            return Ok(CompileFileResult {
                output: cached
                    .llvm_ir
                    .clone()
                    .or(cached.wat.clone())
                    .unwrap_or_default(),
                from_cache: true,
                type_errors: cached.type_errors.clone(),
                semantic_errors: cached.semantic_errors.clone(),
            });
        }

        let result = crate::compile(&content, target.clone()).map_err(|e| e.to_string())?;

        let modified = fs::metadata(file_path)
            .and_then(|m| m.modified())
            .and_then(|t| {
                t.duration_since(SystemTime::UNIX_EPOCH)
                    .map_err(|e| std::io::Error::other(e))
            })
            .map(|d| d.as_secs())
            .unwrap_or(0);

        let entry = CacheEntry {
            file_path: path_str.clone(),
            content_hash: content_hash(&content),
            modified_at: modified,
            ast_json: String::new(),
            llvm_ir: if matches!(target, crate::codegen::Target::LlvmIr) {
                Some(result.output.clone())
            } else {
                None
            },
            wat: if matches!(target, crate::codegen::Target::Wasm) {
                Some(result.output.clone())
            } else {
                None
            },
            type_errors: result.type_errors.clone(),
            semantic_errors: result.semantic_errors.clone(),
        };

        self.cache.update(&path_str, &content, entry);
        let _ = self.cache.save(&self.cache_dir);

        Ok(CompileFileResult {
            output: result.output,
            from_cache: false,
            type_errors: result.type_errors,
            semantic_errors: result.semantic_errors,
        })
    }

    pub fn stats(&self) -> CacheStats {
        self.cache.stats()
    }

    pub fn clear_cache(&mut self) {
        self.cache = CompilationCache::new();
        let _ = self.cache.save(&self.cache_dir);
    }
}

pub struct CompileFileResult {
    pub output: String,
    pub from_cache: bool,
    pub type_errors: Vec<String>,
    pub semantic_errors: Vec<String>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    #[test]
    fn test_content_hash_consistent() {
        let a = content_hash("hello world");
        let b = content_hash("hello world");
        assert_eq!(a, b);
    }

    #[test]
    fn test_content_hash_changes_with_content() {
        let a = content_hash("hello world");
        let b = content_hash("hello world!");
        assert_ne!(a, b);
    }

    #[test]
    fn test_content_hash_format() {
        let h = content_hash("test");
        assert_eq!(h.len(), 16);
        assert!(h.chars().all(|c| c.is_ascii_hexdigit()));
    }

    #[test]
    fn test_is_stale_new_file() {
        let cache = CompilationCache::new();
        assert!(cache.is_stale("new_file.knl", "content"));
    }

    #[test]
    fn test_is_stale_cached_unchanged() {
        let mut cache = CompilationCache::new();
        let content = "fn main\n  do add 1 2";
        let entry = CacheEntry {
            file_path: "test.knl".into(),
            content_hash: content_hash(content),
            modified_at: 0,
            ast_json: String::new(),
            llvm_ir: None,
            wat: None,
            type_errors: vec![],
            semantic_errors: vec![],
        };
        cache.update("test.knl", content, entry);
        assert!(!cache.is_stale("test.knl", content));
    }

    #[test]
    fn test_is_stale_cached_changed() {
        let mut cache = CompilationCache::new();
        let content = "fn main\n  do add 1 2";
        let entry = CacheEntry {
            file_path: "test.knl".into(),
            content_hash: content_hash(content),
            modified_at: 0,
            ast_json: String::new(),
            llvm_ir: None,
            wat: None,
            type_errors: vec![],
            semantic_errors: vec![],
        };
        cache.update("test.knl", content, entry);
        assert!(cache.is_stale("test.knl", "fn main\n  do add 1 3"));
    }

    #[test]
    fn test_new_cache_is_empty() {
        let cache = CompilationCache::new();
        assert!(cache.entries.is_empty());
        assert_eq!(cache.version, env!("CARGO_PKG_VERSION"));
    }

    #[test]
    fn test_cache_save_and_load_roundtrip() {
        let dir = std::env::temp_dir().join("kernl_test_cache_roundtrip");
        let _ = fs::remove_dir_all(&dir);

        let mut cache = CompilationCache::new();
        let entry = CacheEntry {
            file_path: "example.knl".into(),
            content_hash: content_hash("hello"),
            modified_at: 12345,
            ast_json: String::new(),
            llvm_ir: Some("define i64 @main()".into()),
            wat: None,
            type_errors: vec![],
            semantic_errors: vec![],
        };
        cache.update("example.knl", "hello", entry);
        cache.save(&dir).unwrap();

        let loaded = CompilationCache::load(&dir);
        assert_eq!(loaded.entries.len(), 1);
        let e = loaded.entries.get("example.knl").unwrap();
        assert_eq!(e.llvm_ir.as_deref(), Some("define i64 @main()"));
        assert_eq!(e.modified_at, 12345);

        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn test_prune_removes_nonexistent_files() {
        let mut cache = CompilationCache::new();
        let entry = CacheEntry {
            file_path: "/nonexistent/path/test.knl".into(),
            content_hash: "abc".into(),
            modified_at: 0,
            ast_json: String::new(),
            llvm_ir: None,
            wat: None,
            type_errors: vec![],
            semantic_errors: vec![],
        };
        cache
            .entries
            .insert("/nonexistent/path/test.knl".into(), entry);
        assert_eq!(cache.entries.len(), 1);
        cache.prune();
        assert!(cache.entries.is_empty());
    }

    #[test]
    fn test_get_returns_none_for_missing() {
        let cache = CompilationCache::new();
        assert!(cache.get("missing.knl", "content").is_none());
    }

    #[test]
    fn test_get_returns_none_for_stale() {
        let mut cache = CompilationCache::new();
        let entry = CacheEntry {
            file_path: "test.knl".into(),
            content_hash: content_hash("old content"),
            modified_at: 0,
            ast_json: String::new(),
            llvm_ir: None,
            wat: None,
            type_errors: vec![],
            semantic_errors: vec![],
        };
        cache.update("test.knl", "old content", entry);
        assert!(cache.get("test.knl", "new content").is_none());
    }

    #[test]
    fn test_get_returns_entry_for_fresh() {
        let mut cache = CompilationCache::new();
        let content = "fn main\n  do 42";
        let entry = CacheEntry {
            file_path: "test.knl".into(),
            content_hash: content_hash(content),
            modified_at: 0,
            ast_json: String::new(),
            llvm_ir: Some("ir output".into()),
            wat: None,
            type_errors: vec![],
            semantic_errors: vec![],
        };
        cache.update("test.knl", content, entry);
        let got = cache.get("test.knl", content).unwrap();
        assert_eq!(got.llvm_ir.as_deref(), Some("ir output"));
    }

    #[test]
    fn test_stats() {
        let mut cache = CompilationCache::new();
        assert_eq!(cache.stats().total_entries, 0);

        let entry = CacheEntry {
            file_path: "a.knl".into(),
            content_hash: "h".into(),
            modified_at: 0,
            ast_json: String::new(),
            llvm_ir: None,
            wat: None,
            type_errors: vec![],
            semantic_errors: vec![],
        };
        cache.entries.insert("a.knl".into(), entry);
        assert_eq!(cache.stats().total_entries, 1);
        assert!(cache.stats().total_size_bytes > 0);
    }

    #[test]
    fn test_incremental_compiler_caches_file() {
        let dir = std::env::temp_dir().join("kernl_test_incr");
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();

        let knl_path = dir.join("test.knl");
        fs::write(&knl_path, "fn main\n  do add 1 2").unwrap();

        let mut compiler = IncrementalCompiler::new(&dir);

        let r1 = compiler
            .compile_file(&knl_path, &crate::codegen::Target::Debug)
            .unwrap();
        assert!(!r1.from_cache);

        let r2 = compiler
            .compile_file(&knl_path, &crate::codegen::Target::Debug)
            .unwrap();
        assert!(r2.from_cache);

        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn test_incremental_compiler_invalidates_on_change() {
        let dir = std::env::temp_dir().join("kernl_test_incr_inv");
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();

        let knl_path = dir.join("test.knl");
        fs::write(&knl_path, "fn main\n  do add 1 2").unwrap();

        let mut compiler = IncrementalCompiler::new(&dir);

        let r1 = compiler
            .compile_file(&knl_path, &crate::codegen::Target::Debug)
            .unwrap();
        assert!(!r1.from_cache);

        fs::write(&knl_path, "fn main\n  do add 3 4").unwrap();

        let r2 = compiler
            .compile_file(&knl_path, &crate::codegen::Target::Debug)
            .unwrap();
        assert!(!r2.from_cache);

        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn test_clear_cache() {
        let dir = std::env::temp_dir().join("kernl_test_clear");
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();

        let knl_path = dir.join("test.knl");
        fs::write(&knl_path, "fn main\n  do add 1 2").unwrap();

        let mut compiler = IncrementalCompiler::new(&dir);
        compiler
            .compile_file(&knl_path, &crate::codegen::Target::Debug)
            .unwrap();
        assert_eq!(compiler.stats().total_entries, 1);

        compiler.clear_cache();
        assert_eq!(compiler.stats().total_entries, 0);

        let _ = fs::remove_dir_all(&dir);
    }
}
