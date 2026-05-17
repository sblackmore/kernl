use std::collections::HashMap;
use std::fs;
use std::path::Path;

use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct ApiToken {
    pub token: String,
    pub username: String,
    pub created_at: String,
    pub scopes: Vec<String>,
}

pub struct AuthManager {
    tokens: HashMap<String, ApiToken>,
    tokens_file: String,
}

impl AuthManager {
    pub fn new(data_dir: &Path) -> Self {
        let tokens_file = data_dir.join("tokens.json").to_string_lossy().to_string();
        let tokens = if let Ok(data) = fs::read_to_string(&tokens_file) {
            serde_json::from_str::<Vec<ApiToken>>(&data)
                .unwrap_or_default()
                .into_iter()
                .map(|t| (t.token.clone(), t))
                .collect()
        } else {
            HashMap::new()
        };
        Self {
            tokens,
            tokens_file,
        }
    }

    pub fn validate(&self, auth_header: &str) -> Option<&ApiToken> {
        let token = auth_header.strip_prefix("Bearer ")?;
        self.tokens.get(token)
    }

    pub fn has_scope(token: &ApiToken, scope: &str) -> bool {
        token.scopes.contains(&scope.to_string()) || token.scopes.contains(&"*".to_string())
    }

    pub fn register_token(&mut self, username: &str, scopes: Vec<String>) -> String {
        let token_str = format!("knl_{}", generate_token_id());
        let token = ApiToken {
            token: token_str.clone(),
            username: username.to_string(),
            created_at: timestamp_now(),
            scopes,
        };
        self.tokens.insert(token_str.clone(), token);
        self.save();
        token_str
    }

    pub fn revoke_token(&mut self, token: &str) -> bool {
        let removed = self.tokens.remove(token).is_some();
        if removed {
            self.save();
        }
        removed
    }

    fn save(&self) {
        let tokens: Vec<&ApiToken> = self.tokens.values().collect();
        if let Ok(json) = serde_json::to_string_pretty(&tokens) {
            let _ = fs::write(&self.tokens_file, json);
        }
    }
}

fn generate_token_id() -> String {
    use std::time::{SystemTime, UNIX_EPOCH};
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    format!("{now:x}")
}

fn timestamp_now() -> String {
    use std::time::{SystemTime, UNIX_EPOCH};
    let secs = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_secs();
    format!("{secs}")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_auth() -> (AuthManager, tempfile::TempDir) {
        let dir = tempfile::tempdir().unwrap();
        let auth = AuthManager::new(dir.path());
        (auth, dir)
    }

    #[test]
    fn register_and_validate_token() {
        let (mut auth, _dir) = temp_auth();
        let token = auth.register_token("alice", vec!["publish".into(), "search".into()]);
        assert!(token.starts_with("knl_"));

        let header = format!("Bearer {token}");
        let found = auth.validate(&header).unwrap();
        assert_eq!(found.username, "alice");
    }

    #[test]
    fn validate_invalid_token_returns_none() {
        let (auth, _dir) = temp_auth();
        assert!(auth.validate("Bearer bad_token").is_none());
    }

    #[test]
    fn validate_missing_bearer_prefix() {
        let (mut auth, _dir) = temp_auth();
        let token = auth.register_token("bob", vec!["*".into()]);
        assert!(auth.validate(&token).is_none());
    }

    #[test]
    fn has_scope_checks_correctly() {
        let token = ApiToken {
            token: "knl_test".into(),
            username: "test".into(),
            created_at: "0".into(),
            scopes: vec!["publish".into(), "download".into()],
        };
        assert!(AuthManager::has_scope(&token, "publish"));
        assert!(AuthManager::has_scope(&token, "download"));
        assert!(!AuthManager::has_scope(&token, "admin"));
    }

    #[test]
    fn wildcard_scope_matches_everything() {
        let token = ApiToken {
            token: "knl_test".into(),
            username: "admin".into(),
            created_at: "0".into(),
            scopes: vec!["*".into()],
        };
        assert!(AuthManager::has_scope(&token, "publish"));
        assert!(AuthManager::has_scope(&token, "admin"));
        assert!(AuthManager::has_scope(&token, "anything"));
    }

    #[test]
    fn revoke_token_removes_it() {
        let (mut auth, _dir) = temp_auth();
        let token = auth.register_token("carol", vec!["publish".into()]);
        assert!(auth.validate(&format!("Bearer {token}")).is_some());

        assert!(auth.revoke_token(&token));
        assert!(auth.validate(&format!("Bearer {token}")).is_none());
    }

    #[test]
    fn revoke_nonexistent_token_returns_false() {
        let (mut auth, _dir) = temp_auth();
        assert!(!auth.revoke_token("knl_nonexistent"));
    }

    #[test]
    fn persistence_across_instances() {
        let dir = tempfile::tempdir().unwrap();
        let token = {
            let mut auth = AuthManager::new(dir.path());
            auth.register_token("dave", vec!["publish".into()])
        };

        let auth2 = AuthManager::new(dir.path());
        let found = auth2.validate(&format!("Bearer {token}")).unwrap();
        assert_eq!(found.username, "dave");
    }
}
