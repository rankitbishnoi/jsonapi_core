use std::sync::Arc;

use axum::extract::FromRef;
use sqlx::SqlitePool;
use sqlx::sqlite::SqlitePoolOptions;

use jsonapi_axum::{BaseUrl, TypeRegistry};

use crate::config::Config;

static MIGRATOR: sqlx::migrate::Migrator = sqlx::migrate!("./migrations");

/// Shared application state threaded through the router.
#[derive(Clone)]
pub struct AppState {
    pub pool: SqlitePool,
    pub base_url: BaseUrl,
    pub type_registry: Arc<TypeRegistry>,
}

impl AppState {
    async fn init(pool: SqlitePool, base_url: impl Into<String>) -> anyhow::Result<Self> {
        MIGRATOR.run(&pool).await?;
        Ok(Self {
            pool,
            base_url: BaseUrl(base_url.into()),
            type_registry: Arc::new(TypeRegistry::new()),
        })
    }

    /// Connect to an in-memory SQLite database. Used in tests.
    ///
    /// A single connection is required so the `:memory:` DB persists across queries.
    pub async fn in_memory() -> anyhow::Result<Self> {
        let pool = SqlitePoolOptions::new()
            .max_connections(1)
            .connect("sqlite::memory:")
            .await?;
        Self::init(pool, "http://api.test").await
    }

    /// Build state from a [`Config`].
    pub async fn from_config(config: &Config) -> anyhow::Result<Self> {
        let pool = SqlitePoolOptions::new()
            .max_connections(5)
            .connect(&config.database_url)
            .await?;
        let state = Self::init(pool, &config.base_url).await?;
        if config.seed {
            crate::repo::seed::seed(&state.pool).await?;
        }
        Ok(state)
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
