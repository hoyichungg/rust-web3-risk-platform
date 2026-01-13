use anyhow::Result;
use async_trait::async_trait;
use chrono::NaiveDate;
use chrono::Utc;
use domain::WalletTransaction;
use sqlx::{PgPool, Row};
use uuid::Uuid;

#[async_trait]
pub trait TransactionRepository: Send + Sync {
    async fn insert_transactions(&self, txs: &[WalletTransaction]) -> Result<()>;
    async fn last_tx_block(&self, wallet_id: Uuid) -> Result<Option<i64>>;
    async fn update_last_tx_block(&self, wallet_id: Uuid, chain_id: u64, block: i64) -> Result<()>;
    async fn update_last_daily_snapshot(
        &self,
        wallet_id: Uuid,
        chain_id: u64,
        day: NaiveDate,
    ) -> Result<()>;
    async fn net_flow_since(&self, wallet_id: Uuid, since: chrono::DateTime<Utc>) -> Result<f64>;
}

#[derive(Clone)]
pub struct PostgresTransactionRepository {
    pool: PgPool,
}

impl PostgresTransactionRepository {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }
}

#[async_trait]
impl TransactionRepository for PostgresTransactionRepository {
    async fn insert_transactions(&self, txs: &[WalletTransaction]) -> Result<()> {
        if txs.is_empty() {
            return Ok(());
        }
        let mut tx = self.pool.begin().await?;
        for t in txs {
            sqlx::query(
                "INSERT INTO wallet_transactions
                 (id, wallet_id, chain_id, tx_hash, block_number, log_index, asset_symbol, amount, usd_value, direction, from_address, to_address, block_timestamp, raw)
                 VALUES ($1,$2,$3,$4,$5,$6,$7,$8,$9,$10,$11,$12,$13,$14)
                 ON CONFLICT (wallet_id, tx_hash, log_index) DO NOTHING",
            )
            .bind(t.id)
            .bind(t.wallet_id)
            .bind(i64::try_from(t.chain_id).unwrap_or(0))
            .bind(&t.tx_hash)
            .bind(t.block_number)
            .bind(t.log_index)
            .bind(&t.asset_symbol)
            .bind(t.amount)
            .bind(t.usd_value)
            .bind(&t.direction)
            .bind(&t.from_address)
            .bind(&t.to_address)
            .bind(t.block_timestamp)
            .bind(serde_json::json!({}))
            .execute(&mut *tx)
            .await?;
        }
        tx.commit().await?;
        Ok(())
    }

    async fn last_tx_block(&self, wallet_id: Uuid) -> Result<Option<i64>> {
        let row = sqlx::query("SELECT last_tx_block FROM wallet_sync_cursors WHERE wallet_id = $1")
            .bind(wallet_id)
            .fetch_optional(&self.pool)
            .await?;
        Ok(row.map(|r| r.try_get("last_tx_block").unwrap_or(0)))
    }

    async fn update_last_tx_block(&self, wallet_id: Uuid, chain_id: u64, block: i64) -> Result<()> {
        sqlx::query(
            "INSERT INTO wallet_sync_cursors (wallet_id, chain_id, last_tx_block, updated_at)
             VALUES ($1, $2, $3, NOW())
             ON CONFLICT (wallet_id) DO UPDATE
             SET last_tx_block = EXCLUDED.last_tx_block,
                 chain_id = EXCLUDED.chain_id,
                 updated_at = NOW()",
        )
        .bind(wallet_id)
        .bind(i64::try_from(chain_id).unwrap_or(0))
        .bind(block)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    async fn update_last_daily_snapshot(
        &self,
        wallet_id: Uuid,
        chain_id: u64,
        day: NaiveDate,
    ) -> Result<()> {
        sqlx::query(
            "INSERT INTO wallet_sync_cursors (wallet_id, chain_id, last_daily_snapshot, updated_at)
             VALUES ($1, $2, $3, NOW())
             ON CONFLICT (wallet_id) DO UPDATE
             SET last_daily_snapshot = EXCLUDED.last_daily_snapshot,
                 chain_id = EXCLUDED.chain_id,
                 updated_at = NOW()",
        )
        .bind(wallet_id)
        .bind(i64::try_from(chain_id).unwrap_or(0))
        .bind(day)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    async fn net_flow_since(&self, wallet_id: Uuid, since: chrono::DateTime<Utc>) -> Result<f64> {
        let row = sqlx::query(
            "SELECT
                COALESCE(SUM(CASE WHEN direction = 'out' THEN usd_value ELSE 0 END), 0) AS outflow,
                COALESCE(SUM(CASE WHEN direction = 'in' THEN usd_value ELSE 0 END), 0) AS inflow
             FROM wallet_transactions
             WHERE wallet_id = $1 AND block_timestamp >= $2",
        )
        .bind(wallet_id)
        .bind(since)
        .fetch_one(&self.pool)
        .await?;

        let outflow: f64 = row.try_get("outflow").unwrap_or(0.0);
        let inflow: f64 = row.try_get("inflow").unwrap_or(0.0);
        Ok(outflow - inflow)
    }
}
