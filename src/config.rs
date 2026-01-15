use serde::Deserialize;
use std::net::IpAddr;
use std::path::PathBuf;

/// Logging verbosity levels
#[derive(Debug, Clone, Copy, Deserialize, Default, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum LogLevel {
    Trace,
    Debug,
    #[default]
    Info,
    Warn,
    Error,
}

impl LogLevel {
    pub fn as_tracing_level(&self) -> tracing::Level {
        match self {
            LogLevel::Trace => tracing::Level::TRACE,
            LogLevel::Debug => tracing::Level::DEBUG,
            LogLevel::Info => tracing::Level::INFO,
            LogLevel::Warn => tracing::Level::WARN,
            LogLevel::Error => tracing::Level::ERROR,
        }
    }
}

impl std::fmt::Display for LogLevel {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            LogLevel::Trace => write!(f, "trace"),
            LogLevel::Debug => write!(f, "debug"),
            LogLevel::Info => write!(f, "info"),
            LogLevel::Warn => write!(f, "warn"),
            LogLevel::Error => write!(f, "error"),
        }
    }
}

/// Logging output destination
#[derive(Debug, Clone, Deserialize, Default, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum LogOutput {
    #[default]
    Stdout,
    Stderr,
    #[serde(untagged)]
    File(PathBuf),
}

/// Logging format
#[derive(Debug, Clone, Copy, Deserialize, Default, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum LogFormat {
    #[default]
    Compact,
    Pretty,
    Json,
}

/// Logging configuration section
#[derive(Debug, Clone, Deserialize)]
pub struct LoggingConfig {
    #[serde(default)]
    pub level: LogLevel,
    #[serde(default)]
    pub output: LogOutput,
    #[serde(default)]
    pub format: LogFormat,
}

impl Default for LoggingConfig {
    fn default() -> Self {
        Self {
            level: LogLevel::Info,
            output: LogOutput::Stdout,
            format: LogFormat::Compact,
        }
    }
}

/// Server configuration section
#[derive(Debug, Clone, Deserialize)]
pub struct ServerConfig {
    #[serde(default = "default_port")]
    pub port: u16,
    #[serde(default = "default_bind_ip")]
    pub bind_ip: IpAddr,
}

fn default_port() -> u16 {
    3000
}

fn default_bind_ip() -> IpAddr {
    IpAddr::V4(std::net::Ipv4Addr::new(0, 0, 0, 0))
}

impl Default for ServerConfig {
    fn default() -> Self {
        Self {
            port: default_port(),
            bind_ip: default_bind_ip(),
        }
    }
}

/// Main configuration structure
#[derive(Debug, Clone, Deserialize, Default)]
pub struct Config {
    #[serde(default)]
    pub logging: LoggingConfig,
    #[serde(default)]
    pub server: ServerConfig,
}

impl Config {
    /// Load configuration with the following priority (highest to lowest):
    /// 1. CLI-specified config file path
    /// 2. CONFIG_FILE environment variable
    /// 3. Default values
    pub fn load(cli_config_path: Option<&PathBuf>) -> anyhow::Result<Self> {
        // Try CLI path first
        if let Some(path) = cli_config_path {
            return Self::load_from_file(path);
        }

        // Try CONFIG_FILE environment variable
        if let Ok(env_path) = std::env::var("CONFIG_FILE") {
            let path = PathBuf::from(env_path);
            return Self::load_from_file(&path);
        }

        // Return defaults
        Ok(Self::default())
    }

    /// Load configuration from a specific file
    fn load_from_file(path: &PathBuf) -> anyhow::Result<Self> {
        let contents = std::fs::read_to_string(path)
            .map_err(|e| anyhow::anyhow!("Failed to read config file '{}': {}", path.display(), e))?;

        let config: Config = toml::from_str(&contents)
            .map_err(|e| anyhow::anyhow!("Failed to parse config file '{}': {}", path.display(), e))?;

        Ok(config)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_config() {
        let config = Config::default();
        assert_eq!(config.server.port, 3000);
        assert_eq!(config.server.bind_ip, IpAddr::V4(std::net::Ipv4Addr::new(0, 0, 0, 0)));
        assert_eq!(config.logging.level, LogLevel::Info);
        assert_eq!(config.logging.format, LogFormat::Compact);
        assert_eq!(config.logging.output, LogOutput::Stdout);
    }

    #[test]
    fn test_parse_full_config() {
        let toml_str = r#"
[logging]
level = "debug"
output = "/var/log/outlier.log"
format = "json"

[server]
port = 8080
bind_ip = "127.0.0.1"
"#;
        let config: Config = toml::from_str(toml_str).unwrap();
        assert_eq!(config.server.port, 8080);
        assert_eq!(config.server.bind_ip, IpAddr::V4(std::net::Ipv4Addr::new(127, 0, 0, 1)));
        assert_eq!(config.logging.level, LogLevel::Debug);
        assert_eq!(config.logging.format, LogFormat::Json);
        assert!(matches!(config.logging.output, LogOutput::File(_)));
    }

    #[test]
    fn test_parse_partial_config() {
        let toml_str = r#"
[server]
port = 9000
"#;
        let config: Config = toml::from_str(toml_str).unwrap();
        assert_eq!(config.server.port, 9000);
        // Defaults should be applied
        assert_eq!(config.server.bind_ip, IpAddr::V4(std::net::Ipv4Addr::new(0, 0, 0, 0)));
        assert_eq!(config.logging.level, LogLevel::Info);
    }

    #[test]
    fn test_log_level_display() {
        assert_eq!(LogLevel::Trace.to_string(), "trace");
        assert_eq!(LogLevel::Debug.to_string(), "debug");
        assert_eq!(LogLevel::Info.to_string(), "info");
        assert_eq!(LogLevel::Warn.to_string(), "warn");
        assert_eq!(LogLevel::Error.to_string(), "error");
    }
}
