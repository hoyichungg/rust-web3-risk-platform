use anyhow::Result;
use async_trait::async_trait;
use chrono::{DateTime, NaiveDate, Utc};
use domain::{PortfolioSnapshot, Position};
use sqlx::{PgPool, Row};
use uuid::Uuid;

#[async_trait]
pub trait PortfolioSnapshotRepository: Send + Sync {
    async fn insert_snapshot(&self, snapshot: &PortfolioSnapshot) -> Result<()>;
    async fn latest_by_wallet(&self, wallet_id: Uuid) -> Result<Option<PortfolioSnapshot>>;
    async fn log_indexer_run(
        &self,
        wallet_id: Uuid,
        status: &str,
        error: Option<&str>,
    ) -> Result<()>;
    async fn history_by_wallet(
        &self,
        wallet_id: Uuid,
        limit: i64,
    ) -> Result<Vec<PortfolioSnapshot>>;
    async fn history_since(
        &self,
        wallet_id: Uuid,
        since: DateTime<Utc>,
    ) -> Result<Vec<PortfolioSnapshot>>;
    async fn upsert_daily_snapshot(
        &self,
        wallet_id: Uuid,
        day: NaiveDate,
        total_usd_value: f64,
        positions: &[Position],
    ) -> Result<()>;
}

#[derive(Clone)]
pub struct PostgresPortfolioSnapshotRepository {
    pool: PgPool,
}

impl PostgresPortfolioSnapshotRepository {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }
}

#[async_trait]
impl PortfolioSnapshotRepository for PostgresPortfolioSnapshotRepository {
    async fn insert_snapshot(&self, snapshot: &PortfolioSnapshot) -> Result<()> {
        sqlx::query(
            "INSERT INTO portfolio_snapshots (id, wallet_id, total_usd_value, snapshot_time, positions)
             VALUES ($1, $2, $3, $4, $5)",
        )
        .bind(Uuid::new_v4())
        .bind(snapshot.wallet_id)
        .bind(snapshot.total_usd_value)
        .bind(snapshot.timestamp)
        .bind(serde_json::to_value(&snapshot.positions)?)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    async fn latest_by_wallet(&self, wallet_id: Uuid) -> Result<Option<PortfolioSnapshot>> {
        let row = sqlx::query(
            "SELECT wallet_id, total_usd_value::float8 AS total_usd_value, snapshot_time, positions
             FROM portfolio_snapshots WHERE wallet_id = $1 ORDER BY snapshot_time DESC LIMIT 1",
        )
        .bind(wallet_id)
        .fetch_optional(&self.pool)
        .await?;

        if let Some(row) = row {
            let timestamp: DateTime<Utc> = row.try_get("snapshot_time")?;
            let positions: Vec<Position> =
                serde_json::from_value(row.try_get("positions")?).unwrap_or_default();
            Ok(Some(PortfolioSnapshot {
                wallet_id,
                positions,
                total_usd_value: row.try_get("total_usd_value")?,
                timestamp,
            }))
        } else {
            Ok(None)
        }
    }

    async fn log_indexer_run(
        &self,
        wallet_id: Uuid,
        status: &str,
        error: Option<&str>,
    ) -> Result<()> {
        sqlx::query(
            "INSERT INTO portfolio_indexer_runs (id, wallet_id, status, error)
             VALUES ($1, $2, $3, $4)",
        )
        .bind(Uuid::new_v4())
        .bind(wallet_id)
        .bind(status)
        .bind(error)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    async fn history_by_wallet(
        &self,
        wallet_id: Uuid,
        limit: i64,
    ) -> Result<Vec<PortfolioSnapshot>> {
        let rows = sqlx::query(
            "SELECT wallet_id, total_usd_value::float8 AS total_usd_value, snapshot_time, positions
             FROM (
                SELECT wallet_id, total_usd_value::float8 AS total_usd_value, snapshot_time, positions
                FROM portfolio_snapshots
                WHERE wallet_id = $1
                ORDER BY snapshot_time DESC
                LIMIT $2
             ) recent
             ORDER BY snapshot_time ASC",
        )
        .bind(wallet_id)
        .bind(limit.max(1))
        .fetch_all(&self.pool)
        .await?;

        rows.into_iter()
            .map(|row| {
                let timestamp: DateTime<Utc> = row.try_get("snapshot_time")?;
                let positions: Vec<Position> =
                    serde_json::from_value(row.try_get("positions")?).unwrap_or_default();
                Ok(PortfolioSnapshot {
                    wallet_id,
                    positions,
                    total_usd_value: row.try_get("total_usd_value")?,
                    timestamp,
                })
            })
            .collect()
    }

    async fn history_since(
        &self,
        wallet_id: Uuid,
        since: DateTime<Utc>,
    ) -> Result<Vec<PortfolioSnapshot>> {
        let rows = sqlx::query(
            "SELECT wallet_id, total_usd_value::float8 AS total_usd_value, snapshot_time, positions
             FROM portfolio_snapshots
             WHERE wallet_id = $1 AND snapshot_time >= $2
             ORDER BY snapshot_time ASC",
        )
        .bind(wallet_id)
        .bind(since)
        .fetch_all(&self.pool)
        .await?;

        rows.into_iter()
            .map(|row| {
                let timestamp: DateTime<Utc> = row.try_get("snapshot_time")?;
                let positions: Vec<Position> =
                    serde_json::from_value(row.try_get("positions")?).unwrap_or_default();
                Ok(PortfolioSnapshot {
                    wallet_id,
                    positions,
                    total_usd_value: row.try_get("total_usd_value")?,
                    timestamp,
                })
            })
            .collect()
    }

    async fn upsert_daily_snapshot(
        &self,
        wallet_id: Uuid,
        day: NaiveDate,
        total_usd_value: f64,
        positions: &[Position],
    ) -> Result<()> {
        sqlx::query(
            "INSERT INTO portfolio_daily_snapshots (id, wallet_id, day, total_usd_value, positions)
             VALUES ($1, $2, $3, $4, $5)
             ON CONFLICT (wallet_id, day) DO UPDATE
             SET total_usd_value = EXCLUDED.total_usd_value,
                 positions = EXCLUDED.positions,
                 updated_at = NOW()",
        )
        .bind(Uuid::new_v4())
        .bind(wallet_id)
        .bind(day)
        .bind(total_usd_value)
        .bind(serde_json::to_value(positions)?)
        .execute(&self.pool)
        .await?;
        Ok(())
    }
}
