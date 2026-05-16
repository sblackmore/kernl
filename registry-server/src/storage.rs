use std::collections::HashMap;
use std::fmt;
use std::fs;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct PackageMeta {
    pub name: String,
    pub version: String,
    pub description: Option<String>,
    pub authors: Vec<String>,
    pub license: Option<String>,
    pub dependencies: HashMap<String, String>,
    pub published_at: String,
    pub checksum: String,
}

#[derive(Debug)]
pub struct StorageError {
    pub message: String,
}

impl fmt::Display for StorageError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.message)
    }
}

impl std::error::Error for StorageError {}

impl From<std::io::Error> for StorageError {
    fn from(e: std::io::Error) -> Self {
        Self {
            message: e.to_string(),
        }
    }
}

impl From<serde_json::Error> for StorageError {
    fn from(e: serde_json::Error) -> Self {
        Self {
            message: e.to_string(),
        }
    }
}

pub struct Storage {
    data_dir: PathBuf,
}

impl Storage {
    pub fn new(data_dir: &Path) -> Self {
        fs::create_dir_all(data_dir.join("packages")).ok();
        fs::create_dir_all(data_dir.join("tarballs")).ok();
        Self {
            data_dir: data_dir.to_path_buf(),
        }
    }

    pub fn publish(&self, meta: &PackageMeta, tarball: &[u8]) -> Result<(), StorageError> {
        let pkg_dir = self.data_dir.join("packages").join(&meta.name);
        fs::create_dir_all(&pkg_dir)?;

        let meta_path = pkg_dir.join(format!("{}.json", meta.version));
        fs::write(&meta_path, serde_json::to_string_pretty(meta)?)?;

        let tarball_path = self
            .data_dir
            .join("tarballs")
            .join(format!("{}-{}.tar.gz", meta.name, meta.version));
        fs::write(&tarball_path, tarball)?;

        self.update_versions_index(&meta.name)?;

        Ok(())
    }

    pub fn get_package(&self, name: &str, version: &str) -> Result<PackageMeta, StorageError> {
        let meta_path = self
            .data_dir
            .join("packages")
            .join(name)
            .join(format!("{version}.json"));
        let data = fs::read_to_string(&meta_path)?;
        Ok(serde_json::from_str(&data)?)
    }

    pub fn get_latest(&self, name: &str) -> Result<PackageMeta, StorageError> {
        let pkg_dir = self.data_dir.join("packages").join(name);
        let mut versions = self.collect_versions(&pkg_dir)?;
        versions.sort();
        let latest = versions
            .last()
            .ok_or_else(|| StorageError {
                message: "no versions found".into(),
            })?
            .clone();
        self.get_package(name, &latest)
    }

    pub fn search(&self, query: &str) -> Result<Vec<PackageMeta>, StorageError> {
        let pkg_dir = self.data_dir.join("packages");
        let mut results = Vec::new();
        if let Ok(entries) = fs::read_dir(&pkg_dir) {
            for entry in entries.flatten() {
                let name = entry.file_name().to_string_lossy().to_string();
                if name.contains(query) {
                    if let Ok(meta) = self.get_latest(&name) {
                        results.push(meta);
                    }
                }
            }
        }
        Ok(results)
    }

    pub fn get_tarball(&self, name: &str, version: &str) -> Result<Vec<u8>, StorageError> {
        let path = self
            .data_dir
            .join("tarballs")
            .join(format!("{name}-{version}.tar.gz"));
        Ok(fs::read(&path)?)
    }

    fn update_versions_index(&self, name: &str) -> Result<(), StorageError> {
        let pkg_dir = self.data_dir.join("packages").join(name);
        let mut versions = self.collect_versions(&pkg_dir)?;
        versions.sort();
        let index_path = pkg_dir.join("versions.json");
        fs::write(&index_path, serde_json::to_string(&versions)?)?;
        Ok(())
    }

    fn collect_versions(&self, pkg_dir: &Path) -> Result<Vec<String>, StorageError> {
        let mut versions = Vec::new();
        for entry in fs::read_dir(pkg_dir)? {
            if let Ok(e) = entry {
                if let Some(stem) = e.path().file_stem() {
                    if stem != "versions" {
                        versions.push(stem.to_string_lossy().to_string());
                    }
                }
            }
        }
        Ok(versions)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    fn temp_storage() -> (Storage, tempfile::TempDir) {
        let dir = tempfile::tempdir().unwrap();
        let storage = Storage::new(dir.path());
        (storage, dir)
    }

    fn sample_meta(name: &str, version: &str) -> PackageMeta {
        PackageMeta {
            name: name.to_string(),
            version: version.to_string(),
            description: Some("A test package".to_string()),
            authors: vec!["test@example.com".to_string()],
            license: Some("MIT".to_string()),
            dependencies: HashMap::new(),
            published_at: "2026-01-01T00:00:00Z".to_string(),
            checksum: "abc123".to_string(),
        }
    }

    #[test]
    fn test_publish_and_retrieve() {
        let (storage, _dir) = temp_storage();
        let meta = sample_meta("my-pkg", "0.1.0");
        let tarball = b"fake tarball data";

        storage.publish(&meta, tarball).unwrap();

        let retrieved = storage.get_package("my-pkg", "0.1.0").unwrap();
        assert_eq!(retrieved.name, "my-pkg");
        assert_eq!(retrieved.version, "0.1.0");
        assert_eq!(retrieved.description, Some("A test package".to_string()));
    }

    #[test]
    fn test_get_latest() {
        let (storage, _dir) = temp_storage();

        storage
            .publish(&sample_meta("my-pkg", "0.1.0"), b"v1")
            .unwrap();
        storage
            .publish(&sample_meta("my-pkg", "0.2.0"), b"v2")
            .unwrap();

        let latest = storage.get_latest("my-pkg").unwrap();
        assert_eq!(latest.version, "0.2.0");
    }

    #[test]
    fn test_get_tarball() {
        let (storage, _dir) = temp_storage();
        let tarball = b"real tarball bytes";
        storage
            .publish(&sample_meta("my-pkg", "1.0.0"), tarball)
            .unwrap();

        let data = storage.get_tarball("my-pkg", "1.0.0").unwrap();
        assert_eq!(data, tarball);
    }

    #[test]
    fn test_search() {
        let (storage, _dir) = temp_storage();

        storage
            .publish(&sample_meta("math-utils", "0.1.0"), b"a")
            .unwrap();
        storage
            .publish(&sample_meta("string-utils", "0.1.0"), b"b")
            .unwrap();
        storage
            .publish(&sample_meta("http-client", "0.1.0"), b"c")
            .unwrap();

        let results = storage.search("utils").unwrap();
        assert_eq!(results.len(), 2);

        let names: Vec<&str> = results.iter().map(|r| r.name.as_str()).collect();
        assert!(names.contains(&"math-utils"));
        assert!(names.contains(&"string-utils"));
    }

    #[test]
    fn test_get_nonexistent_package() {
        let (storage, _dir) = temp_storage();
        let result = storage.get_package("no-such-pkg", "1.0.0");
        assert!(result.is_err());
    }
}
