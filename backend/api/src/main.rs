use std::net::SocketAddr;

use api::{app::build_router, bootstrap::build_state, config::AppConfig, telemetry};
use axum::{http::HeaderValue, routing::Router};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    dotenvy::dotenv().ok();
    telemetry::init_tracing()?;

    let config = AppConfig::from_env()?;
    let allowed_origins = config
        .frontend_origins
        .iter()
        .map(|value| {
            HeaderValue::from_str(value)
                .map_err(|err| anyhow::anyhow!("invalid FRONTEND_ORIGINS entry {value}: {err}"))
        })
        .collect::<Result<Vec<_>, _>>()?;
    let state = build_state(&config).await?;

    let app: Router = build_router(state.clone(), allowed_origins);

    let addr = SocketAddr::from(([0, 0, 0, 0], config.port));
    tracing::info!(%addr, "listening on address");
    let listener = tokio::net::TcpListener::bind(addr).await?;
    let app = app.into_make_service_with_connect_info::<SocketAddr>();
    axum::serve(listener, app).await?;

    Ok(())
}
