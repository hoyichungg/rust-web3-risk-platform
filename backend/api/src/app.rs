use axum::{
    extract::State,
    http::{
        header::{AUTHORIZATION, CONTENT_TYPE},
        HeaderValue, Method, Request,
    },
    response::IntoResponse,
    routing::get,
    Json, Router,
};
use metrics::set_global_recorder;
use metrics_exporter_prometheus::{PrometheusBuilder, PrometheusHandle};
use std::sync::OnceLock;
use tower_http::{
    cors::{AllowHeaders, AllowMethods, AllowOrigin, CorsLayer},
    request_id::{MakeRequestUuid, PropagateRequestIdLayer, SetRequestIdLayer},
    trace::{DefaultOnRequest, DefaultOnResponse, TraceLayer},
    LatencyUnit,
};
use tracing::{info_span, Level};

use crate::{
    config::Erc20TokenConfig,
    routes::{
        alerts as alert_routes, auth as auth_routes, health, portfolio as portfolio_routes, secure,
        strategies as strategy_routes, wallets as wallet_routes,
    },
    state::AppState,
};

static PROMETHEUS_HANDLE: OnceLock<PrometheusHandle> = OnceLock::new();

fn get_prometheus_handle() -> PrometheusHandle {
    PROMETHEUS_HANDLE
        .get_or_init(|| {
            let builder = PrometheusBuilder::new();
            let recorder = builder.build_recorder();
            let handle = recorder.handle();
            if let Err(e) = set_global_recorder(recorder) {
                tracing::warn!("Global metrics recorder already installed: {}", e);
            }
            handle
        })
        .clone()
}

async fn metrics_handler() -> impl IntoResponse {
    get_prometheus_handle().render()
}

async fn get_public_tokens(State(state): State<AppState>) -> Json<Vec<Erc20TokenConfig>> {
    Json(state.config.erc20_tokens.clone())
}

pub fn build_router(state: AppState, allowed_origins: Vec<HeaderValue>) -> Router {
    let cors = CorsLayer::new()
        .allow_methods(AllowMethods::list([
            Method::GET,
            Method::POST,
            Method::DELETE,
            Method::PUT,
            Method::OPTIONS,
        ]))
        .allow_headers(AllowHeaders::list(vec![AUTHORIZATION, CONTENT_TYPE]))
        .allow_origin(AllowOrigin::list(allowed_origins))
        .allow_credentials(true);

    let request_id_header = axum::http::header::HeaderName::from_static("x-request-id");
    let request_id_for_span = request_id_header.clone();
    let trace_layer = TraceLayer::new_for_http()
        .make_span_with(move |request: &Request<_>| {
            let request_id = request
                .headers()
                .get(&request_id_for_span)
                .and_then(|value| value.to_str().ok())
                .unwrap_or("unknown");
            info_span!(
                "http_request",
                method = %request.method(),
                uri = %request.uri(),
                request_id = %request_id
            )
        })
        .on_request(DefaultOnRequest::new().level(Level::INFO))
        .on_response(
            DefaultOnResponse::new()
                .level(Level::INFO)
                .latency_unit(LatencyUnit::Millis),
        );

    Router::<AppState>::new()
        .route("/", get(|| async { "rust-web3-risk-platform backend" }))
        .route("/metrics", get(metrics_handler))
        .merge(health::router())
        .nest(
            "/api",
            Router::new()
                .merge(auth_routes::router())
                .merge(wallet_routes::router())
                .merge(portfolio_routes::router())
                .merge(strategy_routes::router())
                .merge(alert_routes::router())
                .merge(secure::router())
                .route("/config/tokens", get(get_public_tokens)),
        )
        .with_state(state)
        .layer(cors)
        .layer(PropagateRequestIdLayer::new(request_id_header.clone()))
        .layer(trace_layer)
        .layer(SetRequestIdLayer::new(
            request_id_header,
            MakeRequestUuid::default(),
        ))
}
