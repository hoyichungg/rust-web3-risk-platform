use auth::AuthError;
use std::{net::SocketAddr, time::Duration as StdDuration};

use axum::{
    Json, Router,
    extract::{ConnectInfo, State},
    http::StatusCode,
    routing::{get, post},
};
use axum_extra::extract::cookie::{Cookie, CookieJar};
use domain::{LoginRequest, LoginResponse, NonceResponse, Role, WalletResponse};
use serde::Serialize;
use time::Duration as CookieDuration;

use crate::{
    auth_middleware::{AUTH_ROLE_COOKIE, AUTH_TOKEN_COOKIE, CurrentUser},
    config::AppConfig,
    nonce_limiter::NonceLimiterError,
    state::AppState,
};

const AUTH_REFRESH_COOKIE: &str = "rw3p_refresh";

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/auth/nonce", get(get_nonce))
        .route("/auth/login", post(login))
        .route("/auth/logout", post(logout))
        .route("/auth/refresh", post(refresh_token))
        .route("/auth/link-wallet", post(link_wallet))
}

async fn get_nonce(
    State(state): State<AppState>,
    ConnectInfo(addr): ConnectInfo<SocketAddr>,
) -> Result<Json<NonceResponse>, StatusCode> {
    state
        .nonce_limiter
        .check(addr.ip())
        .await
        .map_err(|err| match err {
            NonceLimiterError::RateLimited => StatusCode::TOO_MANY_REQUESTS,
            NonceLimiterError::Backend { _message: _ } => StatusCode::INTERNAL_SERVER_ERROR,
        })?;
    state
        .auth
        .issue_nonce()
        .await
        .map(Json)
        .map_err(map_auth_err)
}

#[derive(Serialize)]
struct LoginSuccess {
    role: Role,
}

async fn login(
    State(state): State<AppState>,
    jar: CookieJar,
    Json(payload): Json<LoginRequest>,
) -> Result<(CookieJar, Json<LoginSuccess>), StatusCode> {
    let login = state.auth.login(payload).await.map_err(map_auth_err)?;
    let jar = apply_auth_cookies(jar, &login, &state.config);
    Ok((jar, Json(LoginSuccess { role: login.role })))
}

async fn logout(
    State(state): State<AppState>,
    jar: CookieJar,
    user: CurrentUser,
) -> Result<(CookieJar, StatusCode), StatusCode> {
    state
        .auth
        .logout(user.claims().session_id)
        .await
        .map_err(map_auth_err)?;
    let jar = clear_auth_cookies(jar, &state.config);
    Ok((jar, StatusCode::NO_CONTENT))
}

async fn refresh_token(
    State(state): State<AppState>,
    jar: CookieJar,
) -> Result<(CookieJar, Json<LoginSuccess>), StatusCode> {
    let refresh_cookie = jar
        .get(AUTH_REFRESH_COOKIE)
        .ok_or(StatusCode::UNAUTHORIZED)?;
    let login = state
        .auth
        .refresh_session(refresh_cookie.value())
        .await
        .map_err(map_auth_err)?;
    let jar = apply_auth_cookies(jar, &login, &state.config);
    Ok((jar, Json(LoginSuccess { role: login.role })))
}

async fn link_wallet(
    State(state): State<AppState>,
    user: CurrentUser,
    Json(payload): Json<LoginRequest>,
) -> Result<Json<WalletResponse>, StatusCode> {
    let wallet = state
        .auth
        .link_wallet(user.claims().user_id, payload)
        .await
        .map_err(map_auth_err)?;
    Ok(Json(WalletResponse {
        id: wallet.id,
        address: wallet.address,
        chain_id: wallet.chain_id,
    }))
}

fn map_auth_err(err: AuthError) -> StatusCode {
    match err {
        AuthError::InvalidSignature
        | AuthError::InvalidMessage
        | AuthError::NonceNotFound
        | AuthError::NonceExpired
        | AuthError::InvalidToken
        | AuthError::RefreshTokenInvalid => StatusCode::UNAUTHORIZED,
        AuthError::WalletAlreadyLinked => StatusCode::CONFLICT,
        AuthError::Internal(_) => StatusCode::INTERNAL_SERVER_ERROR,
    }
}

fn apply_auth_cookies(jar: CookieJar, login: &LoginResponse, config: &AppConfig) -> CookieJar {
    let token_cookie = Cookie::build((AUTH_TOKEN_COOKIE, login.token.clone()))
        .http_only(true)
        .secure(config.cookie_secure)
        .same_site(config.cookie_same_site)
        .path("/")
        .max_age(duration_to_cookie(config.access_token_ttl))
        .build();
    let role_cookie = Cookie::build((AUTH_ROLE_COOKIE, login.role.as_u8().to_string()))
        .http_only(true)
        .secure(config.cookie_secure)
        .same_site(config.cookie_same_site)
        .path("/")
        .max_age(duration_to_cookie(config.access_token_ttl))
        .build();
    let refresh_cookie = Cookie::build((AUTH_REFRESH_COOKIE, login.refresh_token.clone()))
        .http_only(true)
        .secure(config.cookie_secure)
        .same_site(config.cookie_same_site)
        .path("/")
        .max_age(duration_to_cookie(config.refresh_token_ttl))
        .build();
    jar.add(token_cookie).add(role_cookie).add(refresh_cookie)
}

fn clear_auth_cookies(jar: CookieJar, config: &AppConfig) -> CookieJar {
    let token_cookie = Cookie::build((AUTH_TOKEN_COOKIE, ""))
        .http_only(true)
        .secure(config.cookie_secure)
        .same_site(config.cookie_same_site)
        .path("/")
        .max_age(CookieDuration::seconds(0))
        .build();
    let role_cookie = Cookie::build((AUTH_ROLE_COOKIE, ""))
        .http_only(true)
        .secure(config.cookie_secure)
        .same_site(config.cookie_same_site)
        .path("/")
        .max_age(CookieDuration::seconds(0))
        .build();
    let refresh_cookie = Cookie::build((AUTH_REFRESH_COOKIE, ""))
        .http_only(true)
        .secure(config.cookie_secure)
        .same_site(config.cookie_same_site)
        .path("/")
        .max_age(CookieDuration::seconds(0))
        .build();
    jar.add(token_cookie).add(role_cookie).add(refresh_cookie)
}

fn duration_to_cookie(duration: StdDuration) -> CookieDuration {
    let seconds = duration.as_secs().min(i64::MAX as u64) as i64;
    CookieDuration::seconds(seconds)
}
