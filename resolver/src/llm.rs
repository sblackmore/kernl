use serde_json::{Value, json};
use std::fmt;

#[derive(Debug)]
pub enum LlmError {
    Request(String),
    Parse(String),
    Api(String),
}

impl fmt::Display for LlmError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            LlmError::Request(msg) => write!(f, "LLM request error: {msg}"),
            LlmError::Parse(msg) => write!(f, "LLM parse error: {msg}"),
            LlmError::Api(msg) => write!(f, "LLM API error: {msg}"),
        }
    }
}

pub struct LlmClient {
    endpoint: String,
    model: String,
    api_key: String,
}

impl LlmClient {
    pub fn new(endpoint: String, model: String, api_key: String) -> Self {
        LlmClient {
            endpoint,
            model,
            api_key,
        }
    }

    pub fn build_request_body(&self, system: &str, user: &str) -> Value {
        json!({
            "model": self.model,
            "messages": [
                {"role": "system", "content": system},
                {"role": "user", "content": user}
            ],
            "temperature": 0
        })
    }

    pub fn complete(&self, system: &str, user: &str) -> Result<String, LlmError> {
        let url = format!("{}/chat/completions", self.endpoint.trim_end_matches('/'));
        let body = self.build_request_body(system, user);

        let response: Value = ureq::post(&url)
            .header("Authorization", &format!("Bearer {}", self.api_key))
            .header("Content-Type", "application/json")
            .send_json(&body)
            .map_err(|e| LlmError::Request(e.to_string()))?
            .body_mut()
            .read_json()
            .map_err(|e| LlmError::Parse(e.to_string()))?;

        let content = response["choices"]
            .get(0)
            .and_then(|c| c["message"]["content"].as_str())
            .ok_or_else(|| LlmError::Api("no content in LLM response".into()))?;

        Ok(content.to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_client_creation() {
        let client = LlmClient::new(
            "https://api.openai.com/v1".into(),
            "gpt-4o".into(),
            "sk-test".into(),
        );
        assert_eq!(client.endpoint, "https://api.openai.com/v1");
        assert_eq!(client.model, "gpt-4o");
        assert_eq!(client.api_key, "sk-test");
    }

    #[test]
    fn test_build_request_body() {
        let client = LlmClient::new(
            "https://api.openai.com/v1".into(),
            "gpt-4o".into(),
            "sk-test".into(),
        );

        let body = client.build_request_body("system prompt", "user prompt");

        assert_eq!(body["model"], "gpt-4o");
        assert_eq!(body["temperature"], 0);

        let messages = body["messages"].as_array().unwrap();
        assert_eq!(messages.len(), 2);
        assert_eq!(messages[0]["role"], "system");
        assert_eq!(messages[0]["content"], "system prompt");
        assert_eq!(messages[1]["role"], "user");
        assert_eq!(messages[1]["content"], "user prompt");
    }

    #[test]
    fn test_error_display() {
        let err = LlmError::Request("timeout".into());
        assert_eq!(format!("{err}"), "LLM request error: timeout");

        let err = LlmError::Parse("invalid json".into());
        assert_eq!(format!("{err}"), "LLM parse error: invalid json");

        let err = LlmError::Api("rate limited".into());
        assert_eq!(format!("{err}"), "LLM API error: rate limited");
    }
}
