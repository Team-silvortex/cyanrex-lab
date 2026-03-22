#[derive(Debug, Clone)]
pub struct AppConfig {
    pub host: String,
    pub port: u16,
    pub database_url: String,
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
        }
    }
}
