use alert_engine::AlertService;
use auth::AuthService;
use ethers::providers::{Http, Provider};
use indexer::PortfolioService;
use sqlx::PgPool;
use std::sync::Arc;
use strategy_engine::StrategyService;

use crate::{
    config::AppConfig,
    nonce_limiter::NonceLimiter,
    repositories::{
        AlertRepository, PortfolioSnapshotRepository, PriceCacheRepository, PriceHistoryRepository,
        SessionRepository, StrategyRepository, TransactionRepository, UserRepository,
        WalletRepository,
    },
};

#[allow(dead_code)]
#[derive(Clone)]
pub struct AppState {
    pub config: AppConfig,
    pub db: PgPool,
    pub provider: Arc<Provider<Http>>,
    pub auth: Arc<dyn AuthService>,
    pub portfolio: Arc<dyn PortfolioService>,
    pub strategy: Arc<dyn StrategyService>,
    pub alerts: Arc<dyn AlertService>,
    pub user_repo: Arc<dyn UserRepository>,
    pub strategy_repo: Arc<dyn StrategyRepository>,
    pub alert_repo: Arc<dyn AlertRepository>,
    pub session_repo: Arc<dyn SessionRepository>,
    pub wallet_repo: Arc<dyn WalletRepository>,
    pub portfolio_repo: Arc<dyn PortfolioSnapshotRepository>,
    pub price_cache_repo: Arc<dyn PriceCacheRepository>,
    pub price_history_repo: Arc<dyn PriceHistoryRepository>,
    pub transaction_repo: Arc<dyn TransactionRepository>,
    pub nonce_limiter: Arc<NonceLimiter>,
}

// Ensure critical dependencies uphold Send/Sync for Axum state usage.
#[allow(dead_code)]
fn _assert_state_types_are_send_sync()
where
    AppConfig: Send + Sync + 'static,
    PgPool: Send + Sync + 'static,
    Provider<Http>: Send + Sync + 'static,
    dyn AuthService: Send + Sync,
    dyn PortfolioService: Send + Sync,
    dyn StrategyService: Send + Sync,
    dyn AlertService: Send + Sync,
    dyn UserRepository: Send + Sync,
    dyn StrategyRepository: Send + Sync,
    dyn AlertRepository: Send + Sync,
    dyn SessionRepository: Send + Sync,
    dyn WalletRepository: Send + Sync,
    dyn PortfolioSnapshotRepository: Send + Sync,
    dyn PriceHistoryRepository: Send + Sync,
    dyn PriceCacheRepository: Send + Sync,
    dyn TransactionRepository: Send + Sync,
    NonceLimiter: Send + Sync,
{
}

#[allow(dead_code)]
fn _assert_state_bounds() {
    fn assert_bounds<T: Clone + Send + Sync + 'static>() {}
    assert_bounds::<AppState>();
}
