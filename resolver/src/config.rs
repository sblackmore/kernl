use std::env;

pub struct Config {
    pub port: u16,
    pub llm_endpoint: String,
    pub llm_model: String,
    pub llm_api_key: String,
}

impl Config {
    pub fn from_args(args: &[String]) -> Self {
        let mut port: u16 = 8420;
        let mut llm_endpoint = String::from("https://api.openai.com/v1");
        let mut llm_model = String::from("gpt-4o");
        let mut llm_api_key = env::var("KERNL_LLM_API_KEY").unwrap_or_default();

        let mut i = 0;
        while i < args.len() {
            match args[i].as_str() {
                "--port" => {
                    i += 1;
                    if let Some(val) = args.get(i) {
                        port = val.parse().expect("invalid port number");
                    }
                }
                "--llm-endpoint" => {
                    i += 1;
                    if let Some(val) = args.get(i) {
                        llm_endpoint = val.clone();
                    }
                }
                "--llm-model" => {
                    i += 1;
                    if let Some(val) = args.get(i) {
                        llm_model = val.clone();
                    }
                }
                "--llm-api-key" => {
                    i += 1;
                    if let Some(val) = args.get(i) {
                        llm_api_key = val.clone();
                    }
                }
                _ => {}
            }
            i += 1;
        }

        Config {
            port,
            llm_endpoint,
            llm_model,
            llm_api_key,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_config() {
        let config = Config::from_args(&[]);
        assert_eq!(config.port, 8420);
        assert_eq!(config.llm_endpoint, "https://api.openai.com/v1");
        assert_eq!(config.llm_model, "gpt-4o");
    }

    #[test]
    fn test_arg_parsing() {
        let args: Vec<String> = vec![
            "--port", "9000",
            "--llm-endpoint", "http://localhost:11434/v1",
            "--llm-model", "llama3",
            "--llm-api-key", "test-key-123",
        ]
        .into_iter()
        .map(String::from)
        .collect();

        let config = Config::from_args(&args);
        assert_eq!(config.port, 9000);
        assert_eq!(config.llm_endpoint, "http://localhost:11434/v1");
        assert_eq!(config.llm_model, "llama3");
        assert_eq!(config.llm_api_key, "test-key-123");
    }

    #[test]
    fn test_partial_args() {
        let args: Vec<String> = vec!["--port", "3000"]
            .into_iter()
            .map(String::from)
            .collect();

        let config = Config::from_args(&args);
        assert_eq!(config.port, 3000);
        assert_eq!(config.llm_endpoint, "https://api.openai.com/v1");
        assert_eq!(config.llm_model, "gpt-4o");
    }
}
