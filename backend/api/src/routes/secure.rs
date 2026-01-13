use axum::{
    Json, Router,
    extract::{Path, State},
    http::StatusCode,
    routing::{get, post},
};
use domain::{Role, SessionInfo, UserProfile};
use serde::Serialize;

use crate::{auth_middleware::CurrentUser, repositories::UserProfileData, state::AppState};
use uuid::Uuid;

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/me", get(me))
        .route("/admin/ping", get(admin_ping))
        .route("/admin/users", get(admin_users))
        .route("/admin/sessions", get(admin_sessions))
        .route("/admin/sessions/:session_id/revoke", post(revoke_session))
        .route("/admin/roles/refresh", post(refresh_roles))
}

async fn me(
    State(state): State<AppState>,
    user: CurrentUser,
) -> Result<Json<UserProfile>, StatusCode> {
    let profile = state
        .user_repo
        .find_profile(user.claims().user_id)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    let Some(UserProfileData {
        id,
        primary_wallet,
        wallets,
    }) = profile
    else {
        return Err(StatusCode::NOT_FOUND);
    };

    Ok(Json(UserProfile {
        id,
        primary_wallet,
        role: user.claims().role,
        wallets,
    }))
}

#[derive(Serialize)]
struct AdminPingResponse {
    message: &'static str,
}

async fn admin_ping(user: CurrentUser) -> Result<Json<AdminPingResponse>, StatusCode> {
    user.ensure_role(Role::Admin)?;
    Ok(Json(AdminPingResponse {
        message: "admin pong",
    }))
}

#[derive(Serialize)]
struct AdminWalletResponse {
    id: Uuid,
    address: String,
    chain_id: u64,
    cached_role: Option<Role>,
    cached_at: Option<String>,
}

#[derive(Serialize)]
struct AdminUserResponse {
    id: Uuid,
    primary_wallet: String,
    wallets: Vec<AdminWalletResponse>,
}

async fn admin_users(
    State(state): State<AppState>,
    user: CurrentUser,
) -> Result<Json<Vec<AdminUserResponse>>, StatusCode> {
    user.ensure_role(Role::Admin)?;
    let users = state
        .user_repo
        .list_admin_users()
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    let response = users
        .into_iter()
        .map(|user| AdminUserResponse {
            id: user.id,
            primary_wallet: user.primary_wallet,
            wallets: user
                .wallets
                .into_iter()
                .map(|wallet| AdminWalletResponse {
                    id: wallet.id,
                    address: wallet.address,
                    chain_id: wallet.chain_id,
                    cached_role: wallet.cached_role,
                    cached_at: wallet.cached_at.map(|timestamp| timestamp.to_rfc3339()),
                })
                .collect(),
        })
        .collect();

    Ok(Json(response))
}

async fn admin_sessions(
    State(state): State<AppState>,
    user: CurrentUser,
) -> Result<Json<Vec<SessionInfo>>, StatusCode> {
    user.ensure_role(Role::Admin)?;
    state
        .session_repo
        .list_all()
        .await
        .map(Json)
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)
}

async fn revoke_session(
    State(state): State<AppState>,
    user: CurrentUser,
    Path(session_id): Path<Uuid>,
) -> Result<StatusCode, StatusCode> {
    user.ensure_role(Role::Admin)?;
    let revoked = state
        .session_repo
        .revoke(session_id)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    if revoked {
        Ok(StatusCode::NO_CONTENT)
    } else {
        Err(StatusCode::NOT_FOUND)
    }
}

#[derive(Serialize)]
struct RoleRefreshItem {
    wallet_id: Uuid,
    address: String,
    chain_id: u64,
    role: Role,
}

#[derive(Serialize)]
struct RoleRefreshResponse {
    refreshed: Vec<RoleRefreshItem>,
}

async fn refresh_roles(
    State(state): State<AppState>,
    user: CurrentUser,
) -> Result<Json<RoleRefreshResponse>, StatusCode> {
    user.ensure_role(Role::Admin)?;
    let wallets = state
        .wallet_repo
        .list_all()
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    let mut refreshed = Vec::with_capacity(wallets.len());
    for wallet in wallets {
        let role = state
            .auth
            .refresh_role_cache(&wallet.address, wallet.chain_id)
            .await
            .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
        refreshed.push(RoleRefreshItem {
            wallet_id: wallet.id,
            address: wallet.address,
            chain_id: wallet.chain_id,
            role,
        });
    }

    Ok(Json(RoleRefreshResponse { refreshed }))
}
