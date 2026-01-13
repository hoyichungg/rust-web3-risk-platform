use axum::{
    Json, Router,
    extract::{Path, Query, State},
    http::StatusCode,
    routing::get,
};
use chrono::Utc;
use domain::{PortfolioSnapshot, Position};
use uuid::Uuid;

use crate::{auth_middleware::CurrentUser, state::AppState};

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/portfolio/:wallet_id", get(get_portfolio))
        .route("/portfolio/:wallet_id/history", get(get_portfolio_history))
        .route(
            "/portfolio/:wallet_id/snapshots",
            get(get_portfolio_snapshots),
        )
}

async fn get_portfolio(
    State(state): State<AppState>,
    user: CurrentUser,
    Path(wallet_id): Path<Uuid>,
) -> Result<Json<PortfolioSnapshot>, StatusCode> {
    let wallet = state
        .wallet_repo
        .find_by_id(wallet_id)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    let Some(wallet) = wallet else {
        return Err(StatusCode::NOT_FOUND);
    };

    if wallet.user_id != user.claims().user_id {
        return Err(StatusCode::FORBIDDEN);
    }

    let snapshot = state
        .portfolio_repo
        .latest_by_wallet(wallet_id)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?
        .unwrap_or_else(|| PortfolioSnapshot {
            wallet_id,
            total_usd_value: 0.0,
            timestamp: Utc::now(),
            positions: vec![Position {
                asset_symbol: "ETH".to_string(),
                amount: 0.0,
                usd_value: 0.0,
            }],
        });

    Ok(Json(snapshot))
}

#[derive(Debug, serde::Deserialize)]
struct HistoryQuery {
    limit: Option<i64>,
}

async fn get_portfolio_history(
    State(state): State<AppState>,
    user: CurrentUser,
    Path(wallet_id): Path<Uuid>,
    Query(params): Query<HistoryQuery>,
) -> Result<Json<Vec<PortfolioSnapshot>>, StatusCode> {
    let wallet = state
        .wallet_repo
        .find_by_id(wallet_id)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    let Some(wallet) = wallet else {
        return Err(StatusCode::NOT_FOUND);
    };
    if wallet.user_id != user.claims().user_id {
        return Err(StatusCode::FORBIDDEN);
    }

    let limit = params.limit.unwrap_or(50).clamp(1, 500);
    let history = state
        .portfolio_repo
        .history_by_wallet(wallet_id, limit)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    Ok(Json(history))
}

#[derive(Debug, serde::Deserialize)]
struct SnapshotQuery {
    days: Option<i64>,
}

async fn get_portfolio_snapshots(
    State(state): State<AppState>,
    user: CurrentUser,
    Path(wallet_id): Path<Uuid>,
    Query(params): Query<SnapshotQuery>,
) -> Result<Json<Vec<PortfolioSnapshot>>, StatusCode> {
    let wallet = state
        .wallet_repo
        .find_by_id(wallet_id)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    let Some(wallet) = wallet else {
        return Err(StatusCode::NOT_FOUND);
    };
    if wallet.user_id != user.claims().user_id {
        return Err(StatusCode::FORBIDDEN);
    }

    let days = params.days.unwrap_or(7).clamp(1, 30);
    let since = chrono::Utc::now() - chrono::Duration::days(days);
    let history = state
        .portfolio_repo
        .history_since(wallet_id, since)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    Ok(Json(history))
}
