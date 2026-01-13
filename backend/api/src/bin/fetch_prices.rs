use std::time::Duration;

use api::{config::AppConfig, services::history::load_prices_from_history, telemetry};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    dotenvy::dotenv().ok();
    telemetry::init_tracing()?;
    let config = AppConfig::from_env()?;
    let symbols = std::env::var("PRICE_SYMBOLS").unwrap_or_else(|_| "ETH,WETH,USDC".to_string());
    let days: u32 = std::env::var("PRICE_DAYS")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(30);

    let state = api::bootstrap::build_state(&config).await?;
    let symbols: Vec<String> = symbols
        .split(',')
        .filter_map(|s| {
            let trimmed = s.trim();
            if trimmed.is_empty() {
                None
            } else {
                Some(trimmed.to_uppercase())
            }
        })
        .collect();

    for symbol in symbols {
        match load_prices_from_history(&state, &symbol, days).await {
            Ok(points) => {
                tracing::info!(%symbol, days, points = points.len(), "price history synced");
                // 小睡 1 秒避免踩到免費 API rate limit。
                tokio::time::sleep(Duration::from_secs(1)).await;
            }
            Err(err) => {
                tracing::warn!(%symbol, %err, "price history fetch failed");
            }
        }
    }

    Ok(())
}
