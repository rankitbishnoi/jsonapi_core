use tokio::net::TcpListener;
use tracing_subscriber::EnvFilter;

use jsonapi_showcase_backend::config::Config;
use jsonapi_showcase_backend::router::build_router;
use jsonapi_showcase_backend::state::AppState;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::try_from_default_env().unwrap_or_else(|_| "info".into()))
        .init();

    let config = Config::from_env();
    let state = AppState::from_config(&config).await?;
    let app = build_router(state);

    let listener = TcpListener::bind(&config.bind).await?;
    tracing::info!("listening on {}", config.bind);

    axum::serve(listener, app)
        .with_graceful_shutdown(async {
            tokio::signal::ctrl_c()
                .await
                .expect("failed to listen for ctrl_c");
        })
        .await?;

    Ok(())
}
