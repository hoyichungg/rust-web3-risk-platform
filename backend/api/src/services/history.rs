use anyhow::Result;
use chrono::{Duration as ChronoDuration, Utc};
use domain::PriceHistoryPoint;
use std::time::Duration;
use strategy_engine::PricePoint;
use uuid::Uuid;

use crate::{config::AppConfig, services::CoingeckoPriceOracle, state::AppState};

pub async fn load_prices_from_history(
    state: &AppState,
    symbol: &str,
    days: u32,
) -> Result<Vec<PricePoint>> {
    let to = Utc::now();
    let from = to - ChronoDuration::days(days as i64);
    let symbol_upper = symbol.to_uppercase();

    // Try cache first; ensure coverage from 'from' to 'to'
    let cached = state
        .price_history_repo
        .fetch_range(&symbol_upper, None, from, to)
        .await?;
    if !cached.is_empty() {
        let first_ts = cached.first().map(|p| p.price_ts).unwrap_or(from);
        let last_ts = cached.last().map(|p| p.price_ts).unwrap_or(from);
        let covered = first_ts <= from && last_ts >= to - ChronoDuration::hours(1);
        if covered {
            return Ok(cached
                .into_iter()
                .map(|p| PricePoint {
                    timestamp: p.price_ts,
                    price: p.price,
                })
                .collect());
        }
    }

    // Fallback to live fetch then persist
    let fetched = fetch_coingecko_history(symbol_upper.clone(), days, &state.config).await?;
    let points: Vec<PriceHistoryPoint> = fetched
        .iter()
        .map(|p| PriceHistoryPoint {
            id: Uuid::new_v4(),
            symbol: symbol_upper.clone(),
            price: p.price,
            price_ts: p.timestamp,
            source: "coingecko".to_string(),
            chain_id: None,
        })
        .collect();
    state.price_history_repo.upsert_points(&points).await?;
    Ok(fetched)
}

async fn fetch_coingecko_history(
    symbol: String,
    days: u32,
    config: &AppConfig,
) -> Result<Vec<PricePoint>> {
    let oracle = CoingeckoPriceOracle::new(
        config.coingecko_api_base.clone(),
        config.token_price_ids.clone(),
        Duration::from_secs(60),
    );
    oracle.fetch_market_chart(&symbol, days).await
}
