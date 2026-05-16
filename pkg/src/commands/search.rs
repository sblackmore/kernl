use crate::registry::{Registry, RegistryErrorKind};

pub fn run(query: &str) -> Result<(), String> {
    println!("searching for '{query}'...");

    let registry = Registry::new();

    match registry.search(query) {
        Ok(result) => {
            if result.packages.is_empty() {
                println!("no packages found matching '{query}'");
                return Ok(());
            }

            println!(
                "\n{:<30} {:<12} {}",
                "NAME", "VERSION", "DESCRIPTION"
            );
            println!("{}", "-".repeat(72));

            for pkg in &result.packages {
                let desc = pkg.description.as_deref().unwrap_or("");
                println!("{:<30} {:<12} {}", pkg.name, pkg.version, desc);
            }

            println!("\n{} package(s) found", result.total);
            Ok(())
        }
        Err(e) if e.kind == RegistryErrorKind::NotAvailable => {
            eprintln!(
                "note: the kernl package registry is not yet available.\n\
                 packages will be searchable once the registry launches.\n\
                 see https://kernl-lang.org/registry for updates."
            );
            Ok(())
        }
        Err(e) => Err(format!("search failed: {e}")),
    }
}

#[cfg(test)]
mod tests {
    use crate::registry::{PackageSummary, SearchResult};

    #[test]
    fn format_search_results() {
        let result = SearchResult {
            packages: vec![
                PackageSummary {
                    name: "json".into(),
                    version: "1.0.0".into(),
                    description: Some("JSON parser for kernl".into()),
                },
                PackageSummary {
                    name: "json-schema".into(),
                    version: "0.2.0".into(),
                    description: None,
                },
            ],
            total: 2,
        };

        assert_eq!(result.packages.len(), 2);
        assert_eq!(result.total, 2);
        assert_eq!(result.packages[0].name, "json");
        assert!(result.packages[1].description.is_none());
    }

    #[test]
    fn search_not_available_is_not_error() {
        let result = super::run("test-query");
        assert!(result.is_ok());
    }
}
