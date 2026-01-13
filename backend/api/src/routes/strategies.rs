use axum::{
    Json, Router,
    extract::{Path, Query, State},
    http::StatusCode,
    routing::{delete, get, post},
};
use chrono::Utc;
use domain::{BacktestResult, Strategy};
use serde::Deserialize;
use uuid::Uuid;

use crate::{auth_middleware::CurrentUser, state::AppState};
use strategy_engine::PricePoint;
use rand::Rng;

use crate::services::history::load_prices_from_history;

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/strategies", get(list_strategies).post(create_strategy))
        .route("/strategies/:strategy_id/backtest", post(run_backtest))
        .route("/strategies/:strategy_id/backtests", get(list_backtests))
        .route("/strategies/:strategy_id", delete(delete_strategy))
}

#[derive(Debug, Deserialize)]
struct CreateStrategyRequest {
    name: String,
    #[serde(rename = "type")]
    r#type: String,
    params: serde_json::Value,
}

async fn create_strategy(
    State(state): State<AppState>,
    user: CurrentUser,
    Json(payload): Json<CreateStrategyRequest>,
) -> Result<Json<Strategy>, StatusCode> {
    let strategy = Strategy {
        id: Uuid::new_v4(),
        user_id: user.claims().user_id,
        name: payload.name,
        r#type: payload.r#type,
        params: payload.params,
    };
    state
        .strategy_repo
        .create(&strategy)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    Ok(Json(strategy))
}

async fn list_strategies(
    State(state): State<AppState>,
    user: CurrentUser,
) -> Result<Json<Vec<Strategy>>, StatusCode> {
    state
        .strategy_repo
        .list_by_user(user.claims().user_id)
        .await
        .map(Json)
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)
}

#[derive(Debug, Deserialize)]
struct BacktestPayload {
    prices: Option<Vec<PriceInput>>,
    short_window: Option<usize>,
    long_window: Option<usize>,
    symbol: Option<String>,
    days: Option<u32>,
}

#[derive(Debug, Deserialize)]
struct PriceInput {
    timestamp: chrono::DateTime<Utc>,
    price: f64,
}

async fn run_backtest(
    State(state): State<AppState>,
    user: CurrentUser,
    Path(strategy_id): Path<Uuid>,
    Json(payload): Json<BacktestPayload>,
) -> Result<Json<BacktestResult>, StatusCode> {
    let Some(mut strategy) = state
        .strategy_repo
        .find_by_id(strategy_id, user.claims().user_id)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?
    else {
        return Err(StatusCode::NOT_FOUND);
    };

    if let Some(short) = payload.short_window {
        strategy.params["short_window"] = serde_json::json!(short);
    }
    if let Some(long) = payload.long_window {
        strategy.params["long_window"] = serde_json::json!(long);
    }

    let prices: Vec<PricePoint> = if let Some(points) = payload.prices {
        points
            .into_iter()
            .map(|p| PricePoint {
                timestamp: p.timestamp,
                price: p.price,
            })
            .collect()
    } else {
        let symbol = payload.symbol.as_deref().unwrap_or("ETH");
        let days = payload.days.unwrap_or(30);
        match load_prices_from_history(&state, symbol, days).await {
            Ok(points) if !points.is_empty() => points,
            Ok(_) => synthetic_prices(days),
            Err(err) => {
                tracing::warn!(%err, %symbol, days, "price history load failed, fallback to synthetic");
                synthetic_prices(days)
            }
        }
    };

    let mut result = state.strategy.backtest(strategy.clone(), prices).await;
    if result.completed_at.is_none() {
        result.completed_at = Some(Utc::now());
    }
    state
        .strategy_repo
        .save_backtest(&result)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    Ok(Json(result))
}

fn synthetic_prices(days: u32) -> Vec<PricePoint> {
    let days = days.max(7);
    let mut rng = rand::thread_rng();
    let mut price = 100.0;
    let now = Utc::now();
    let mut points = Vec::with_capacity(days as usize);
    for i in (0..days).rev() {
        let ts = now - chrono::Duration::days(i.into());
        let drift = 0.0015;
        let noise: f64 = rng.gen_range(-0.01..0.01);
        price *= 1.0 + drift + noise;
        points.push(PricePoint {
            timestamp: ts,
            price: (price * 100.0).round() / 100.0,
        });
    }
    points
}

#[derive(Debug, serde::Deserialize)]
struct BacktestQuery {
    limit: Option<i64>,
}

async fn list_backtests(
    State(state): State<AppState>,
    user: CurrentUser,
    Path(strategy_id): Path<Uuid>,
    Query(params): Query<BacktestQuery>,
) -> Result<Json<Vec<BacktestResult>>, StatusCode> {
    let strategy = state
        .strategy_repo
        .find_by_id(strategy_id, user.claims().user_id)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    if strategy.is_none() {
        return Err(StatusCode::NOT_FOUND);
    }

    state
        .strategy_repo
        .list_backtests(
            strategy_id,
            user.claims().user_id,
            params.limit.unwrap_or(5).clamp(1, 20) as usize,
        )
        .await
        .map(Json)
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)
}

async fn delete_strategy(
    State(state): State<AppState>,
    user: CurrentUser,
    Path(strategy_id): Path<Uuid>,
) -> Result<StatusCode, StatusCode> {
    let deleted = state
        .strategy_repo
        .delete(strategy_id, user.claims().user_id)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    if deleted {
        Ok(StatusCode::NO_CONTENT)
    } else {
        Err(StatusCode::NOT_FOUND)
    }
}
