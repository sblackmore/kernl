use std::collections::HashMap;

pub struct DocumentState {
    documents: HashMap<String, String>,
}

impl DocumentState {
    pub fn new() -> Self {
        Self {
            documents: HashMap::new(),
        }
    }

    pub fn set(&mut self, uri: &str, text: String) {
        self.documents.insert(uri.to_string(), text);
    }

    pub fn get(&self, uri: &str) -> Option<&str> {
        self.documents.get(uri).map(|s| s.as_str())
    }

    pub fn remove(&mut self, uri: &str) {
        self.documents.remove(uri);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn open_and_get() {
        let mut state = DocumentState::new();
        state.set("file:///test.knl", "fn main\n  do 1".into());
        assert_eq!(state.get("file:///test.knl"), Some("fn main\n  do 1"));
    }

    #[test]
    fn change_overwrites() {
        let mut state = DocumentState::new();
        state.set("file:///a.knl", "v1".into());
        state.set("file:///a.knl", "v2".into());
        assert_eq!(state.get("file:///a.knl"), Some("v2"));
    }

    #[test]
    fn remove_document() {
        let mut state = DocumentState::new();
        state.set("file:///a.knl", "content".into());
        state.remove("file:///a.knl");
        assert_eq!(state.get("file:///a.knl"), None);
    }

    #[test]
    fn get_missing_returns_none() {
        let state = DocumentState::new();
        assert_eq!(state.get("file:///nonexistent.knl"), None);
    }
}
