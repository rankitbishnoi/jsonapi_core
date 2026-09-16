use std::sync::Arc;

use axum::extract::FromRef;
use sqlx::SqlitePool;

use jsonapi_axum::{BaseUrl, TypeRegistry};

use crate::config::Config;

/// Shared application state threaded through the router.
#[derive(Clone)]
pub struct AppState {
    pub pool: SqlitePool,
    pub base_url: BaseUrl,
    pub type_registry: Arc<TypeRegistry>,
}

impl AppState {
    /// Connect to an in-memory SQLite database. Used in tests.
    pub async fn in_memory() -> anyhow::Result<Self> {
        let pool = SqlitePool::connect("sqlite::memory:").await?;
        Ok(Self {
            pool,
            base_url: BaseUrl("http://api.test".to_string()),
            type_registry: Arc::new(TypeRegistry::new()),
        })
    }

    /// Build state from a [`Config`]. For Task 1, delegates to in-memory.
    pub async fn from_config(_config: &Config) -> anyhow::Result<Self> {
        Self::in_memory().await
    }
}

impl FromRef<AppState> for BaseUrl {
    fn from_ref(state: &AppState) -> BaseUrl {
        state.base_url.clone()
    }
}

impl FromRef<AppState> for Arc<TypeRegistry> {
    fn from_ref(state: &AppState) -> Arc<TypeRegistry> {
        state.type_registry.clone()
    }
}
