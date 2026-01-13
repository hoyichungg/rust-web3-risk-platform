use std::str::FromStr;

use axum::{
    Json, Router,
    extract::{Path, State},
    http::StatusCode,
    routing::{delete, get, post},
};
use domain::{CreateWalletRequest, WalletResponse};
use ethers::types::Address;
use uuid::Uuid;

use crate::{auth_middleware::CurrentUser, state::AppState};

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/wallets", get(list_wallets).post(create_wallet))
        .route("/wallets/:wallet_id", delete(delete_wallet))
        .route("/wallets/:wallet_id/primary", post(set_primary_wallet))
}

async fn list_wallets(
    State(state): State<AppState>,
    user: CurrentUser,
) -> Result<Json<Vec<WalletResponse>>, StatusCode> {
    let wallets = state
        .wallet_repo
        .list_by_user(user.claims().user_id)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    Ok(Json(
        wallets
            .into_iter()
            .map(|wallet| WalletResponse {
                id: wallet.id,
                address: wallet.address,
                chain_id: wallet.chain_id,
            })
            .collect(),
    ))
}

async fn create_wallet(
    State(state): State<AppState>,
    user: CurrentUser,
    Json(payload): Json<CreateWalletRequest>,
) -> Result<Json<WalletResponse>, StatusCode> {
    let address = Address::from_str(payload.address.trim()).map_err(|_| StatusCode::BAD_REQUEST)?;
    let address = format!("{:#x}", address);

    let wallet = state
        .wallet_repo
        .create_wallet(user.claims().user_id, &address, payload.chain_id)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    Ok(Json(WalletResponse {
        id: wallet.id,
        address: wallet.address,
        chain_id: wallet.chain_id,
    }))
}

async fn delete_wallet(
    State(state): State<AppState>,
    user: CurrentUser,
    Path(wallet_id): Path<Uuid>,
) -> Result<StatusCode, StatusCode> {
    let deleted = state
        .wallet_repo
        .delete_wallet(user.claims().user_id, wallet_id)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    if deleted {
        Ok(StatusCode::NO_CONTENT)
    } else {
        Err(StatusCode::NOT_FOUND)
    }
}

async fn set_primary_wallet(
    State(state): State<AppState>,
    user: CurrentUser,
    Path(wallet_id): Path<Uuid>,
) -> Result<StatusCode, StatusCode> {
    // 確認錢包存在且屬於本人
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

    let updated = state
        .user_repo
        .set_primary_wallet(user.claims().user_id, wallet_id)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    if updated {
        Ok(StatusCode::NO_CONTENT)
    } else {
        Err(StatusCode::NOT_FOUND)
    }
}
