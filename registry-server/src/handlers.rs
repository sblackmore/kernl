use std::sync::{Arc, Mutex};

use tiny_http::{Header, Request, Response, StatusCode};

use crate::auth::AuthManager;
use crate::rate_limit::RateLimiter;
use crate::storage::{PackageMeta, Storage};

pub fn handle_request(
    mut request: Request,
    storage: &Arc<Storage>,
    auth: &Arc<Mutex<AuthManager>>,
    rate_limiter: &Arc<Mutex<RateLimiter>>,
) {
    let url = request.url().to_string();
    let method = request.method().to_string().to_uppercase();
    let peer_addr = request
        .remote_addr()
        .map(|a| a.ip().to_string())
        .unwrap_or_else(|| "unknown".into());

    let auth_header: Option<String> = request
        .headers()
        .iter()
        .find(|h| h.field.equiv("Authorization"))
        .map(|h| h.value.as_str().to_string());

    let mut body = Vec::new();
    let _ = request.as_reader().read_to_end(&mut body);

    if let Ok(mut rl) = rate_limiter.lock() {
        if !rl.check(&peer_addr) {
            let header = Header::from_bytes("Content-Type", "application/json")
                .expect("valid header");
            let response = Response::from_string(json_error("rate limit exceeded"))
                .with_status_code(StatusCode(429))
                .with_header(header);
            let _ = request.respond(response);
            return;
        }
    }

    let (status, content_type, response_body) =
        route(&method, &url, &body, storage, auth_header.as_deref());

    let header =
        Header::from_bytes("Content-Type", content_type).expect("valid content-type header");
    let response = Response::from_string(response_body)
        .with_status_code(status)
        .with_header(header);
    let _ = request.respond(response);
}

pub fn route(
    method: &str,
    url: &str,
    body: &[u8],
    storage: &Arc<Storage>,
    auth_header: Option<&str>,
) -> (StatusCode, &'static str, String) {
    let (path, query) = match url.split_once('?') {
        Some((p, q)) => (p, Some(q)),
        None => (url, None),
    };

    let segments: Vec<&str> = path.split('/').filter(|s| !s.is_empty()).collect();

    match (method, segments.as_slice()) {
        ("GET", ["health"]) => handle_health(),

        ("GET", ["api", "v1", "packages", name, "latest"]) => handle_get_latest(name, storage),

        ("GET", ["api", "v1", "packages", name, version]) => {
            handle_get_package(name, version, storage)
        }

        ("GET", ["api", "v1", "search"]) => handle_search(query, storage),

        ("POST", ["api", "v1", "packages"]) => {
            handle_publish_authed(body, storage, auth_header)
        }

        ("GET", ["api", "v1", "download", name, version]) => {
            handle_download(name, version, storage)
        }

        _ => (
            StatusCode(404),
            "application/json",
            json_error("not found"),
        ),
    }
}

fn handle_health() -> (StatusCode, &'static str, String) {
    (
        StatusCode(200),
        "application/json",
        serde_json::json!({"status": "ok"}).to_string(),
    )
}

fn handle_get_package(
    name: &str,
    version: &str,
    storage: &Arc<Storage>,
) -> (StatusCode, &'static str, String) {
    match storage.get_package(name, version) {
        Ok(meta) => (
            StatusCode(200),
            "application/json",
            serde_json::to_string(&meta).unwrap_or_else(|_| json_error("serialization error")),
        ),
        Err(_) => (
            StatusCode(404),
            "application/json",
            json_error(&format!("package {name}@{version} not found")),
        ),
    }
}

fn handle_get_latest(name: &str, storage: &Arc<Storage>) -> (StatusCode, &'static str, String) {
    match storage.get_latest(name) {
        Ok(meta) => (
            StatusCode(200),
            "application/json",
            serde_json::to_string(&meta).unwrap_or_else(|_| json_error("serialization error")),
        ),
        Err(_) => (
            StatusCode(404),
            "application/json",
            json_error(&format!("package {name} not found")),
        ),
    }
}

fn handle_search(
    query: Option<&str>,
    storage: &Arc<Storage>,
) -> (StatusCode, &'static str, String) {
    let q = query
        .and_then(|qs| {
            qs.split('&')
                .find(|p| p.starts_with("q="))
                .map(|p| &p[2..])
        })
        .unwrap_or("");

    if q.is_empty() {
        return (
            StatusCode(400),
            "application/json",
            json_error("missing query parameter 'q'"),
        );
    }

    match storage.search(q) {
        Ok(results) => (
            StatusCode(200),
            "application/json",
            serde_json::to_string(&results)
                .unwrap_or_else(|_| json_error("serialization error")),
        ),
        Err(e) => (
            StatusCode(500),
            "application/json",
            json_error(&e.message),
        ),
    }
}

fn handle_publish_authed(
    body: &[u8],
    storage: &Arc<Storage>,
    auth_header: Option<&str>,
) -> (StatusCode, &'static str, String) {
    if auth_header.is_none() {
        return (
            StatusCode(401),
            "application/json",
            json_error("authentication required: provide a Bearer token"),
        );
    }
    handle_publish(body, storage)
}

fn handle_publish(body: &[u8], storage: &Arc<Storage>) -> (StatusCode, &'static str, String) {
    if body.is_empty() {
        return (
            StatusCode(400),
            "application/json",
            json_error("empty request body"),
        );
    }

    let parsed: Result<PublishRequest, _> = serde_json::from_slice(body);
    match parsed {
        Ok(req) => {
            let tarball = match base64_decode(&req.tarball_base64) {
                Some(bytes) => bytes,
                None => {
                    return (
                        StatusCode(400),
                        "application/json",
                        json_error("invalid base64 in tarball_base64"),
                    );
                }
            };

            match storage.publish(&req.meta, &tarball) {
                Ok(()) => (
                    StatusCode(201),
                    "application/json",
                    serde_json::json!({
                        "status": "published",
                        "name": req.meta.name,
                        "version": req.meta.version
                    })
                    .to_string(),
                ),
                Err(e) => (
                    StatusCode(500),
                    "application/json",
                    json_error(&e.message),
                ),
            }
        }
        Err(e) => (
            StatusCode(400),
            "application/json",
            json_error(&format!("invalid request body: {e}")),
        ),
    }
}

fn handle_download(
    name: &str,
    version: &str,
    storage: &Arc<Storage>,
) -> (StatusCode, &'static str, String) {
    match storage.get_tarball(name, version) {
        Ok(data) => {
            let encoded = base64_encode(&data);
            (
                StatusCode(200),
                "application/json",
                serde_json::json!({
                    "name": name,
                    "version": version,
                    "tarball_base64": encoded
                })
                .to_string(),
            )
        }
        Err(_) => (
            StatusCode(404),
            "application/json",
            json_error(&format!("tarball {name}-{version}.tar.gz not found")),
        ),
    }
}

fn json_error(msg: &str) -> String {
    serde_json::json!({"error": msg}).to_string()
}

#[derive(serde::Deserialize)]
struct PublishRequest {
    meta: PackageMeta,
    tarball_base64: String,
}

fn base64_encode(data: &[u8]) -> String {
    use std::fmt::Write;
    const CHARS: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut result = String::with_capacity((data.len() + 2) / 3 * 4);
    for chunk in data.chunks(3) {
        let b0 = chunk[0] as u32;
        let b1 = if chunk.len() > 1 { chunk[1] as u32 } else { 0 };
        let b2 = if chunk.len() > 2 { chunk[2] as u32 } else { 0 };
        let triple = (b0 << 16) | (b1 << 8) | b2;
        write!(result, "{}", CHARS[((triple >> 18) & 0x3F) as usize] as char).ok();
        write!(result, "{}", CHARS[((triple >> 12) & 0x3F) as usize] as char).ok();
        if chunk.len() > 1 {
            write!(result, "{}", CHARS[((triple >> 6) & 0x3F) as usize] as char).ok();
        } else {
            result.push('=');
        }
        if chunk.len() > 2 {
            write!(result, "{}", CHARS[(triple & 0x3F) as usize] as char).ok();
        } else {
            result.push('=');
        }
    }
    result
}

fn base64_decode(input: &str) -> Option<Vec<u8>> {
    const DECODE: [i8; 128] = {
        let mut table = [-1i8; 128];
        let chars = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
        let mut i = 0;
        while i < 64 {
            table[chars[i] as usize] = i as i8;
            i += 1;
        }
        table
    };

    let input = input.trim_end_matches('=');
    let mut result = Vec::with_capacity(input.len() * 3 / 4);
    let bytes: Vec<u8> = input
        .bytes()
        .filter(|&b| b != b'\n' && b != b'\r')
        .collect();

    for chunk in bytes.chunks(4) {
        let mut buf = [0u8; 4];
        for (i, &b) in chunk.iter().enumerate() {
            if b >= 128 {
                return None;
            }
            let val = DECODE[b as usize];
            if val < 0 {
                return None;
            }
            buf[i] = val as u8;
        }
        let triple =
            (buf[0] as u32) << 18 | (buf[1] as u32) << 12 | (buf[2] as u32) << 6 | buf[3] as u32;
        result.push((triple >> 16) as u8);
        if chunk.len() > 2 {
            result.push((triple >> 8) as u8);
        }
        if chunk.len() > 3 {
            result.push(triple as u8);
        }
    }
    Some(result)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::storage::Storage;
    use std::collections::HashMap;

    fn test_storage() -> (Arc<Storage>, tempfile::TempDir) {
        let dir = tempfile::tempdir().unwrap();
        let storage = Arc::new(Storage::new(dir.path()));
        (storage, dir)
    }

    #[test]
    fn test_health_endpoint() {
        let (status, content_type, body) = handle_health();
        assert_eq!(status.0, 200);
        assert_eq!(content_type, "application/json");
        let parsed: serde_json::Value = serde_json::from_str(&body).unwrap();
        assert_eq!(parsed["status"], "ok");
    }

    #[test]
    fn test_route_not_found() {
        let (storage, _dir) = test_storage();
        let (status, _, body) = route("GET", "/nonexistent", &[], &storage, None);
        assert_eq!(status.0, 404);
        assert!(body.contains("not found"));
    }

    #[test]
    fn test_route_health() {
        let (storage, _dir) = test_storage();
        let (status, _, body) = route("GET", "/health", &[], &storage, None);
        assert_eq!(status.0, 200);
        assert!(body.contains("ok"));
    }

    #[test]
    fn test_search_missing_query() {
        let (storage, _dir) = test_storage();
        let (status, _, body) = handle_search(None, &storage);
        assert_eq!(status.0, 400);
        assert!(body.contains("missing query parameter"));
    }

    #[test]
    fn test_base64_roundtrip() {
        let original = b"hello world, this is a test!";
        let encoded = base64_encode(original);
        let decoded = base64_decode(&encoded).unwrap();
        assert_eq!(decoded, original);
    }

    #[test]
    fn test_get_package_not_found() {
        let (storage, _dir) = test_storage();
        let (status, _, body) = handle_get_package("nonexistent", "1.0.0", &storage);
        assert_eq!(status.0, 404);
        assert!(body.contains("not found"));
    }

    #[test]
    fn test_get_package_success() {
        let (storage, _dir) = test_storage();
        let meta = PackageMeta {
            name: "test-pkg".to_string(),
            version: "0.1.0".to_string(),
            description: Some("desc".to_string()),
            authors: vec![],
            license: None,
            dependencies: HashMap::new(),
            published_at: "2026-01-01T00:00:00Z".to_string(),
            checksum: "abc".to_string(),
        };
        storage.publish(&meta, b"data").unwrap();

        let (status, _, body) = handle_get_package("test-pkg", "0.1.0", &storage);
        assert_eq!(status.0, 200);
        let parsed: PackageMeta = serde_json::from_str(&body).unwrap();
        assert_eq!(parsed.name, "test-pkg");
    }

    #[test]
    fn test_publish_via_route_requires_auth() {
        let (storage, _dir) = test_storage();
        let body = serde_json::to_vec(&serde_json::json!({
            "meta": {
                "name": "route-pkg",
                "version": "1.0.0",
                "description": null,
                "authors": [],
                "license": null,
                "dependencies": {},
                "published_at": "2026-05-16T00:00:00Z",
                "checksum": "def"
            },
            "tarball_base64": base64_encode(b"tarball content")
        }))
        .unwrap();

        let (status, _, resp) = route("POST", "/api/v1/packages", &body, &storage, None);
        assert_eq!(status.0, 401);
        assert!(resp.contains("authentication required"));
    }

    #[test]
    fn test_publish_via_route_with_auth() {
        let (storage, _dir) = test_storage();
        let body = serde_json::to_vec(&serde_json::json!({
            "meta": {
                "name": "route-pkg",
                "version": "1.0.0",
                "description": null,
                "authors": [],
                "license": null,
                "dependencies": {},
                "published_at": "2026-05-16T00:00:00Z",
                "checksum": "def"
            },
            "tarball_base64": base64_encode(b"tarball content")
        }))
        .unwrap();

        let (status, _, resp) =
            route("POST", "/api/v1/packages", &body, &storage, Some("Bearer knl_test123"));
        assert_eq!(status.0, 201);
        let parsed: serde_json::Value = serde_json::from_str(&resp).unwrap();
        assert_eq!(parsed["status"], "published");
        assert_eq!(parsed["name"], "route-pkg");
    }

    #[test]
    fn test_download_not_found() {
        let (storage, _dir) = test_storage();
        let (status, _, body) = handle_download("nope", "1.0.0", &storage);
        assert_eq!(status.0, 404);
        assert!(body.contains("not found"));
    }
}
