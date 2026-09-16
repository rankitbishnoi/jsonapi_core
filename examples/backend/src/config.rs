/// Application configuration, read from environment variables.
#[derive(Debug, Clone)]
pub struct Config {
    pub bind: String,
    pub database_url: String,
    pub base_url: String,
    pub cors_origins: Vec<String>,
    pub seed: bool,
}

impl Config {
    /// Build a [`Config`] from environment variables, falling back to defaults.
    pub fn from_env() -> Self {
        let bind = std::env::var("APP_BIND").unwrap_or_else(|_| "127.0.0.1:8080".to_string());
        let database_url = std::env::var("DATABASE_URL")
            .unwrap_or_else(|_| "sqlite://showcase.db?mode=rwc".to_string());
        let base_url = std::env::var("APP_BASE_URL")
            .unwrap_or_else(|_| format!("http://{bind}"));
        let cors_origins = std::env::var("APP_CORS_ORIGINS")
            .unwrap_or_else(|_| "http://127.0.0.1:8081".to_string())
            .split(',')
            .map(|s| s.trim().to_string())
            .collect();
        let seed = std::env::var("APP_SEED")
            .map(|v| v != "false" && v != "0")
            .unwrap_or(true);
        Self {
            bind,
            database_url,
            base_url,
            cors_origins,
            seed,
        }
    }
}
