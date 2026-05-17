//! Lambda bootstrap: map API Gateway HTTP API v2 events to a small stdin protocol,
//! run `kernlc kn/order_api.knl --invoke-stdin --run`, then merge stdout back into an in-memory order store.

use base64::{engine::general_purpose::STANDARD as B64_ENGINE, Engine};
use lambda_runtime::{service_fn, Error, LambdaEvent};
use serde_json::{json, Value};
use std::io::Write;
use std::path::PathBuf;
use std::process::{Command, Stdio};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Mutex, OnceLock};

#[derive(Clone, Debug)]
struct Order {
    id: String,
    customer_id: String,
    total_cents: i64,
    status: String,
}

impl Order {
    fn to_tsv(&self) -> String {
        format!(
            "{}\t{}\t{}\t{}",
            self.id, self.customer_id, self.total_cents, self.status
        )
    }

    fn from_tsv(line: &str) -> Option<Order> {
        let mut it = line.split('\t');
        Some(Order {
            id: it.next()?.to_string(),
            customer_id: it.next()?.to_string(),
            total_cents: it.next()?.parse().ok()?,
            status: it.next()?.to_string(),
        })
    }
}

fn kernlc_path() -> PathBuf {
    std::env::var("LAMBDA_TASK_ROOT")
        .map(PathBuf::from)
        .unwrap_or_else(|_| PathBuf::from("/var/task"))
        .join("kernlc")
}

fn kn_path() -> PathBuf {
    std::env::var("LAMBDA_TASK_ROOT")
        .map(PathBuf::from)
        .unwrap_or_else(|_| PathBuf::from("/var/task"))
        .join("kn")
        .join("order_api.knl")
}

fn orders_store() -> &'static Mutex<Vec<Order>> {
    static STORE: OnceLock<Mutex<Vec<Order>>> = OnceLock::new();
    STORE.get_or_init(|| {
        Mutex::new(vec![
            Order {
                id: "ord-1001".into(),
                customer_id: "cust-42".into(),
                total_cents: 1299,
                status: "paid".into(),
            },
            Order {
                id: "ord-1002".into(),
                customer_id: "cust-77".into(),
                total_cents: 450,
                status: "open".into(),
            },
        ])
    })
}

static NEXT_ORD: AtomicU64 = AtomicU64::new(1003);

fn alloc_order_id() -> String {
    let n = NEXT_ORD.fetch_add(1, Ordering::SeqCst);
    format!("ord-{n}")
}

fn build_stdin(op: &str, payload: &str, orders: &[Order]) -> String {
    let mut s = String::new();
    s.push_str(op);
    s.push('\n');
    s.push_str(payload);
    s.push('\n');
    for (i, o) in orders.iter().enumerate() {
        if i > 0 {
            s.push('\n');
        }
        s.push_str(&o.to_tsv());
    }
    s
}

fn invoke_kernlc(stdin_blob: &str) -> Result<(u16, Value, Option<Vec<Order>>), Error> {
    let kernlc = kernlc_path();
    let kn = kn_path();

    let mut child = Command::new(&kernlc)
        .arg(&kn)
        .arg("--invoke-stdin")
        .arg("--run")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|e| Error::from(format!("failed to spawn kernlc: {e}")))?;

    if let Some(mut stdin) = child.stdin.take() {
        stdin
            .write_all(stdin_blob.as_bytes())
            .map_err(|e| Error::from(format!("failed to write kernlc stdin: {e}")))?;
    }

    let output = child
        .wait_with_output()
        .map_err(|e| Error::from(format!("kernlc wait_with_output: {e}")))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(Error::from(format!(
            "kernlc exited {}: {}",
            output.status, stderr
        )));
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    let lines: Vec<&str> = stdout.lines().map(str::trim_end).collect();

    if lines.len() < 3 {
        return Err(Error::from(format!(
            "kernlc stdout expected >= 3 lines, got {:?}",
            lines
        )));
    }

    let status: u16 = lines[0]
        .trim()
        .parse()
        .map_err(|e| Error::from(format!("invalid status line {:?}: {e}", lines[0])))?;

    let inner_body: Value =
        serde_json::from_str(lines[1].trim()).map_err(|e| Error::from(format!(
            "kernlc body line was not JSON: {e}; raw={:?}",
            lines[1]
        )))?;

    match lines[2].trim() {
        "__KEEP_STATE__" => Ok((status, inner_body, None)),
        "__STATE__" => {
            let mut rows = Vec::new();
            for row in lines.iter().skip(3) {
                let row = row.trim();
                if row.is_empty() {
                    continue;
                }
                if let Some(o) = Order::from_tsv(row) {
                    rows.push(o);
                }
            }
            Ok((status, inner_body, Some(rows)))
        }
        other => Err(Error::from(format!(
            "expected __KEEP_STATE__ or __STATE__, got {other:?}"
        ))),
    }
}

fn api_gateway_response(status: u16, body: &Value) -> Value {
    json!({
        "statusCode": status,
        "headers": { "Content-Type": "application/json" },
        "body": serde_json::to_string(body).unwrap_or_else(|_| "{}".into()),
    })
}

/// API Gateway HTTP API v2 may omit `body`, send it base64 (`isBase64Encoded`), or use a non-string JSON shape.
fn http_api_body(event: &Value) -> Result<String, Error> {
    let Some(body_field) = event.get("body") else {
        return Ok(String::new());
    };
    if body_field.is_null() {
        return Ok(String::new());
    }

    let raw = match body_field {
        Value::String(s) => s.clone(),
        other => serde_json::to_string(other)
            .map_err(|e| Error::from(format!("serialize body field: {e}")))?,
    };

    let is_b64 = event
        .get("isBase64Encoded")
        .and_then(Value::as_bool)
        .unwrap_or(false);

    if is_b64 {
        let bytes = B64_ENGINE
            .decode(raw.trim())
            .map_err(|e| Error::from(format!("base64 decode body: {e}")))?;
        String::from_utf8(bytes).map_err(|e| Error::from(format!("body utf-8: {e}")))
    } else {
        Ok(raw)
    }
}

fn request_path<'a>(event: &'a Value) -> &'a str {
    if let Some(s) = event.get("rawPath").and_then(Value::as_str) {
        return s;
    }
    if let Some(s) = event.get("path").and_then(Value::as_str) {
        return s;
    }
    event
        .pointer("/requestContext/http/path")
        .and_then(Value::as_str)
        .unwrap_or("/")
}

fn merge_order(existing: &Order, patch: &Value) -> Order {
    let mut o = existing.clone();
    if let Some(s) = patch.get("customerId").and_then(Value::as_str) {
        o.customer_id = s.to_string();
    }
    if let Some(n) = patch.get("totalCents").and_then(Value::as_i64) {
        o.total_cents = n;
    }
    if let Some(s) = patch.get("status").and_then(Value::as_str) {
        o.status = s.to_string();
    }
    o
}

#[derive(Debug)]
enum RouteOutcome {
    Invoke { op: String, payload: String },
    Early { status: u16, body: Value },
}

fn route_op(event: &Value, orders: &[Order]) -> Result<RouteOutcome, Error> {
    let method = event["requestContext"]["http"]["method"]
        .as_str()
        .unwrap_or("GET")
        .to_ascii_uppercase();

    let path = request_path(event);
    let body_raw = http_api_body(event)?;

    match (method.as_str(), path) {
        ("GET", "/health") => Ok(RouteOutcome::Invoke {
            op: "health".into(),
            payload: String::new(),
        }),
        ("GET", "/customers") => Ok(RouteOutcome::Invoke {
            op: "customers.list".into(),
            payload: String::new(),
        }),
        ("GET", "/orders") => Ok(RouteOutcome::Invoke {
            op: "orders.list".into(),
            payload: String::new(),
        }),
        ("GET", p) if p.starts_with("/orders/") => {
            let id = p.trim_start_matches("/orders/").trim();
            if id.is_empty() {
                return Ok(RouteOutcome::Early {
                    status: 400,
                    body: json!({"error":"bad_request","detail":"missing order id"}),
                });
            }
            Ok(RouteOutcome::Invoke {
                op: "orders.get".into(),
                payload: id.to_string(),
            })
        }
        ("POST", "/orders") => {
            let v: Value = match serde_json::from_str(body_raw.trim()) {
                Ok(v) => v,
                Err(e) => {
                    return Ok(RouteOutcome::Early {
                        status: 400,
                        body: json!({"error":"bad_request","detail": format!("invalid JSON body: {e}")}),
                    });
                }
            };
            let customer_id = v["customerId"]
                .as_str()
                .unwrap_or("cust-unknown")
                .to_string();
            let total_cents = v["totalCents"].as_i64().unwrap_or(0);
            let status = v["status"].as_str().unwrap_or("open").to_string();
            let id = alloc_order_id();
            let row = Order {
                id,
                customer_id,
                total_cents,
                status,
            };
            Ok(RouteOutcome::Invoke {
                op: "orders.add".into(),
                payload: row.to_tsv(),
            })
        }
        ("PATCH", p) if p.starts_with("/orders/") => {
            let id = p.trim_start_matches("/orders/").trim();
            if id.is_empty() {
                return Ok(RouteOutcome::Early {
                    status: 400,
                    body: json!({"error":"bad_request","detail":"missing order id"}),
                });
            }
            let patch: Value = if body_raw.trim().is_empty() {
                json!({})
            } else {
                match serde_json::from_str(body_raw.trim()) {
                    Ok(v) => v,
                    Err(e) => {
                        return Ok(RouteOutcome::Early {
                            status: 400,
                            body: json!({"error":"bad_request","detail": format!("invalid JSON body: {e}")}),
                        });
                    }
                }
            };
            let Some(existing) = orders.iter().find(|o| o.id == id) else {
                return Ok(RouteOutcome::Early {
                    status: 404,
                    body: json!({"error":"not_found","detail": format!("unknown order id {id}")}),
                });
            };
            let merged = merge_order(existing, &patch);
            Ok(RouteOutcome::Invoke {
                op: "orders.update".into(),
                payload: merged.to_tsv(),
            })
        }
        ("DELETE", p) if p.starts_with("/orders/") => {
            let id = p.trim_start_matches("/orders/").trim();
            if id.is_empty() {
                return Ok(RouteOutcome::Early {
                    status: 400,
                    body: json!({"error":"bad_request","detail":"missing order id"}),
                });
            }
            Ok(RouteOutcome::Invoke {
                op: "orders.delete".into(),
                payload: id.to_string(),
            })
        }
        _ => Ok(RouteOutcome::Invoke {
            op: format!("unknown:{method}:{path}"),
            payload: String::new(),
        }),
    }
}

async fn handler(event: LambdaEvent<Value>) -> Result<Value, Error> {
    let mut guard = orders_store()
        .lock()
        .map_err(|_| Error::from("orders store lock poisoned"))?;

    let outcome = route_op(&event.payload, &guard)?;

    match outcome {
        RouteOutcome::Early { status, body } => {
            drop(guard);
            Ok(api_gateway_response(status, &body))
        }
        RouteOutcome::Invoke { op, payload } => {
            let stdin_blob = build_stdin(&op, &payload, &guard);
            let (status, inner_body, new_orders) = invoke_kernlc(&stdin_blob)?;

            if let Some(rows) = new_orders {
                *guard = rows;
            }
            drop(guard);

            Ok(api_gateway_response(status, &inner_body))
        }
    }
}

#[tokio::main]
async fn main() -> Result<(), Error> {
    lambda_runtime::run(service_fn(handler)).await
}
