mod auth;
mod config;
mod handlers;
mod rate_limit;
mod storage;

use std::env;
use std::path::Path;
use std::sync::{Arc, Mutex};

use auth::AuthManager;
use config::Config;
use rate_limit::RateLimiter;
use storage::Storage;

fn main() {
    let args: Vec<String> = env::args().skip(1).collect();
    let config = Config::from_args(&args);

    let data_path = Path::new(&config.data_dir);
    let storage = Arc::new(Storage::new(data_path));
    let auth = Arc::new(Mutex::new(AuthManager::new(data_path)));
    let rate_limiter = Arc::new(Mutex::new(RateLimiter::new(60, config.rate_limit)));

    let addr = format!("0.0.0.0:{}", config.port);
    let server = tiny_http::Server::http(&addr).unwrap_or_else(|e| {
        eprintln!("Failed to start server on {addr}: {e}");
        std::process::exit(1);
    });

    eprintln!(
        "kernl-registry listening on port {}, data dir: {}, rate limit: {}/min",
        config.port, config.data_dir, config.rate_limit
    );

    for request in server.incoming_requests() {
        let storage = Arc::clone(&storage);
        let auth = Arc::clone(&auth);
        let rate_limiter = Arc::clone(&rate_limiter);
        std::thread::spawn(move || {
            handlers::handle_request(request, &storage, &auth, &rate_limiter);
        });
    }
}
