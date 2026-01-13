use anyhow::Result;
use async_trait::async_trait;
use domain::{BacktestResult, Strategy};
use sqlx::{PgPool, Row};
use tracing::warn;
use uuid::Uuid;

#[async_trait]
pub trait StrategyRepository: Send + Sync {
    async fn create(&self, strategy: &Strategy) -> Result<()>;
    async fn list_by_user(&self, user_id: Uuid) -> Result<Vec<Strategy>>;
    async fn find_by_id(&self, id: Uuid, user_id: Uuid) -> Result<Option<Strategy>>;
    async fn save_backtest(&self, result: &BacktestResult) -> Result<()>;
    async fn list_backtests(
        &self,
        strategy_id: Uuid,
        user_id: Uuid,
        limit: usize,
    ) -> Result<Vec<BacktestResult>>;
    async fn delete(&self, id: Uuid, user_id: Uuid) -> Result<bool>;
}

#[derive(Clone)]
pub struct PostgresStrategyRepository {
    pool: PgPool,
}

impl PostgresStrategyRepository {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }
}

#[async_trait]
impl StrategyRepository for PostgresStrategyRepository {
    async fn create(&self, strategy: &Strategy) -> Result<()> {
        sqlx::query(
            "INSERT INTO strategies (id, user_id, name, type, params) VALUES ($1, $2, $3, $4, $5)",
        )
        .bind(strategy.id)
        .bind(strategy.user_id)
        .bind(&strategy.name)
        .bind(&strategy.r#type)
        .bind(&strategy.params)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    async fn list_by_user(&self, user_id: Uuid) -> Result<Vec<Strategy>> {
        let rows = sqlx::query(
            "SELECT id, user_id, name, type, params FROM strategies WHERE user_id = $1 ORDER BY created_at DESC",
        )
        .bind(user_id)
        .fetch_all(&self.pool)
        .await?;

        rows.into_iter()
            .map(|row| {
                Ok(Strategy {
                    id: row.try_get("id")?,
                    user_id: row.try_get("user_id")?,
                    name: row.try_get("name")?,
                    r#type: row.try_get("type")?,
                    params: row.try_get("params")?,
                })
            })
            .collect()
    }

    async fn find_by_id(&self, id: Uuid, user_id: Uuid) -> Result<Option<Strategy>> {
        let row = sqlx::query(
            "SELECT id, user_id, name, type, params FROM strategies WHERE id = $1 AND user_id = $2 LIMIT 1",
        )
        .bind(id)
        .bind(user_id)
        .fetch_optional(&self.pool)
        .await?;

        if let Some(row) = row {
            Ok(Some(Strategy {
                id: row.try_get("id")?,
                user_id: row.try_get("user_id")?,
                name: row.try_get("name")?,
                r#type: row.try_get("type")?,
                params: row.try_get("params")?,
            }))
        } else {
            Ok(None)
        }
    }

    async fn save_backtest(&self, result: &BacktestResult) -> Result<()> {
        sqlx::query(
            "INSERT INTO strategy_backtests (id, strategy_id, started_at, completed_at, result)
             VALUES ($1, $2, NOW(), NOW(), $3)",
        )
        .bind(Uuid::new_v4())
        .bind(result.strategy_id)
        .bind(serde_json::to_value(&result)?)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    async fn list_backtests(
        &self,
        strategy_id: Uuid,
        user_id: Uuid,
        limit: usize,
    ) -> Result<Vec<BacktestResult>> {
        let rows = sqlx::query(
            "SELECT b.result, b.completed_at FROM strategy_backtests b
             JOIN strategies s ON s.id = b.strategy_id
             WHERE b.strategy_id = $1 AND s.user_id = $2
             ORDER BY b.completed_at DESC
             LIMIT $3",
        )
        .bind(strategy_id)
        .bind(user_id)
        .bind(limit as i64)
        .fetch_all(&self.pool)
        .await?;

        rows.into_iter()
            .filter_map(|row| {
                let value: serde_json::Value = row.try_get("result").ok()?;
                match serde_json::from_value::<BacktestResult>(value) {
                    Ok(mut parsed) => {
                        let completed_at: chrono::DateTime<chrono::Utc> =
                            row.try_get("completed_at").unwrap_or_else(|_| chrono::Utc::now());
                        parsed.completed_at = Some(completed_at);
                        Some(Ok(parsed))
                    }
                    Err(err) => {
                        warn!(error = %err, "invalid backtest result json; skipping");
                        None
                    }
                }
            })
            .collect()
    }

    async fn delete(&self, id: Uuid, user_id: Uuid) -> Result<bool> {
        // Remove backtests first, then the strategy.
        let mut tx = self.pool.begin().await?;
        sqlx::query("DELETE FROM strategy_backtests WHERE strategy_id = $1")
            .bind(id)
            .execute(&mut *tx)
            .await?;

        let res = sqlx::query("DELETE FROM strategies WHERE id = $1 AND user_id = $2")
            .bind(id)
            .bind(user_id)
            .execute(&mut *tx)
            .await?;
        tx.commit().await?;
        Ok(res.rows_affected() > 0)
    }
}
