use std::time::Duration;

use api::{bootstrap::build_state, config::AppConfig, telemetry};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    dotenvy::dotenv().ok();
    telemetry::init_tracing()?;

    // 確保 worker 模式會啟用 alert evaluator
    let mut config = AppConfig::from_env()?;
    config.enable_alert_worker = true;
    let _state = build_state(&config).await?;
    tracing::info!("alert worker started; polling every 60s");

    // Keep process alive; background tasks在 build_state 內啟動
    loop {
        tokio::time::sleep(Duration::from_secs(3600)).await;
    }
}
