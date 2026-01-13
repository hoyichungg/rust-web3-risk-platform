use axum::Router;
use axum::routing::get;

use crate::state::AppState;

pub fn router() -> Router<AppState> {
    Router::new().route("/healthz", get(healthz))
}

async fn healthz() -> &'static str {
    "ok"
}
