use serde::{Deserialize, Serialize};

const DEFAULT_REGISTRY: &str = "https://registry.kernl-lang.org/api/v1";

#[derive(Debug, Serialize, Deserialize)]
pub struct PackageInfo {
    pub name: String,
    pub version: String,
    pub description: Option<String>,
    pub authors: Vec<String>,
    pub license: Option<String>,
    pub dependencies: std::collections::HashMap<String, String>,
    pub tarball_url: String,
    pub checksum: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct SearchResult {
    pub packages: Vec<PackageSummary>,
    pub total: usize,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct PackageSummary {
    pub name: String,
    pub version: String,
    pub description: Option<String>,
}

#[derive(Debug)]
pub struct RegistryError {
    pub message: String,
    pub kind: RegistryErrorKind,
}

#[derive(Debug, PartialEq)]
pub enum RegistryErrorKind {
    NotAvailable,
    NotFound,
    Unauthorized,
    Network,
}

impl std::fmt::Display for RegistryError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.message)
    }
}

pub struct Registry {
    base_url: String,
}

impl Registry {
    pub fn new() -> Self {
        Self {
            base_url: DEFAULT_REGISTRY.into(),
        }
    }

    /// Base URL from `KERNL_REGISTRY_URL` or [`DEFAULT_REGISTRY`].
    pub fn from_environment() -> Self {
        let base_url =
            std::env::var("KERNL_REGISTRY_URL").unwrap_or_else(|_| DEFAULT_REGISTRY.into());
        Self { base_url }
    }

    pub fn with_url(url: String) -> Self {
        Self { base_url: url }
    }

    pub fn base_url(&self) -> &str {
        &self.base_url
    }

    /// Fetch package info: GET /packages/{name}/{version}
    pub fn get_package(&self, name: &str, version: &str) -> Result<PackageInfo, RegistryError> {
        Err(RegistryError {
            message: format!(
                "registry not available — package '{name}@{version}' would be fetched from {}/packages/{name}/{version}",
                self.base_url
            ),
            kind: RegistryErrorKind::NotAvailable,
        })
    }

    /// Search packages: GET /search?q={query}
    pub fn search(&self, query: &str) -> Result<SearchResult, RegistryError> {
        Err(RegistryError {
            message: format!(
                "registry not available — would search for '{query}' at {}/search",
                self.base_url
            ),
            kind: RegistryErrorKind::NotAvailable,
        })
    }

    /// Publish a package: POST /packages (requires auth)
    pub fn publish(
        &self,
        _tarball: &[u8],
        _manifest: &crate::manifest::Manifest,
    ) -> Result<(), RegistryError> {
        Err(RegistryError {
            message: "registry not available for publishing".into(),
            kind: RegistryErrorKind::NotAvailable,
        })
    }

    /// Download a package tarball: GET /packages/{name}/{version}/download
    pub fn download(&self, name: &str, version: &str) -> Result<Vec<u8>, RegistryError> {
        Err(RegistryError {
            message: format!(
                "registry not available — would download '{name}@{version}' from {}/packages/{name}/{version}/download",
                self.base_url
            ),
            kind: RegistryErrorKind::NotAvailable,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn new_uses_default_url() {
        let reg = Registry::new();
        assert_eq!(reg.base_url(), DEFAULT_REGISTRY);
    }

    #[test]
    fn with_url_uses_custom_url() {
        let reg = Registry::with_url("https://custom.example.com/api".into());
        assert_eq!(reg.base_url(), "https://custom.example.com/api");
    }

    #[test]
    fn get_package_returns_not_available() {
        let reg = Registry::new();
        let err = reg.get_package("math", "0.1.0").unwrap_err();
        assert_eq!(err.kind, RegistryErrorKind::NotAvailable);
        assert!(err.message.contains("math@0.1.0"));
    }

    #[test]
    fn search_returns_not_available() {
        let reg = Registry::new();
        let err = reg.search("json").unwrap_err();
        assert_eq!(err.kind, RegistryErrorKind::NotAvailable);
        assert!(err.message.contains("json"));
    }

    #[test]
    fn download_returns_not_available() {
        let reg = Registry::new();
        let err = reg.download("utils", "1.0.0").unwrap_err();
        assert_eq!(err.kind, RegistryErrorKind::NotAvailable);
    }
}
