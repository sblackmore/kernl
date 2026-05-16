const DEFAULT_PORT: u16 = 3400;
const DEFAULT_DATA_DIR: &str = "./data";

pub struct Config {
    pub port: u16,
    pub data_dir: String,
}

impl Config {
    pub fn from_args(args: &[String]) -> Self {
        let mut port = DEFAULT_PORT;
        let mut data_dir = DEFAULT_DATA_DIR.to_string();

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
                _ => {}
            }
            i += 1;
        }

        Self { port, data_dir }
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
        ];
        let config = Config::from_args(&args);
        assert_eq!(config.port, 9000);
        assert_eq!(config.data_dir, "/var/data");
    }

    #[test]
    fn test_invalid_port_uses_default() {
        let args = vec!["--port".into(), "not_a_number".into()];
        let config = Config::from_args(&args);
        assert_eq!(config.port, 3400);
    }
}
