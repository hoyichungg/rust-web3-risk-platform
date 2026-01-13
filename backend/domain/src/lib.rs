use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct User {
    pub id: Uuid,
    pub primary_wallet: String,
}

#[repr(u8)]
#[derive(Debug, Serialize, Deserialize, Clone, Copy, PartialEq, Eq)]
pub enum Role {
    None,
    Admin,
    Viewer,
}

impl Role {
    pub fn from_u8(value: u8) -> Self {
        match value {
            1 => Role::Admin,
            2 => Role::Viewer,
            _ => Role::None,
        }
    }

    pub fn as_u8(self) -> u8 {
        self as u8
    }
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Wallet {
    pub id: Uuid,
    pub user_id: Uuid,
    pub address: String,
    pub chain_id: u64,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Position {
    pub asset_symbol: String,
    pub amount: f64,
    pub usd_value: f64,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct PortfolioSnapshot {
    pub wallet_id: Uuid,
    pub positions: Vec<Position>,
    pub total_usd_value: f64,
    pub timestamp: DateTime<Utc>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct WalletTransaction {
    pub id: Uuid,
    pub wallet_id: Uuid,
    pub chain_id: u64,
    pub tx_hash: String,
    pub block_number: i64,
    pub log_index: i64,
    pub asset_symbol: String,
    pub amount: f64,
    pub usd_value: f64,
    pub direction: String,
    pub from_address: String,
    pub to_address: String,
    pub block_timestamp: DateTime<Utc>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct PriceHistoryPoint {
    pub id: Uuid,
    pub symbol: String,
    pub price: f64,
    pub price_ts: DateTime<Utc>,
    pub source: String,
    #[serde(default)]
    pub chain_id: Option<u64>,
}

#[derive(Debug, Deserialize)]
pub struct CreateWalletRequest {
    pub address: String,
    pub chain_id: u64,
}

#[derive(Debug, Serialize)]
pub struct WalletResponse {
    pub id: Uuid,
    pub address: String,
    pub chain_id: u64,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Strategy {
    pub id: Uuid,
    pub user_id: Uuid,
    pub name: String,
    pub r#type: String,
    pub params: serde_json::Value,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct BacktestResult {
    pub strategy_id: Uuid,
    pub equity_curve: Vec<(DateTime<Utc>, f64)>,
    pub metrics: serde_json::Value,
    pub completed_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct AlertRule {
    pub id: Uuid,
    pub user_id: Uuid,
    pub r#type: String,
    pub threshold: f64,
    pub enabled: bool,
    pub cooldown_secs: i64,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct NonceResponse {
    pub nonce: String,
}

#[derive(Debug, Deserialize)]
pub struct LoginRequest {
    pub message: String,
    pub signature: String,
}

#[derive(Debug, Serialize)]
pub struct LoginResponse {
    pub token: String,
    pub refresh_token: String,
    pub role: Role,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct UserWallet {
    pub id: Uuid,
    pub address: String,
    pub chain_id: u64,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct UserProfile {
    pub id: Uuid,
    pub primary_wallet: String,
    pub role: Role,
    pub wallets: Vec<UserWallet>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct SessionInfo {
    pub id: Uuid,
    pub user_id: Uuid,
    pub wallet_id: Uuid,
    pub wallet_address: String,
    pub primary_wallet: String,
    pub created_at: DateTime<Utc>,
    pub refreshed_at: DateTime<Utc>,
    pub expires_at: DateTime<Utc>,
    pub revoked_at: Option<DateTime<Utc>>,
}
