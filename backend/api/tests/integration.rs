use std::{sync::Arc, time::Duration};

use alert_engine::InMemoryAlertService;
use api::{
    app::build_router,
    config::AppConfig,
    nonce_limiter::NonceLimiter,
    repositories::{
        PostgresAlertRepository, PostgresPortfolioSnapshotRepository, PostgresPriceCacheRepository,
        PostgresPriceHistoryRepository, PostgresSessionRepository, PostgresStrategyRepository,
        PostgresTransactionRepository, PostgresUserRepository, PostgresWalletRepository,
    },
    state::AppState,
};
use async_trait::async_trait;
use auth::{AuthError, AuthResult, AuthService, JwtClaims};
use axum::{
    body::{Body, to_bytes},
    http::{HeaderValue, Request, StatusCode},
};
use axum_extra::extract::cookie::SameSite;
use chrono::{Duration as ChronoDuration, Utc};
use domain::{LoginRequest, LoginResponse, NonceResponse, Role, Wallet};
use ethers::providers::{Http, Provider};
use indexer::InMemoryPortfolioService;
use sqlx::PgPool;
use strategy_engine::InMemoryStrategyService;
use tower::ServiceExt;
use uuid::Uuid;

#[derive(Clone)]
struct StubAuthService {
    claims: JwtClaims,
}

#[async_trait]
impl AuthService for StubAuthService {
    async fn issue_nonce(&self) -> AuthResult<NonceResponse> {
        Err(AuthError::InvalidMessage)
    }

    async fn login(&self, _payload: LoginRequest) -> AuthResult<LoginResponse> {
        Err(AuthError::InvalidSignature)
    }

    async fn validate_token(&self, token: &str) -> AuthResult<JwtClaims> {
        if token == "test-token" {
            Ok(self.claims.clone())
        } else {
            Err(AuthError::InvalidToken)
        }
    }

    async fn logout(&self, _session_id: Uuid) -> AuthResult<()> {
        Ok(())
    }

    async fn refresh_session(&self, _refresh_token: &str) -> AuthResult<LoginResponse> {
        Err(AuthError::InvalidToken)
    }

    async fn link_wallet(&self, _user_id: Uuid, _payload: LoginRequest) -> AuthResult<Wallet> {
        Err(AuthError::InvalidSignature)
    }

    async fn refresh_role_cache(&self, _address: &str, _chain_id: u64) -> AuthResult<Role> {
        Ok(Role::Viewer)
    }
}

#[derive(Clone)]
struct SingleTokenAuthService {
    token: String,
    claims: JwtClaims,
}

#[async_trait]
impl AuthService for SingleTokenAuthService {
    async fn issue_nonce(&self) -> AuthResult<NonceResponse> {
        Err(AuthError::InvalidMessage)
    }

    async fn login(&self, _payload: LoginRequest) -> AuthResult<LoginResponse> {
        Err(AuthError::InvalidSignature)
    }

    async fn validate_token(&self, token: &str) -> AuthResult<JwtClaims> {
        if token == self.token {
            Ok(self.claims.clone())
        } else {
            Err(AuthError::InvalidToken)
        }
    }

    async fn logout(&self, _session_id: Uuid) -> AuthResult<()> {
        Ok(())
    }

    async fn refresh_session(&self, _refresh_token: &str) -> AuthResult<LoginResponse> {
        Err(AuthError::InvalidToken)
    }

    async fn link_wallet(&self, _user_id: Uuid, _payload: LoginRequest) -> AuthResult<Wallet> {
        Err(AuthError::InvalidSignature)
    }

    async fn refresh_role_cache(&self, _address: &str, _chain_id: u64) -> AuthResult<Role> {
        Ok(self.claims.role)
    }
}

fn test_config(database_url: String) -> AppConfig {
    AppConfig {
        database_url,
        rpc_url: "http://localhost:8545".to_string(),
        chain_rpc_urls: Default::default(),
        chain_ws_urls: Default::default(),
        role_manager_address: Default::default(),
        coingecko_api_base: "https://api.coingecko.com/api/v3".to_string(),
        jwt_secret: "dev-secret".to_string(),
        jwt_audience: "rw3p".to_string(),
        jwt_issuer: "rw3p-api".to_string(),
        siwe_domain: "localhost:3000".to_string(),
        siwe_uri: "http://localhost:3000".to_string(),
        siwe_statement: "Sign in to Rust Web3 Risk Platform".to_string(),
        frontend_origins: vec!["http://localhost:3000".to_string()],
        cookie_secure: false,
        cookie_same_site: SameSite::Lax,
        access_token_ttl: Duration::from_secs(900),
        refresh_token_ttl: Duration::from_secs(604800),
        portfolio_sync_interval: Duration::from_secs(30),
        portfolio_max_concurrency: 4,
        portfolio_sync_retries: 3,
        ws_trigger_enabled: false,
        nonce_throttle_window: Duration::from_secs(1),
        role_cache_ttl_default: Duration::from_secs(300),
        role_cache_ttl_overrides: Default::default(),
        erc20_tokens: Vec::new(),
        token_price_ids: Default::default(),
        price_cache_ttl: Duration::from_secs(60),
        token_prices: Default::default(),
        redis_url: None,
        port: 0,
        portfolio_simulation: false,
        enable_alert_worker: false,
    }
}

#[sqlx::test(migrations = "../migrations")]
async fn get_me_returns_profile(pool: PgPool) {
    let user_id = Uuid::new_v4();
    let wallet_id = Uuid::new_v4();
    let wallet_address = "0x0000000000000000000000000000000000000001";

    sqlx::query("INSERT INTO users (id, primary_wallet) VALUES ($1, $2)")
        .bind(user_id)
        .bind(wallet_address)
        .execute(&pool)
        .await
        .expect("insert user");

    sqlx::query("INSERT INTO wallets (id, user_id, address, chain_id) VALUES ($1, $2, $3, $4)")
        .bind(wallet_id)
        .bind(user_id)
        .bind(wallet_address)
        .bind(1_i64)
        .execute(&pool)
        .await
        .expect("insert wallet");

    let config = test_config(std::env::var("DATABASE_URL").unwrap_or_default());
    let now = Utc::now();
    let claims = JwtClaims {
        sub: wallet_address.to_lowercase(),
        role: Role::Viewer,
        aud: config.jwt_audience.clone(),
        iss: config.jwt_issuer.clone(),
        exp: (now + ChronoDuration::minutes(15))
            .timestamp()
            .try_into()
            .unwrap(),
        iat: now.timestamp().try_into().unwrap(),
        session_id: Uuid::new_v4(),
        user_id,
        wallet_id,
    };

    let state = AppState {
        config: config.clone(),
        db: pool.clone(),
        provider: Arc::new(
            Provider::<Http>::try_from(config.rpc_url.as_str()).expect("provider should init"),
        ),
        auth: Arc::new(StubAuthService { claims }),
        portfolio: Arc::new(InMemoryPortfolioService::default()),
        strategy: Arc::new(InMemoryStrategyService::default()),
        alerts: Arc::new(InMemoryAlertService::default()),
        user_repo: Arc::new(PostgresUserRepository::new(pool.clone())),
        strategy_repo: Arc::new(PostgresStrategyRepository::new(pool.clone())),
        alert_repo: Arc::new(PostgresAlertRepository::new(pool.clone())),
        session_repo: Arc::new(PostgresSessionRepository::new(pool.clone())),
        wallet_repo: Arc::new(PostgresWalletRepository::new(pool.clone())),
        portfolio_repo: Arc::new(PostgresPortfolioSnapshotRepository::new(pool.clone())),
        price_history_repo: Arc::new(PostgresPriceHistoryRepository::new(pool.clone())),
        price_cache_repo: Arc::new(PostgresPriceCacheRepository::new(pool.clone())),
        transaction_repo: Arc::new(PostgresTransactionRepository::new(pool.clone())),
        nonce_limiter: Arc::new(
            NonceLimiter::new(Duration::from_secs(1), None)
                .await
                .expect("nonce limiter"),
        ),
    };

    let router = build_router(
        state,
        vec![HeaderValue::from_static("http://localhost:3000")],
    );

    let response = router
        .oneshot(
            Request::builder()
                .uri("/api/me")
                .header("Authorization", "Bearer test-token")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .expect("router response");

    assert_eq!(response.status(), StatusCode::OK);

    let body = to_bytes(response.into_body(), 1024 * 1024)
        .await
        .expect("body");
    let profile: domain::UserProfile = serde_json::from_slice(&body).expect("json");
    assert_eq!(profile.id, user_id);
    assert_eq!(profile.wallets.len(), 1);
    assert_eq!(profile.wallets[0].id, wallet_id);
    assert_eq!(profile.wallets[0].address, wallet_address);
    assert_eq!(profile.wallets[0].chain_id, 1);
}

#[sqlx::test(migrations = "../migrations")]
async fn admin_lists_and_revokes_sessions(pool: PgPool) {
    let user_id = Uuid::new_v4();
    let wallet_id = Uuid::new_v4();
    let wallet_address = "0x00000000000000000000000000000000000000aa";
    let session_id = Uuid::new_v4();
    let config = test_config(std::env::var("DATABASE_URL").unwrap_or_default());
    let now = Utc::now();
    let claims = JwtClaims {
        sub: wallet_address.to_lowercase(),
        role: Role::Admin,
        aud: config.jwt_audience.clone(),
        iss: config.jwt_issuer.clone(),
        exp: (now + ChronoDuration::minutes(15))
            .timestamp()
            .try_into()
            .unwrap(),
        iat: now.timestamp().try_into().unwrap(),
        session_id: Uuid::new_v4(),
        user_id,
        wallet_id,
    };

    sqlx::query("INSERT INTO users (id, primary_wallet) VALUES ($1, $2)")
        .bind(user_id)
        .bind(wallet_address)
        .execute(&pool)
        .await
        .expect("insert user");

    sqlx::query("INSERT INTO wallets (id, user_id, address, chain_id) VALUES ($1, $2, $3, $4)")
        .bind(wallet_id)
        .bind(user_id)
        .bind(wallet_address)
        .bind(1_i64)
        .execute(&pool)
        .await
        .expect("insert wallet");

    sqlx::query(
        "INSERT INTO user_sessions (id, user_id, wallet_id, refresh_token_hash, expires_at, refreshed_at)
         VALUES ($1, $2, $3, $4, $5, $6)",
    )
    .bind(session_id)
    .bind(user_id)
    .bind(wallet_id)
    .bind("hash-123")
    .bind(now + ChronoDuration::hours(1))
    .bind(now)
    .execute(&pool)
    .await
    .expect("insert session");

    let state = AppState {
        config: config.clone(),
        db: pool.clone(),
        provider: Arc::new(
            Provider::<Http>::try_from(config.rpc_url.as_str()).expect("provider should init"),
        ),
        auth: Arc::new(SingleTokenAuthService {
            token: "admin-token".to_string(),
            claims: claims.clone(),
        }),
        portfolio: Arc::new(InMemoryPortfolioService::default()),
        strategy: Arc::new(InMemoryStrategyService::default()),
        alerts: Arc::new(InMemoryAlertService::default()),
        user_repo: Arc::new(PostgresUserRepository::new(pool.clone())),
        strategy_repo: Arc::new(PostgresStrategyRepository::new(pool.clone())),
        alert_repo: Arc::new(PostgresAlertRepository::new(pool.clone())),
        session_repo: Arc::new(PostgresSessionRepository::new(pool.clone())),
        wallet_repo: Arc::new(PostgresWalletRepository::new(pool.clone())),
        portfolio_repo: Arc::new(PostgresPortfolioSnapshotRepository::new(pool.clone())),
        price_history_repo: Arc::new(PostgresPriceHistoryRepository::new(pool.clone())),
        price_cache_repo: Arc::new(PostgresPriceCacheRepository::new(pool.clone())),
        transaction_repo: Arc::new(PostgresTransactionRepository::new(pool.clone())),
        nonce_limiter: Arc::new(
            NonceLimiter::new(Duration::from_secs(1), None)
                .await
                .expect("nonce limiter"),
        ),
    };

    let router = build_router(
        state,
        vec![HeaderValue::from_static("http://localhost:3000")],
    );

    let list_resp = router
        .clone()
        .oneshot(
            Request::builder()
                .uri("/api/admin/sessions")
                .header("Authorization", "Bearer admin-token")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .expect("router response");
    assert_eq!(list_resp.status(), StatusCode::OK);
    let body = to_bytes(list_resp.into_body(), 1024 * 1024)
        .await
        .expect("body");
    let sessions: Vec<domain::SessionInfo> = serde_json::from_slice(&body).expect("json");
    assert_eq!(sessions.len(), 1);
    assert_eq!(sessions[0].id, session_id);
    assert!(sessions[0].revoked_at.is_none());

    let revoke_resp = router
        .clone()
        .oneshot(
            Request::builder()
                .uri(format!("/api/admin/sessions/{session_id}/revoke"))
                .method("POST")
                .header("Authorization", "Bearer admin-token")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .expect("router response");
    assert_eq!(revoke_resp.status(), StatusCode::NO_CONTENT);

    let revoked: Option<chrono::DateTime<Utc>> =
        sqlx::query_scalar("SELECT revoked_at FROM user_sessions WHERE id = $1")
            .bind(session_id)
            .fetch_one(&pool)
            .await
            .expect("fetch revoked_at");
    assert!(revoked.is_some());

    // Revoking again should be 404
    let second_revoke = router
        .oneshot(
            Request::builder()
                .uri(format!("/api/admin/sessions/{session_id}/revoke"))
                .method("POST")
                .header("Authorization", "Bearer admin-token")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .expect("router response");
    assert_eq!(second_revoke.status(), StatusCode::NOT_FOUND);
}

#[sqlx::test(migrations = "../migrations")]
async fn non_admin_cannot_access_admin_sessions(pool: PgPool) {
    let user_id = Uuid::new_v4();
    let wallet_id = Uuid::new_v4();
    let wallet_address = "0x00000000000000000000000000000000000000bb";
    let config = test_config(std::env::var("DATABASE_URL").unwrap_or_default());
    let now = Utc::now();
    let claims = JwtClaims {
        sub: wallet_address.to_lowercase(),
        role: Role::Viewer,
        aud: config.jwt_audience.clone(),
        iss: config.jwt_issuer.clone(),
        exp: (now + ChronoDuration::minutes(15))
            .timestamp()
            .try_into()
            .unwrap(),
        iat: now.timestamp().try_into().unwrap(),
        session_id: Uuid::new_v4(),
        user_id,
        wallet_id,
    };

    sqlx::query("INSERT INTO users (id, primary_wallet) VALUES ($1, $2)")
        .bind(user_id)
        .bind(wallet_address)
        .execute(&pool)
        .await
        .expect("insert user");

    sqlx::query("INSERT INTO wallets (id, user_id, address, chain_id) VALUES ($1, $2, $3, $4)")
        .bind(wallet_id)
        .bind(user_id)
        .bind(wallet_address)
        .bind(1_i64)
        .execute(&pool)
        .await
        .expect("insert wallet");

    let state = AppState {
        config: config.clone(),
        db: pool.clone(),
        provider: Arc::new(
            Provider::<Http>::try_from(config.rpc_url.as_str()).expect("provider should init"),
        ),
        auth: Arc::new(SingleTokenAuthService {
            token: "viewer-token".to_string(),
            claims,
        }),
        portfolio: Arc::new(InMemoryPortfolioService::default()),
        strategy: Arc::new(InMemoryStrategyService::default()),
        alerts: Arc::new(InMemoryAlertService::default()),
        user_repo: Arc::new(PostgresUserRepository::new(pool.clone())),
        strategy_repo: Arc::new(PostgresStrategyRepository::new(pool.clone())),
        alert_repo: Arc::new(PostgresAlertRepository::new(pool.clone())),
        session_repo: Arc::new(PostgresSessionRepository::new(pool.clone())),
        wallet_repo: Arc::new(PostgresWalletRepository::new(pool.clone())),
        portfolio_repo: Arc::new(PostgresPortfolioSnapshotRepository::new(pool.clone())),
        price_history_repo: Arc::new(PostgresPriceHistoryRepository::new(pool.clone())),
        price_cache_repo: Arc::new(PostgresPriceCacheRepository::new(pool.clone())),
        transaction_repo: Arc::new(PostgresTransactionRepository::new(pool.clone())),
        nonce_limiter: Arc::new(
            NonceLimiter::new(Duration::from_secs(1), None)
                .await
                .expect("nonce limiter"),
        ),
    };

    let router = build_router(
        state,
        vec![HeaderValue::from_static("http://localhost:3000")],
    );

    let resp = router
        .oneshot(
            Request::builder()
                .uri("/api/admin/sessions")
                .header("Authorization", "Bearer viewer-token")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .expect("router response");
    assert_eq!(resp.status(), StatusCode::FORBIDDEN);
}
