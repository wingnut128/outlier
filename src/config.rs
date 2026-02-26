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

/// Authentication mode
#[derive(Debug, Clone, Deserialize, Default, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum AuthMode {
    #[default]
    ApiKey,
    Jwt,
    Both,
}

/// JWT-specific configuration
#[derive(Debug, Clone, Deserialize)]
pub struct JwtConfig {
    #[serde(default)]
    pub issuer: String,
    #[serde(default)]
    pub audience: String,
    #[serde(default)]
    pub jwks_url: Option<String>,
    #[serde(default = "default_jwks_cache_ttl")]
    pub jwks_cache_ttl_secs: u64,
    #[serde(default = "default_algorithms")]
    pub algorithms: Vec<String>,
}

impl Default for JwtConfig {
    fn default() -> Self {
        Self {
            issuer: String::new(),
            audience: String::new(),
            jwks_url: None,
            jwks_cache_ttl_secs: default_jwks_cache_ttl(),
            algorithms: default_algorithms(),
        }
    }
}

fn default_jwks_cache_ttl() -> u64 {
    3600
}

fn default_algorithms() -> Vec<String> {
    vec!["RS256".to_string()]
}

/// Authentication configuration section
#[derive(Debug, Clone, Deserialize, Default)]
pub struct AuthConfig {
    #[serde(default)]
    pub enabled: bool,
    #[serde(default)]
    pub mode: AuthMode,
    #[serde(default)]
    pub api_keys: Vec<String>,
    #[serde(default)]
    pub jwt: JwtConfig,
}

/// Rate limiting configuration section
#[derive(Debug, Clone, Deserialize)]
pub struct RateLimitConfig {
    #[serde(default)]
    pub enabled: bool,
    #[serde(default = "default_per_ip_per_second")]
    pub per_ip_per_second: u32,
    #[serde(default = "default_per_ip_burst")]
    pub per_ip_burst: u32,
    #[serde(default = "default_global_per_second")]
    pub global_per_second: u32,
    #[serde(default = "default_global_burst")]
    pub global_burst: u32,
}

fn default_per_ip_per_second() -> u32 {
    10
}

fn default_per_ip_burst() -> u32 {
    20
}

fn default_global_per_second() -> u32 {
    100
}

fn default_global_burst() -> u32 {
    200
}

impl Default for RateLimitConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            per_ip_per_second: default_per_ip_per_second(),
            per_ip_burst: default_per_ip_burst(),
            global_per_second: default_global_per_second(),
            global_burst: default_global_burst(),
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
    #[serde(default)]
    pub auth: AuthConfig,
    #[serde(default)]
    pub rate_limit: RateLimitConfig,
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
        let contents = std::fs::read_to_string(path).map_err(|e| {
            anyhow::anyhow!("Failed to read config file '{}': {}", path.display(), e)
        })?;

        let config: Config = toml::from_str(&contents).map_err(|e| {
            anyhow::anyhow!("Failed to parse config file '{}': {}", path.display(), e)
        })?;

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
        assert_eq!(
            config.server.bind_ip,
            IpAddr::V4(std::net::Ipv4Addr::new(0, 0, 0, 0))
        );
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
        assert_eq!(
            config.server.bind_ip,
            IpAddr::V4(std::net::Ipv4Addr::new(127, 0, 0, 1))
        );
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
        assert_eq!(
            config.server.bind_ip,
            IpAddr::V4(std::net::Ipv4Addr::new(0, 0, 0, 0))
        );
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

    #[test]
    fn test_default_auth_config() {
        let config = AuthConfig::default();
        assert!(!config.enabled);
        assert_eq!(config.mode, AuthMode::ApiKey);
        assert!(config.api_keys.is_empty());
        assert!(config.jwt.issuer.is_empty());
        assert!(config.jwt.audience.is_empty());
        assert_eq!(config.jwt.jwks_cache_ttl_secs, 3600);
        assert_eq!(config.jwt.algorithms, vec!["RS256"]);
    }

    #[test]
    fn test_default_rate_limit_config() {
        let config = RateLimitConfig::default();
        assert!(!config.enabled);
        assert_eq!(config.per_ip_per_second, 10);
        assert_eq!(config.per_ip_burst, 20);
        assert_eq!(config.global_per_second, 100);
        assert_eq!(config.global_burst, 200);
    }

    #[test]
    fn test_parse_auth_config() {
        let toml_str = r#"
[auth]
enabled = true
api_keys = ["key1", "key2"]
"#;
        let config: Config = toml::from_str(toml_str).unwrap();
        assert!(config.auth.enabled);
        assert_eq!(config.auth.api_keys, vec!["key1", "key2"]);
    }

    #[test]
    fn test_parse_rate_limit_config() {
        let toml_str = r#"
[rate_limit]
enabled = true
per_ip_per_second = 5
per_ip_burst = 10
global_per_second = 50
global_burst = 100
"#;
        let config: Config = toml::from_str(toml_str).unwrap();
        assert!(config.rate_limit.enabled);
        assert_eq!(config.rate_limit.per_ip_per_second, 5);
        assert_eq!(config.rate_limit.per_ip_burst, 10);
        assert_eq!(config.rate_limit.global_per_second, 50);
        assert_eq!(config.rate_limit.global_burst, 100);
    }

    #[test]
    fn test_parse_partial_auth_config_defaults() {
        let toml_str = r#"
[auth]
enabled = true
"#;
        let config: Config = toml::from_str(toml_str).unwrap();
        assert!(config.auth.enabled);
        assert!(config.auth.api_keys.is_empty());
    }

    #[test]
    fn test_config_without_auth_or_rate_limit_uses_defaults() {
        let toml_str = r#"
[server]
port = 3000
"#;
        let config: Config = toml::from_str(toml_str).unwrap();
        assert!(!config.auth.enabled);
        assert!(!config.rate_limit.enabled);
    }

    #[test]
    fn test_parse_jwt_config() {
        let toml_str = r#"
[auth]
enabled = true
mode = "jwt"

[auth.jwt]
issuer = "https://example.auth0.com/"
audience = "https://api.outlier.dev"
jwks_cache_ttl_secs = 1800
algorithms = ["RS256", "RS384"]
"#;
        let config: Config = toml::from_str(toml_str).unwrap();
        assert!(config.auth.enabled);
        assert_eq!(config.auth.mode, AuthMode::Jwt);
        assert_eq!(config.auth.jwt.issuer, "https://example.auth0.com/");
        assert_eq!(config.auth.jwt.audience, "https://api.outlier.dev");
        assert_eq!(config.auth.jwt.jwks_cache_ttl_secs, 1800);
        assert_eq!(config.auth.jwt.algorithms, vec!["RS256", "RS384"]);
    }

    #[test]
    fn test_parse_both_mode() {
        let toml_str = r#"
[auth]
enabled = true
mode = "both"
api_keys = ["key1"]

[auth.jwt]
issuer = "https://example.auth0.com/"
audience = "https://api.outlier.dev"
"#;
        let config: Config = toml::from_str(toml_str).unwrap();
        assert_eq!(config.auth.mode, AuthMode::Both);
        assert_eq!(config.auth.api_keys, vec!["key1"]);
        assert!(!config.auth.jwt.issuer.is_empty());
    }

    #[test]
    fn test_default_auth_mode_is_api_key() {
        let toml_str = r#"
[auth]
enabled = true
api_keys = ["key1"]
"#;
        let config: Config = toml::from_str(toml_str).unwrap();
        assert_eq!(config.auth.mode, AuthMode::ApiKey);
    }

    #[test]
    fn test_jwt_config_with_jwks_url_override() {
        let toml_str = r#"
[auth]
enabled = true
mode = "jwt"

[auth.jwt]
issuer = "https://example.auth0.com/"
audience = "https://api.outlier.dev"
jwks_url = "https://custom.example.com/keys"
"#;
        let config: Config = toml::from_str(toml_str).unwrap();
        assert_eq!(
            config.auth.jwt.jwks_url.as_deref(),
            Some("https://custom.example.com/keys")
        );
    }
}
