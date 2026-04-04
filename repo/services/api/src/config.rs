use serde::Deserialize;
use std::fs;

#[derive(Debug, Clone, Deserialize)]
pub struct AppConfig {
    pub server: ServerConfig,
    pub database: DatabaseConfig,
    pub security: SecurityConfig,
    pub session: SessionConfig,
    pub auth_policy: AuthPolicyConfig,
    pub retention: RetentionConfig,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ServerConfig {
    pub host: String,
    pub port: u16,
    pub allowed_origins: Vec<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct DatabaseConfig {
    pub url: String,
    pub max_connections: u32,
    pub min_connections: u32,
    pub connect_timeout_secs: u64,
}

#[derive(Debug, Clone, Deserialize)]
pub struct SecurityConfig {
    pub key_bootstrap: KeyBootstrapConfig,
}

#[derive(Debug, Clone, Deserialize)]
pub struct KeyBootstrapConfig {
    pub strategy: String,
    pub key_file: String,
    pub key_length: usize,
}

#[derive(Debug, Clone, Deserialize)]
pub struct SessionConfig {
    pub cookie_name: String,
    pub secure: bool,
    pub http_only: bool,
    pub same_site: String,
    pub ttl_minutes: u32,
}

#[derive(Debug, Clone, Deserialize)]
pub struct RetentionConfig {
    pub audit_log_days: u32,
    pub session_days: u32,
    pub patient_record_days: u32,
    pub clinical_years_min: u32,
}

#[derive(Debug, Clone, Deserialize)]
pub struct AuthPolicyConfig {
    pub password_min_length: u32,
    pub lockout_failed_attempts: u32,
    pub lockout_minutes: u32,
    pub session_inactivity_minutes: u32,
    pub offline_only: bool,
}

impl AppConfig {
    pub fn load(path: &str) -> Result<Self, Box<dyn std::error::Error>> {
        let content = fs::read_to_string(path)?;
        let mut cfg: Self = toml::from_str(&content)?;
        if let Ok(url) = std::env::var("DATABASE_URL") {
            cfg.database.url = url;
        }
        Ok(cfg)
    }
}
