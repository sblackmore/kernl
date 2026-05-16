mod error;
mod handlers;
mod state;
mod transport;

use std::io::{self, BufReader, BufWriter};

use state::DocumentState;
use transport::{read_message, write_message};

fn main() {
    let stdin = io::stdin();
    let stdout = io::stdout();
    let mut reader = BufReader::new(stdin.lock());
    let mut writer = BufWriter::new(stdout.lock());
    let mut state = DocumentState::new();

    eprintln!("kernl-lsp: starting");

    loop {
        let msg = match read_message(&mut reader) {
            Ok(msg) => msg,
            Err(e) => {
                eprintln!("kernl-lsp: read error: {e}");
                break;
            }
        };

        let method = match msg.get("method").and_then(|m| m.as_str()) {
            Some(m) => m.to_string(),
            None => continue,
        };

        let params = msg.get("params").cloned().unwrap_or(serde_json::Value::Null);
        let id = msg.get("id").cloned();

        if method == "exit" {
            std::process::exit(0);
        }

        let result = handlers::handle_request(&method, &params, &mut state, &mut writer);

        if let (Some(id), Some(result)) = (id, result) {
            let response = serde_json::json!({
                "jsonrpc": "2.0",
                "id": id,
                "result": result
            });
            if let Err(e) = write_message(&mut writer, &response) {
                eprintln!("kernl-lsp: write error: {e}");
                break;
            }
        }
    }
}
