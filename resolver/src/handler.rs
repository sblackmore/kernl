use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use tiny_http::{Request, Response, StatusCode};

use crate::llm::{LlmClient, LlmError};

#[derive(Debug, Deserialize)]
pub struct ResolveRequest {
    pub intent: String,
    pub params: Vec<(String, String)>,
    #[serde(default = "default_confidence_threshold")]
    pub confidence_threshold: f64,
}

fn default_confidence_threshold() -> f64 {
    0.8
}

#[derive(Debug, Serialize)]
pub struct ResolveResponse {
    pub result: Value,
    pub confidence: f64,
    pub used_fallback: bool,
}

#[derive(Debug, Serialize)]
pub struct ErrorResponse {
    pub error: String,
}

const SYSTEM_PROMPT: &str = "You are a kernl fluid-mode resolver. Given an intent and parameters, \
    produce a result. Respond with JSON: {\"result\": <value>, \"confidence\": <0.0-1.0>}";

pub fn handle_request(request: Request, client: &LlmClient) {
    let method = request.method().to_string();
    let url = request.url().to_string();

    match (method.as_str(), url.as_str()) {
        ("POST", "/resolve") => handle_resolve(request, client),
        ("GET", "/health") => handle_health(request),
        ("GET", "/info") => handle_info(request, client),
        _ => send_error(request, 404, "not found"),
    }
}

fn handle_resolve(mut request: Request, client: &LlmClient) {
    let mut body = String::new();
    if let Err(e) = request.as_reader().read_to_string(&mut body) {
        send_error(request, 400, &format!("failed to read body: {e}"));
        return;
    }

    let resolve_req: ResolveRequest = match serde_json::from_str(&body) {
        Ok(r) => r,
        Err(e) => {
            send_error(request, 400, &format!("invalid request JSON: {e}"));
            return;
        }
    };

    let params_formatted = resolve_req
        .params
        .iter()
        .map(|(k, v)| format!("  {k}: {v}"))
        .collect::<Vec<_>>()
        .join("\n");

    let user_message = format!(
        "Intent: {}\nParameters:\n{}\nReturn type context: value",
        resolve_req.intent, params_formatted
    );

    let llm_response = match client.complete(SYSTEM_PROMPT, &user_message) {
        Ok(r) => r,
        Err(e) => {
            let status = match &e {
                LlmError::Request(_) => 502,
                LlmError::Parse(_) => 502,
                LlmError::Api(_) => 502,
            };
            send_error(request, status, &e.to_string());
            return;
        }
    };

    let parsed: Value = match serde_json::from_str(&llm_response) {
        Ok(v) => v,
        Err(_) => {
            json!({
                "result": llm_response,
                "confidence": 0.5
            })
        }
    };

    let result = parsed.get("result").cloned().unwrap_or(Value::String(llm_response.clone()));
    let confidence = parsed
        .get("confidence")
        .and_then(|c| c.as_f64())
        .unwrap_or(0.5);

    let used_fallback = confidence < resolve_req.confidence_threshold;

    let response = ResolveResponse {
        result,
        confidence,
        used_fallback,
    };

    send_json(request, 200, &response);
}

fn handle_health(request: Request) {
    send_json(request, 200, &json!({"status": "ok"}));
}

fn handle_info(request: Request, _client: &LlmClient) {
    send_json(
        request,
        200,
        &json!({
            "version": env!("CARGO_PKG_VERSION"),
            "model": "configured via --llm-model"
        }),
    );
}

fn send_json<T: Serialize>(request: Request, status: u16, body: &T) {
    let json = serde_json::to_string(body).unwrap_or_else(|_| r#"{"error":"serialize failure"}"#.into());
    let response = Response::from_string(json)
        .with_status_code(StatusCode(status))
        .with_header(
            tiny_http::Header::from_bytes(&b"Content-Type"[..], &b"application/json"[..]).unwrap(),
        );
    let _ = request.respond(response);
}

fn send_error(request: Request, status: u16, message: &str) {
    let body = ErrorResponse {
        error: message.to_string(),
    };
    send_json(request, status, &body);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_request_parsing() {
        let json = r#"{
            "intent": "surface items user would engage with",
            "params": [["user", "User{id: 42}"], ["context", "Context{...}"]],
            "confidence_threshold": 0.85
        }"#;

        let req: ResolveRequest = serde_json::from_str(json).unwrap();
        assert_eq!(req.intent, "surface items user would engage with");
        assert_eq!(req.params.len(), 2);
        assert_eq!(req.params[0].0, "user");
        assert_eq!(req.params[0].1, "User{id: 42}");
        assert_eq!(req.params[1].0, "context");
        assert_eq!(req.params[1].1, "Context{...}");
        assert!((req.confidence_threshold - 0.85).abs() < f64::EPSILON);
    }

    #[test]
    fn test_request_parsing_default_threshold() {
        let json = r#"{
            "intent": "sort items",
            "params": [["list", "[3,1,2]"]]
        }"#;

        let req: ResolveRequest = serde_json::from_str(json).unwrap();
        assert!((req.confidence_threshold - 0.8).abs() < f64::EPSILON);
    }

    #[test]
    fn test_response_serialization() {
        let resp = ResolveResponse {
            result: serde_json::json!("sorted list"),
            confidence: 0.92,
            used_fallback: false,
        };

        let json = serde_json::to_string(&resp).unwrap();
        let parsed: Value = serde_json::from_str(&json).unwrap();

        assert_eq!(parsed["result"], "sorted list");
        assert_eq!(parsed["confidence"], 0.92);
        assert_eq!(parsed["used_fallback"], false);
    }

    #[test]
    fn test_error_response_serialization() {
        let resp = ErrorResponse {
            error: "something went wrong".into(),
        };

        let json = serde_json::to_string(&resp).unwrap();
        let parsed: Value = serde_json::from_str(&json).unwrap();

        assert_eq!(parsed["error"], "something went wrong");
    }
}
