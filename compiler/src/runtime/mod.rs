pub mod executor;

use crate::parser::ast::*;
use std::fmt;

// ---------------------------------------------------------------------------
// Error
// ---------------------------------------------------------------------------

#[derive(Debug)]
pub struct RuntimeError {
    pub message: String,
}

impl fmt::Display for RuntimeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "runtime error: {}", self.message)
    }
}

impl std::error::Error for RuntimeError {}

// ---------------------------------------------------------------------------
// Resolver protocol
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
pub struct ResolverConfig {
    pub endpoint: String,
    pub model: String,
    pub timeout_ms: u64,
    pub max_retries: u32,
}

#[derive(Debug, Clone)]
pub struct ResolverRequest {
    pub intent: String,
    pub params: Vec<(String, String)>,
    pub confidence_threshold: f64,
}

#[derive(Debug, Clone)]
pub struct ResolverResponse {
    pub result: String,
    pub confidence: f64,
    pub used_fallback: bool,
}

// ---------------------------------------------------------------------------
// Resolver mode
// ---------------------------------------------------------------------------

pub enum ResolverMode {
    Stub,
    Http(HttpResolver),
}

// ---------------------------------------------------------------------------
// FluidRuntime — stub resolver
// ---------------------------------------------------------------------------

pub struct FluidRuntime {
    pub config: ResolverConfig,
}

impl FluidRuntime {
    pub fn new(config: ResolverConfig) -> Self {
        Self { config }
    }

    /// Stub implementation: high-confidence requests resolve directly,
    /// low-confidence requests fall back.
    pub fn resolve(&self, request: &ResolverRequest) -> Result<ResolverResponse, RuntimeError> {
        if request.confidence_threshold > 0.5 {
            Ok(ResolverResponse {
                result: format!("stub_result({})", request.intent),
                confidence: 0.9,
                used_fallback: false,
            })
        } else {
            Ok(ResolverResponse {
                result: format!("fallback_result({})", request.intent),
                confidence: 0.4,
                used_fallback: true,
            })
        }
    }
}

// ---------------------------------------------------------------------------
// HttpResolver — live LLM resolver via OpenAI-compatible API
// ---------------------------------------------------------------------------

#[derive(serde::Serialize)]
struct ChatRequest {
    model: String,
    messages: Vec<ChatMessage>,
    temperature: f64,
}

#[derive(serde::Serialize)]
struct ChatMessage {
    role: String,
    content: String,
}

#[derive(serde::Deserialize)]
struct ChatResponse {
    choices: Vec<ChatChoice>,
}

#[derive(serde::Deserialize)]
struct ChatChoice {
    message: ChatMessageResponse,
}

#[derive(serde::Deserialize)]
struct ChatMessageResponse {
    content: String,
}

#[derive(serde::Deserialize)]
struct ResolvedValue {
    result: String,
    confidence: f64,
}

pub struct HttpResolver {
    config: ResolverConfig,
}

impl HttpResolver {
    pub fn new(config: ResolverConfig) -> Self {
        Self { config }
    }

    pub fn resolve(&self, request: &ResolverRequest) -> Result<ResolverResponse, RuntimeError> {
        let chat_req = ChatRequest {
            model: self.config.model.clone(),
            messages: vec![
                ChatMessage {
                    role: "system".into(),
                    content: "You are a kernl fluid-mode resolver. Given an intent and parameters, \
                              return a JSON object with 'result' (string) and 'confidence' (float 0-1) \
                              fields. Return ONLY valid JSON.".into(),
                },
                ChatMessage {
                    role: "user".into(),
                    content: format!(
                        "Intent: {}\nParams: {:?}\nReturn a JSON object with 'result' and 'confidence' fields.",
                        request.intent, request.params
                    ),
                },
            ],
            temperature: 0.0,
        };

        let mut response = ureq::post(&self.config.endpoint)
            .header("Content-Type", "application/json")
            .send_json(&chat_req)
            .map_err(|e| RuntimeError {
                message: format!("HTTP request failed: {e}"),
            })?;

        let response_body = response.body_mut().read_to_string().map_err(|e| RuntimeError {
            message: format!("failed to read response body: {e}"),
        })?;

        let chat_resp: ChatResponse =
            serde_json::from_str(&response_body).map_err(|e| RuntimeError {
                message: format!("failed to parse chat response: {e}"),
            })?;

        let content = chat_resp
            .choices
            .first()
            .map(|c| c.message.content.clone())
            .ok_or_else(|| RuntimeError {
                message: "no choices in response".into(),
            })?;

        let resolved: ResolvedValue =
            serde_json::from_str(&content).map_err(|e| RuntimeError {
                message: format!("failed to parse resolved value: {e}"),
            })?;

        let used_fallback = resolved.confidence < request.confidence_threshold;

        Ok(ResolverResponse {
            result: resolved.result,
            confidence: resolved.confidence,
            used_fallback,
        })
    }
}

// ---------------------------------------------------------------------------
// Codegen helpers for fluid-mode functions
// ---------------------------------------------------------------------------

pub struct RuntimeCodegen;

impl RuntimeCodegen {
    /// Emit LLVM IR that calls the runtime resolver for a fluid function.
    pub fn emit_fluid_llvm(func: &Function) -> String {
        let intent = func.intent.as_deref().unwrap_or(&func.name);
        let threshold = func.confidence.unwrap_or(0.8);

        let params_ir: Vec<String> = func
            .params
            .iter()
            .map(|p| format!("i64 %{}", p.name))
            .collect();

        let mut ir = String::new();

        ir.push_str("declare i64 @__kernl_resolve(i8*, double)\n\n");

        ir.push_str(&format!(
            "define i64 @{}({}) {{\n",
            func.name,
            params_ir.join(", ")
        ));
        ir.push_str("entry:\n");

        ir.push_str(&format!(
            "  %intent = alloca [{}  x i8]\n",
            intent.len() + 1
        ));
        ir.push_str(&format!(
            "  %threshold = fptrunc double {threshold:.6e} to double\n"
        ));
        ir.push_str("  %resolved = call i64 @__kernl_resolve(i8* %intent, double %threshold)\n");

        ir.push_str(&format!(
            "  %conf = fcmp oge double %threshold, {threshold:.6e}\n"
        ));
        ir.push_str("  br i1 %conf, label %use_resolved, label %use_fallback\n\n");

        ir.push_str("use_resolved:\n");
        ir.push_str("  ret i64 %resolved\n\n");

        ir.push_str("use_fallback:\n");
        if func.fallback.is_some() {
            ir.push_str("  %fb = call i64 @__kernl_fallback()\n");
            ir.push_str("  ret i64 %fb\n");
        } else {
            ir.push_str("  ret i64 0\n");
        }

        ir.push_str("}\n");
        ir
    }

    /// Emit WAT (WebAssembly text) that calls the runtime resolver for a fluid function.
    pub fn emit_fluid_wasm(func: &Function) -> String {
        let intent = func.intent.as_deref().unwrap_or(&func.name);
        let threshold = func.confidence.unwrap_or(0.8);

        let params_wat: Vec<String> = func
            .params
            .iter()
            .map(|p| format!("(param ${} i64)", p.name))
            .collect();

        let mut wat = String::new();

        wat.push_str("(module\n");
        wat.push_str(
            "  (import \"kernl\" \"__kernl_resolve\" (func $__kernl_resolve (param i32 f64) (result i64)))\n",
        );
        wat.push_str(&format!(
            "  (func ${} {} (result i64)\n",
            func.name,
            params_wat.join(" ")
        ));

        wat.push_str(&format!(
            "    ;; intent: \"{intent}\"\n"
        ));
        wat.push_str(&format!("    i32.const 0 ;; intent string ptr\n"));
        wat.push_str(&format!("    f64.const {threshold}\n"));
        wat.push_str("    call $__kernl_resolve\n");

        wat.push_str("    ;; branch: if confidence >= threshold use result, else fallback\n");
        wat.push_str(&format!("    f64.const {threshold}\n"));
        wat.push_str("    f64.ge\n");
        wat.push_str("    (if (result i64)\n");
        wat.push_str("      (then\n");
        wat.push_str("        i64.const 1 ;; resolved value placeholder\n");
        wat.push_str("      )\n");
        wat.push_str("      (else\n");

        if func.fallback.is_some() {
            wat.push_str("        i64.const -1 ;; fallback value\n");
        } else {
            wat.push_str("        i64.const 0 ;; no fallback\n");
        }

        wat.push_str("      )\n");
        wat.push_str("    )\n");
        wat.push_str("  )\n");
        wat.push_str(&format!(
            "  (export \"{}\" (func ${}))\n",
            func.name, func.name
        ));
        wat.push_str(")\n");

        wat
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    fn stub_config() -> ResolverConfig {
        ResolverConfig {
            endpoint: "http://localhost:8080/v1/resolve".into(),
            model: "gpt-4".into(),
            timeout_ms: 5000,
            max_retries: 3,
        }
    }

    fn fluid_function() -> Function {
        Function {
            name: "classify".into(),
            params: vec![Param {
                name: "text".into(),
                ty: Type::Named("str".into()),
            }],
            returns: Some(Param {
                name: "label".into(),
                ty: Type::Named("str".into()),
            }),
            invariants: vec![],
            requires: vec![],
            ensures: vec![],
            mode: FnMode::Fluid,
            intent: Some("classify the input text into a category".into()),
            confidence: Some(0.85),
            fallback: Some(Expr::StrLit("unknown".into())),
            guarantee: None,
            body: Expr::StrLit("placeholder".into()),
        }
    }

    #[test]
    fn config_creation() {
        let cfg = stub_config();
        assert_eq!(cfg.endpoint, "http://localhost:8080/v1/resolve");
        assert_eq!(cfg.model, "gpt-4");
        assert_eq!(cfg.timeout_ms, 5000);
        assert_eq!(cfg.max_retries, 3);
    }

    #[test]
    fn stub_resolve_high_confidence() {
        let rt = FluidRuntime::new(stub_config());
        let req = ResolverRequest {
            intent: "summarize".into(),
            params: vec![("text".into(), "hello world".into())],
            confidence_threshold: 0.8,
        };
        let resp = rt.resolve(&req).unwrap();
        assert_eq!(resp.confidence, 0.9);
        assert!(!resp.used_fallback);
        assert!(resp.result.contains("stub_result"));
    }

    #[test]
    fn stub_resolve_low_confidence_triggers_fallback() {
        let rt = FluidRuntime::new(stub_config());
        let req = ResolverRequest {
            intent: "summarize".into(),
            params: vec![],
            confidence_threshold: 0.3,
        };
        let resp = rt.resolve(&req).unwrap();
        assert!(resp.used_fallback);
        assert!(resp.result.contains("fallback_result"));
    }

    #[test]
    fn emit_fluid_llvm_structure() {
        let ir = RuntimeCodegen::emit_fluid_llvm(&fluid_function());
        assert!(ir.contains("declare i64 @__kernl_resolve(i8*, double)"));
        assert!(ir.contains("define i64 @classify"));
        assert!(ir.contains("call i64 @__kernl_resolve"));
        assert!(ir.contains("use_resolved:"));
        assert!(ir.contains("use_fallback:"));
        assert!(ir.contains("br i1 %conf"));
    }

    #[test]
    fn emit_fluid_wasm_structure() {
        let wat = RuntimeCodegen::emit_fluid_wasm(&fluid_function());
        assert!(wat.contains("(import \"kernl\" \"__kernl_resolve\""));
        assert!(wat.contains("(func $classify"));
        assert!(wat.contains("call $__kernl_resolve"));
        assert!(wat.contains("f64.ge"));
        assert!(wat.contains("(if (result i64)"));
        assert!(wat.contains("(export \"classify\""));
    }

    #[test]
    fn http_resolver_build_request_does_not_panic() {
        let config = ResolverConfig {
            endpoint: "http://127.0.0.1:1/v1/chat/completions".into(),
            model: "gpt-4".into(),
            timeout_ms: 1000,
            max_retries: 1,
        };
        let resolver = HttpResolver::new(config);
        let req = ResolverRequest {
            intent: "test_intent".into(),
            params: vec![("key".into(), "value".into())],
            confidence_threshold: 0.8,
        };
        let _ = &resolver;
        let _ = &req;
    }

    #[test]
    fn http_resolver_failed_connection_returns_error() {
        let config = ResolverConfig {
            endpoint: "http://127.0.0.1:1/v1/chat/completions".into(),
            model: "gpt-4".into(),
            timeout_ms: 100,
            max_retries: 0,
        };
        let resolver = HttpResolver::new(config);
        let req = ResolverRequest {
            intent: "test".into(),
            params: vec![],
            confidence_threshold: 0.9,
        };
        let result = resolver.resolve(&req);
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(err.message.contains("HTTP request failed"));
    }

    #[test]
    fn resolver_mode_enum_variants() {
        let stub_mode = ResolverMode::Stub;
        let http_mode = ResolverMode::Http(HttpResolver::new(stub_config()));
        match stub_mode {
            ResolverMode::Stub => {}
            ResolverMode::Http(_) => panic!("expected Stub"),
        }
        match http_mode {
            ResolverMode::Http(_) => {}
            ResolverMode::Stub => panic!("expected Http"),
        }
    }
}
