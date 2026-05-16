mod config;
mod handler;
mod llm;

use config::Config;
use llm::LlmClient;
use std::env;
use std::sync::Arc;

fn main() {
    let args: Vec<String> = env::args().skip(1).collect();
    let config = Config::from_args(&args);

    let client = Arc::new(LlmClient::new(
        config.llm_endpoint.clone(),
        config.llm_model.clone(),
        config.llm_api_key.clone(),
    ));

    let addr = format!("0.0.0.0:{}", config.port);
    let server = tiny_http::Server::http(&addr).unwrap_or_else(|e| {
        eprintln!("Failed to bind to {addr}: {e}");
        std::process::exit(1);
    });

    println!("kernl-resolver v{}", env!("CARGO_PKG_VERSION"));
    println!("  port:     {}", config.port);
    println!("  endpoint: {}", config.llm_endpoint);
    println!("  model:    {}", config.llm_model);
    println!("  api_key:  {}", if config.llm_api_key.is_empty() { "(not set)" } else { "(set)" });
    println!("Listening on {addr}...");

    for request in server.incoming_requests() {
        let client = Arc::clone(&client);
        handler::handle_request(request, &client);
    }
}
