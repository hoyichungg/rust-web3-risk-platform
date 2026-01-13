use anyhow::Result;
use async_trait::async_trait;
use chrono::{DateTime, Utc};
use domain::PriceHistoryPoint;
use sqlx::{PgPool, Row};
use uuid::Uuid;

#[async_trait]
pub trait PriceHistoryRepository: Send + Sync {
    async fn upsert_points(&self, points: &[PriceHistoryPoint]) -> Result<()>;
    async fn fetch_range(
        &self,
        symbol: &str,
        chain_id: Option<u64>,
        from: DateTime<Utc>,
        to: DateTime<Utc>,
    ) -> Result<Vec<PriceHistoryPoint>>;
    async fn latest_timestamp(
        &self,
        symbol: &str,
        chain_id: Option<u64>,
    ) -> Result<Option<DateTime<Utc>>>;
}

#[derive(Clone)]
pub struct PostgresPriceHistoryRepository {
    pool: PgPool,
}

impl PostgresPriceHistoryRepository {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }
}

#[async_trait]
impl PriceHistoryRepository for PostgresPriceHistoryRepository {
    async fn upsert_points(&self, points: &[PriceHistoryPoint]) -> Result<()> {
        if points.is_empty() {
            return Ok(());
        }
        let mut tx = self.pool.begin().await?;
        for p in points {
            sqlx::query(
                "INSERT INTO price_history (id, symbol, price, price_ts, source, chain_id)
                 VALUES ($1, $2, $3, $4, $5, $6)
                 ON CONFLICT (symbol, price_ts) DO UPDATE
                 SET price = EXCLUDED.price, source = EXCLUDED.source, chain_id = EXCLUDED.chain_id",
            )
            .bind(p.id)
            .bind(&p.symbol)
            .bind(p.price)
            .bind(p.price_ts)
            .bind(&p.source)
            .bind(p.chain_id.unwrap_or(0) as i64)
            .execute(&mut *tx)
            .await?;
        }
        tx.commit().await?;
        Ok(())
    }

    async fn fetch_range(
        &self,
        symbol: &str,
        chain_id: Option<u64>,
        from: DateTime<Utc>,
        to: DateTime<Utc>,
    ) -> Result<Vec<PriceHistoryPoint>> {
        let rows = sqlx::query(
            "SELECT id, symbol, price, price_ts, source, chain_id
             FROM price_history
             WHERE symbol = $1 AND price_ts BETWEEN $2 AND $3 AND chain_id = $4
             ORDER BY price_ts ASC",
        )
        .bind(symbol)
        .bind(from)
        .bind(to)
        .bind(i64::try_from(chain_id.unwrap_or(0)).unwrap_or(0))
        .fetch_all(&self.pool)
        .await?;

        Ok(rows
            .into_iter()
            .map(|row| PriceHistoryPoint {
                id: row.try_get("id").unwrap_or_else(|_| Uuid::new_v4()),
                symbol: row.try_get("symbol").unwrap_or_default(),
                price: row.try_get::<f64, _>("price").unwrap_or(0.0),
                price_ts: row
                    .try_get("price_ts")
                    .unwrap_or_else(|_| DateTime::<Utc>::MIN_UTC),
                source: row
                    .try_get("source")
                    .unwrap_or_else(|_| "unknown".to_string()),
                chain_id: row
                    .try_get::<i64, _>("chain_id")
                    .ok()
                    .and_then(|v| v.try_into().ok()),
            })
            .collect())
    }

    async fn latest_timestamp(
        &self,
        symbol: &str,
        chain_id: Option<u64>,
    ) -> Result<Option<DateTime<Utc>>> {
        let row = sqlx::query(
            "SELECT price_ts FROM price_history WHERE symbol = $1 AND chain_id = $2 ORDER BY price_ts DESC LIMIT 1",
        )
        .bind(symbol)
        .bind(i64::try_from(chain_id.unwrap_or(0)).unwrap_or(0))
        .fetch_optional(&self.pool)
        .await?;
        Ok(row.map(|r| r.try_get("price_ts").unwrap()))
    }
}
