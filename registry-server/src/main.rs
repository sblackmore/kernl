mod config;
mod handlers;
mod storage;

use std::env;
use std::path::Path;
use std::sync::Arc;

use config::Config;
use storage::Storage;

fn main() {
    let args: Vec<String> = env::args().skip(1).collect();
    let config = Config::from_args(&args);

    let storage = Arc::new(Storage::new(Path::new(&config.data_dir)));

    let addr = format!("0.0.0.0:{}", config.port);
    let server = tiny_http::Server::http(&addr).unwrap_or_else(|e| {
        eprintln!("Failed to start server on {addr}: {e}");
        std::process::exit(1);
    });

    eprintln!(
        "kernl-registry listening on port {}, data dir: {}",
        config.port, config.data_dir
    );

    for request in server.incoming_requests() {
        let storage = Arc::clone(&storage);
        std::thread::spawn(move || {
            handlers::handle_request(request, &storage);
        });
    }
}
