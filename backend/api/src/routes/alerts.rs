use axum::{
    Json, Router,
    extract::{Path, State},
    http::StatusCode,
    routing::{get, post, put},
};
use domain::AlertRule;
use serde::Deserialize;
use uuid::Uuid;

use crate::{auth_middleware::CurrentUser, state::AppState};

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/alerts", get(list_alerts).post(create_alert))
        .route("/alerts/:alert_id", put(update_alert).delete(delete_alert))
        .route("/alerts/:alert_id/test", post(simulate_trigger))
        .route("/alerts/triggers", get(list_triggers))
}

#[derive(Debug, Deserialize)]
struct AlertPayload {
    #[serde(rename = "type")]
    r#type: String,
    threshold: f64,
    enabled: Option<bool>,
    cooldown_secs: Option<i64>,
}

async fn list_alerts(
    State(state): State<AppState>,
    user: CurrentUser,
) -> Result<Json<Vec<AlertRule>>, StatusCode> {
    state
        .alert_repo
        .list_rules(user.claims().user_id)
        .await
        .map(Json)
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)
}

async fn create_alert(
    State(state): State<AppState>,
    user: CurrentUser,
    Json(payload): Json<AlertPayload>,
) -> Result<Json<AlertRule>, StatusCode> {
    let rule = AlertRule {
        id: Uuid::new_v4(),
        user_id: user.claims().user_id,
        r#type: payload.r#type,
        threshold: payload.threshold,
        enabled: payload.enabled.unwrap_or(true),
        cooldown_secs: payload.cooldown_secs.unwrap_or(300),
    };
    state
        .alert_repo
        .create_rule(&rule)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    Ok(Json(rule))
}

async fn update_alert(
    State(state): State<AppState>,
    user: CurrentUser,
    Path(alert_id): Path<Uuid>,
    Json(payload): Json<AlertPayload>,
) -> Result<StatusCode, StatusCode> {
    let rule = AlertRule {
        id: alert_id,
        user_id: user.claims().user_id,
        r#type: payload.r#type,
        threshold: payload.threshold,
        enabled: payload.enabled.unwrap_or(true),
        cooldown_secs: payload.cooldown_secs.unwrap_or(300),
    };
    let updated = state
        .alert_repo
        .update_rule(&rule)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    if updated {
        Ok(StatusCode::NO_CONTENT)
    } else {
        Err(StatusCode::NOT_FOUND)
    }
}

async fn delete_alert(
    State(state): State<AppState>,
    user: CurrentUser,
    Path(alert_id): Path<Uuid>,
) -> Result<StatusCode, StatusCode> {
    let deleted = state
        .alert_repo
        .delete_rule(alert_id, user.claims().user_id)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    if deleted {
        Ok(StatusCode::NO_CONTENT)
    } else {
        Err(StatusCode::NOT_FOUND)
    }
}

async fn simulate_trigger(
    State(state): State<AppState>,
    user: CurrentUser,
    Path(alert_id): Path<Uuid>,
) -> Result<StatusCode, StatusCode> {
    let wallets = state
        .wallet_repo
        .list_by_user(user.claims().user_id)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    let wallet = wallets.first().ok_or(StatusCode::BAD_REQUEST)?;

    state
        .alert_repo
        .insert_trigger(alert_id, wallet.id, "Test trigger from simulate endpoint")
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    Ok(StatusCode::CREATED)
}

async fn list_triggers(
    State(state): State<AppState>,
    user: CurrentUser,
) -> Result<Json<Vec<crate::repositories::AlertTrigger>>, StatusCode> {
    state
        .alert_repo
        .list_triggers(user.claims().user_id, 50)
        .await
        .map(Json)
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)
}
