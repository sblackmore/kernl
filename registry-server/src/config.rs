use std::env;

const DEFAULT_PORT: u16 = 3400;
const DEFAULT_DATA_DIR: &str = "./data";
const DEFAULT_RATE_LIMIT: usize = 100;

pub struct Config {
    pub port: u16,
    pub data_dir: String,
    pub rate_limit: usize,
}

impl Config {
    pub fn from_args(args: &[String]) -> Self {
        let mut port = env::var("KERNL_REGISTRY_PORT")
            .ok()
            .and_then(|s| s.parse().ok())
            .unwrap_or(DEFAULT_PORT);
        let mut data_dir = env::var("KERNL_REGISTRY_DATA")
            .unwrap_or_else(|_| DEFAULT_DATA_DIR.to_string());
        let mut rate_limit = DEFAULT_RATE_LIMIT;

        let mut i = 0;
        while i < args.len() {
            match args[i].as_str() {
                "--port" => {
                    if i + 1 < args.len() {
                        port = args[i + 1].parse().unwrap_or(DEFAULT_PORT);
                        i += 1;
                    }
                }
                "--data-dir" => {
                    if i + 1 < args.len() {
                        data_dir = args[i + 1].clone();
                        i += 1;
                    }
                }
                "--rate-limit" => {
                    if i + 1 < args.len() {
                        rate_limit = args[i + 1].parse().unwrap_or(DEFAULT_RATE_LIMIT);
                        i += 1;
                    }
                }
                _ => {}
            }
            i += 1;
        }

        Self {
            port,
            data_dir,
            rate_limit,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_defaults() {
        let config = Config::from_args(&[]);
        assert_eq!(config.port, 3400);
        assert_eq!(config.data_dir, "./data");
        assert_eq!(config.rate_limit, 100);
    }

    #[test]
    fn test_custom_port() {
        let args = vec!["--port".into(), "8080".into()];
        let config = Config::from_args(&args);
        assert_eq!(config.port, 8080);
        assert_eq!(config.data_dir, "./data");
    }

    #[test]
    fn test_custom_data_dir() {
        let args = vec!["--data-dir".into(), "/tmp/registry".into()];
        let config = Config::from_args(&args);
        assert_eq!(config.port, 3400);
        assert_eq!(config.data_dir, "/tmp/registry");
    }

    #[test]
    fn test_all_args() {
        let args = vec![
            "--port".into(),
            "9000".into(),
            "--data-dir".into(),
            "/var/data".into(),
            "--rate-limit".into(),
            "50".into(),
        ];
        let config = Config::from_args(&args);
        assert_eq!(config.port, 9000);
        assert_eq!(config.data_dir, "/var/data");
        assert_eq!(config.rate_limit, 50);
    }

    #[test]
    fn test_invalid_port_uses_default() {
        let args = vec!["--port".into(), "not_a_number".into()];
        let config = Config::from_args(&args);
        assert_eq!(config.port, 3400);
    }

    #[test]
    fn test_rate_limit_custom() {
        let args = vec!["--rate-limit".into(), "25".into()];
        let config = Config::from_args(&args);
        assert_eq!(config.rate_limit, 25);
    }
}
