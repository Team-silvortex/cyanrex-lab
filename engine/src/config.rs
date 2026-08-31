#[derive(Debug, Clone)]
pub struct AppConfig {
    pub host: String,
    pub port: u16,
    pub database_url: String,
    pub instance_id: String,
}

impl AppConfig {
    pub fn from_env() -> Self {
        let host = std::env::var("ENGINE_HOST").unwrap_or_else(|_| "0.0.0.0".to_string());
        let port = std::env::var("ENGINE_PORT")
            .ok()
            .and_then(|v| v.parse::<u16>().ok())
            .unwrap_or(8080);
        let database_url = std::env::var("DATABASE_URL")
            .unwrap_or_else(|_| "postgres://postgres:postgres@localhost:5432/cyanrex".to_string());

        Self {
            host,
            port,
            database_url,
            instance_id: runtime_instance_id(),
        }
    }
}

pub fn runtime_instance_id() -> String {
    let raw = std::env::var("CYANREX_INSTANCE_ID")
        .ok()
        .unwrap_or_else(|| "default".to_string());
    sanitize_instance_id(&raw, "default")
}

pub fn db_fallback_enabled() -> bool {
    std::env::var("CYANREX_DB_FALLBACK")
        .map(|value| matches!(value.to_lowercase().as_str(), "1" | "true" | "on" | "yes"))
        .unwrap_or(true)
}

fn sanitize_instance_id(raw: &str, fallback: &str) -> String {
    let cleaned: String = raw
        .chars()
        .filter(|c| c.is_ascii_alphanumeric() || *c == '_' || *c == '-')
        .collect();
    if cleaned.is_empty() {
        return fallback.to_string();
    }
    let truncated = if cleaned.len() > 64 {
        cleaned[..64].to_string()
    } else {
        cleaned
    };
    if truncated.is_empty() {
        fallback.to_string()
    } else {
        truncated
    }
}
