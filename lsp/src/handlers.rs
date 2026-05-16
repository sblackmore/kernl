use std::io::Write;

use kernlc::lexer::Lexer;
use kernlc::parser::Parser;
use kernlc::semantic::SemanticAnalyzer;
use kernlc::stdlib;
use kernlc::typeck::TypeChecker;
use serde_json::{json, Value};

use crate::state::DocumentState;
use crate::transport::write_message;

pub fn handle_request(
    method: &str,
    params: &Value,
    state: &mut DocumentState,
    writer: &mut impl Write,
) -> Option<Value> {
    match method {
        "initialize" => Some(handle_initialize()),
        "shutdown" => Some(Value::Null),
        "textDocument/hover" => Some(handle_hover(params, state)),
        "textDocument/completion" => Some(handle_completion()),
        "initialized" => None,
        "exit" => std::process::exit(0),
        "textDocument/didOpen" => {
            handle_did_open(params, state, writer);
            None
        }
        "textDocument/didChange" => {
            handle_did_change(params, state, writer);
            None
        }
        "textDocument/didClose" => {
            handle_did_close(params, state);
            None
        }
        _ => None,
    }
}

fn handle_initialize() -> Value {
    json!({
        "capabilities": {
            "textDocumentSync": 1,
            "hoverProvider": true,
            "completionProvider": {
                "triggerCharacters": [".", ":"]
            },
            "diagnosticProvider": {
                "interFileDependencies": false,
                "workspaceDiagnostics": false
            }
        },
        "serverInfo": {
            "name": "kernl-lsp",
            "version": "0.1.0"
        }
    })
}

fn handle_did_open(params: &Value, state: &mut DocumentState, writer: &mut impl Write) {
    if let Some(doc) = params.get("textDocument") {
        let uri = doc["uri"].as_str().unwrap_or_default();
        let text = doc["text"].as_str().unwrap_or_default();
        state.set(uri, text.to_string());
        publish_diagnostics(uri, text, writer);
    }
}

fn handle_did_change(params: &Value, state: &mut DocumentState, writer: &mut impl Write) {
    if let Some(doc) = params.get("textDocument") {
        let uri = doc["uri"].as_str().unwrap_or_default();
        if let Some(changes) = params.get("contentChanges").and_then(|c| c.as_array()) {
            if let Some(last) = changes.last() {
                let text = last["text"].as_str().unwrap_or_default();
                state.set(uri, text.to_string());
                publish_diagnostics(uri, text, writer);
            }
        }
    }
}

fn handle_did_close(params: &Value, state: &mut DocumentState) {
    if let Some(doc) = params.get("textDocument") {
        let uri = doc["uri"].as_str().unwrap_or_default();
        state.remove(uri);
    }
}

fn publish_diagnostics(uri: &str, source: &str, writer: &mut impl Write) {
    let diagnostics = compute_diagnostics(source);
    let notification = json!({
        "jsonrpc": "2.0",
        "method": "textDocument/publishDiagnostics",
        "params": {
            "uri": uri,
            "diagnostics": diagnostics
        }
    });
    let _ = write_message(writer, &notification);
}

fn compute_diagnostics(source: &str) -> Vec<Value> {
    let mut diagnostics = Vec::new();

    let tokens = match Lexer::new(source).tokenize() {
        Ok(t) => t,
        Err(e) => {
            let line = e.line.saturating_sub(1);
            let col = e.col.saturating_sub(1);
            diagnostics.push(json!({
                "range": {
                    "start": { "line": line, "character": col },
                    "end": { "line": line, "character": col + 1 }
                },
                "severity": 1,
                "source": "kernl",
                "message": e.to_string()
            }));
            return diagnostics;
        }
    };

    let program = match Parser::new(tokens).parse_program() {
        Ok(p) => p,
        Err(e) => {
            diagnostics.push(json!({
                "range": {
                    "start": { "line": 0, "character": 0 },
                    "end": { "line": 0, "character": 0 }
                },
                "severity": 1,
                "source": "kernl",
                "message": e.to_string()
            }));
            return diagnostics;
        }
    };

    for err in SemanticAnalyzer::check(&program) {
        let line = err.line.saturating_sub(1);
        diagnostics.push(json!({
            "range": {
                "start": { "line": line, "character": 0 },
                "end": { "line": line, "character": 0 }
            },
            "severity": 1,
            "source": "kernl",
            "message": err.to_string()
        }));
    }

    for err in TypeChecker::check(&program) {
        diagnostics.push(json!({
            "range": {
                "start": { "line": 0, "character": 0 },
                "end": { "line": 0, "character": 0 }
            },
            "severity": 1,
            "source": "kernl",
            "message": err.to_string()
        }));
    }

    diagnostics
}

fn handle_hover(params: &Value, state: &DocumentState) -> Value {
    let uri = params
        .pointer("/textDocument/uri")
        .and_then(|v| v.as_str())
        .unwrap_or_default();
    let line = params
        .pointer("/position/line")
        .and_then(|v| v.as_u64())
        .unwrap_or(0) as usize;
    let character = params
        .pointer("/position/character")
        .and_then(|v| v.as_u64())
        .unwrap_or(0) as usize;

    let source = match state.get(uri) {
        Some(s) => s,
        None => return Value::Null,
    };

    let word = word_at_position(source, line, character);
    if word.is_empty() {
        return Value::Null;
    }

    if let Some(builtin) = stdlib::get_builtin(&word) {
        let params_str: Vec<String> = builtin
            .params
            .iter()
            .map(|p| format!("{}: {}", p.name, p.ty))
            .collect();
        let sig = format!(
            "**{}**({}) → {}\n\n{}",
            builtin.name,
            params_str.join(", "),
            builtin.return_ty,
            builtin.description
        );
        return json!({
            "contents": {
                "kind": "markdown",
                "value": sig
            }
        });
    }

    if let Some(desc) = keyword_description(&word) {
        return json!({
            "contents": {
                "kind": "markdown",
                "value": desc
            }
        });
    }

    Value::Null
}

fn word_at_position(source: &str, line: usize, character: usize) -> String {
    let src_line = match source.lines().nth(line) {
        Some(l) => l,
        None => return String::new(),
    };

    if character >= src_line.len() {
        return String::new();
    }

    let bytes = src_line.as_bytes();
    let mut start = character;
    while start > 0 && (bytes[start - 1].is_ascii_alphanumeric() || bytes[start - 1] == b'_') {
        start -= 1;
    }

    let mut end = character;
    while end < bytes.len() && (bytes[end].is_ascii_alphanumeric() || bytes[end] == b'_') {
        end += 1;
    }

    src_line[start..end].to_string()
}

fn keyword_description(word: &str) -> Option<&'static str> {
    match word {
        "fn" => Some("**fn** — Define a function"),
        "in" => Some("**in** — Function input parameters"),
        "out" => Some("**out** — Function output parameter"),
        "inv" => Some("**inv** — Function invariant (must be bool)"),
        "do" => Some("**do** — Function body"),
        "let" => Some("**let** — Immutable variable binding"),
        "mut" => Some("**mut** — Mutable variable binding"),
        "if" => Some("**if** — Conditional branch"),
        "elif" => Some("**elif** — Else-if branch"),
        "else" => Some("**else** — Default branch"),
        "end" => Some("**end** — Close a block"),
        "each" => Some("**each** — Iterate over a collection"),
        "while" => Some("**while** — Loop while condition holds"),
        "struct" => Some("**struct** — Define a data structure"),
        "mod" => Some("**mod** — Module declaration"),
        "use" => Some("**use** — Import a module"),
        "mode" => Some("**mode** — AI execution mode"),
        "intent" => Some("**intent** — Declare function intent for AI"),
        "confidence" => Some("**confidence** — Confidence threshold"),
        "fallback" => Some("**fallback** — Fallback behavior"),
        "guarantee" => Some("**guarantee** — Runtime guarantee"),
        "true" => Some("**true** — Boolean literal"),
        "false" => Some("**false** — Boolean literal"),
        _ => None,
    }
}

fn handle_completion() -> Value {
    let mut items: Vec<Value> = Vec::new();

    let keywords = [
        ("fn", "Function definition"),
        ("in", "Input parameters"),
        ("out", "Output parameter"),
        ("inv", "Invariant"),
        ("do", "Function body"),
        ("let", "Immutable binding"),
        ("mut", "Mutable binding"),
        ("if", "Conditional"),
        ("elif", "Else-if"),
        ("else", "Else branch"),
        ("end", "End block"),
        ("each", "For-each loop"),
        ("while", "While loop"),
        ("struct", "Struct definition"),
        ("mod", "Module"),
        ("use", "Import"),
        ("mode", "AI mode"),
        ("intent", "Intent declaration"),
        ("confidence", "Confidence threshold"),
        ("fallback", "Fallback behavior"),
        ("guarantee", "Guarantee"),
        ("true", "Boolean true"),
        ("false", "Boolean false"),
    ];

    for (kw, detail) in &keywords {
        items.push(json!({
            "label": kw,
            "kind": 14, // Keyword
            "detail": detail
        }));
    }

    let operators = [
        ("add", "Addition"),
        ("sub", "Subtraction"),
        ("mul", "Multiplication"),
        ("div", "Division"),
        ("mod", "Modulo"),
        ("eq", "Equality"),
        ("neq", "Not equal"),
        ("gt", "Greater than"),
        ("lt", "Less than"),
        ("gte", "Greater or equal"),
        ("lte", "Less or equal"),
        ("and", "Logical and"),
        ("or", "Logical or"),
        ("not", "Logical not"),
    ];

    for (op, detail) in &operators {
        items.push(json!({
            "label": op,
            "kind": 24, // Operator
            "detail": detail
        }));
    }

    for builtin in stdlib::builtins() {
        let params_str: Vec<String> = builtin
            .params
            .iter()
            .map(|p| format!("{}: {}", p.name, p.ty))
            .collect();
        items.push(json!({
            "label": builtin.name,
            "kind": 3, // Function
            "detail": format!("({}) → {}", params_str.join(", "), builtin.return_ty),
            "documentation": builtin.description
        }));
    }

    json!({ "isIncomplete": false, "items": items })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn initialize_returns_capabilities() {
        let result = handle_initialize();
        assert_eq!(result["capabilities"]["textDocumentSync"], 1);
        assert_eq!(result["capabilities"]["hoverProvider"], true);
        assert!(result["capabilities"]["completionProvider"]["triggerCharacters"]
            .as_array()
            .unwrap()
            .contains(&json!(".")));
        assert!(result["capabilities"]["completionProvider"]["triggerCharacters"]
            .as_array()
            .unwrap()
            .contains(&json!(":")));
    }

    #[test]
    fn hover_on_builtin_returns_description() {
        let mut state = DocumentState::new();
        state.set("file:///t.knl", "print x".into());

        let params = json!({
            "textDocument": { "uri": "file:///t.knl" },
            "position": { "line": 0, "character": 0 }
        });
        let result = handle_hover(&params, &state);
        let content = result["contents"]["value"].as_str().unwrap();
        assert!(content.contains("print"), "expected print info, got: {content}");
        assert!(content.contains("Output a value"), "expected description, got: {content}");
    }

    #[test]
    fn hover_on_keyword_returns_description() {
        let mut state = DocumentState::new();
        state.set("file:///t.knl", "fn test".into());

        let params = json!({
            "textDocument": { "uri": "file:///t.knl" },
            "position": { "line": 0, "character": 0 }
        });
        let result = handle_hover(&params, &state);
        let content = result["contents"]["value"].as_str().unwrap();
        assert!(content.contains("fn"), "expected fn info, got: {content}");
    }

    #[test]
    fn hover_on_unknown_returns_null() {
        let mut state = DocumentState::new();
        state.set("file:///t.knl", "xyz".into());

        let params = json!({
            "textDocument": { "uri": "file:///t.knl" },
            "position": { "line": 0, "character": 0 }
        });
        let result = handle_hover(&params, &state);
        assert_eq!(result, Value::Null);
    }

    #[test]
    fn completion_includes_all_keywords() {
        let result = handle_completion();
        let items = result["items"].as_array().unwrap();
        let labels: Vec<&str> = items.iter().filter_map(|i| i["label"].as_str()).collect();

        let expected_keywords = [
            "fn", "in", "out", "inv", "do", "let", "mut", "if", "elif", "else", "end",
            "each", "while", "struct", "mod", "use", "mode", "intent", "confidence",
            "fallback", "guarantee", "true", "false",
        ];
        for kw in &expected_keywords {
            assert!(labels.contains(kw), "missing keyword completion: {kw}");
        }
    }

    #[test]
    fn completion_includes_all_builtins() {
        let result = handle_completion();
        let items = result["items"].as_array().unwrap();
        let labels: Vec<&str> = items.iter().filter_map(|i| i["label"].as_str()).collect();

        for name in stdlib::builtin_names() {
            assert!(labels.contains(&name), "missing builtin completion: {name}");
        }
    }

    #[test]
    fn completion_includes_operators() {
        let result = handle_completion();
        let items = result["items"].as_array().unwrap();
        let labels: Vec<&str> = items.iter().filter_map(|i| i["label"].as_str()).collect();

        let ops = [
            "add", "sub", "mul", "div", "mod", "eq", "neq", "gt", "lt", "gte", "lte",
            "and", "or", "not",
        ];
        for op in &ops {
            assert!(labels.contains(op), "missing operator completion: {op}");
        }
    }

    #[test]
    fn diagnostics_for_valid_source() {
        let diagnostics = compute_diagnostics("fn ok\n  in x: int\n  out r: int\n  do x");
        assert!(diagnostics.is_empty(), "expected no diagnostics for valid source, got: {diagnostics:?}");
    }

    #[test]
    fn diagnostics_for_lex_error() {
        let diagnostics = compute_diagnostics("fn test\n  do $invalid");
        assert!(!diagnostics.is_empty(), "expected diagnostics for lex error");
    }

    #[test]
    fn diagnostics_for_semantic_error() {
        let diagnostics = compute_diagnostics("fn bad\n  in x: int\n  out r: int\n  do add x unknown_var");
        assert!(!diagnostics.is_empty(), "expected diagnostics for undefined variable");
        let msg = diagnostics[0]["message"].as_str().unwrap();
        assert!(msg.contains("unknown_var"), "expected undefined variable error, got: {msg}");
    }

    #[test]
    fn word_at_position_basic() {
        assert_eq!(word_at_position("fn test", 0, 0), "fn");
        assert_eq!(word_at_position("fn test", 0, 3), "test");
        assert_eq!(word_at_position("fn test", 0, 5), "test");
    }

    #[test]
    fn word_at_position_empty_line() {
        assert_eq!(word_at_position("", 0, 0), "");
        assert_eq!(word_at_position("hello", 1, 0), "");
    }
}
