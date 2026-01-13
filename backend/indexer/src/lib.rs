use async_trait::async_trait;
use chrono::Utc;
use domain::PortfolioSnapshot;
use uuid::Uuid;

#[async_trait]
pub trait PortfolioService: Send + Sync {
    async fn latest_snapshot(&self, wallet_id: Uuid) -> Option<PortfolioSnapshot>;
}

#[derive(Clone, Default)]
pub struct InMemoryPortfolioService;

#[async_trait]
impl PortfolioService for InMemoryPortfolioService {
    async fn latest_snapshot(&self, wallet_id: Uuid) -> Option<PortfolioSnapshot> {
        // 簡化：回傳空倉位的 snapshot，後續可串真正 indexer。
        Some(PortfolioSnapshot {
            wallet_id,
            positions: vec![],
            total_usd_value: 0.0,
            timestamp: Utc::now(),
        })
    }
}
