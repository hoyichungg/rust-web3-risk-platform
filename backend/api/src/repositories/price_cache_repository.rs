use anyhow::Result;
use async_trait::async_trait;
use chrono::{DateTime, Utc};
use sqlx::{PgPool, Row};

#[async_trait]
pub trait PriceCacheRepository: Send + Sync {
    async fn get_cached_price(&self, symbol: &str, now: DateTime<Utc>) -> Result<Option<f64>>;
    async fn upsert_price(
        &self,
        symbol: &str,
        price: f64,
        ttl_seconds: i64,
        source: &str,
    ) -> Result<()>;
}

#[derive(Clone)]
pub struct PostgresPriceCacheRepository {
    pool: PgPool,
}

impl PostgresPriceCacheRepository {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }
}

#[async_trait]
impl PriceCacheRepository for PostgresPriceCacheRepository {
    async fn get_cached_price(&self, symbol: &str, now: DateTime<Utc>) -> Result<Option<f64>> {
        let row =
            sqlx::query("SELECT price FROM price_cache WHERE symbol = $1 AND expires_at > $2")
                .bind(symbol)
                .bind(now)
                .fetch_optional(&self.pool)
                .await?;

        Ok(row.map(|r| r.try_get("price").unwrap_or(0.0)))
    }

    async fn upsert_price(
        &self,
        symbol: &str,
        price: f64,
        ttl_seconds: i64,
        source: &str,
    ) -> Result<()> {
        sqlx::query(
            "INSERT INTO price_cache (symbol, price, fetched_at, expires_at, source)
             VALUES ($1, $2, NOW(), NOW() + ($3 || ' seconds')::interval, $4)
             ON CONFLICT (symbol) DO UPDATE
             SET price = EXCLUDED.price,
                 fetched_at = NOW(),
                 expires_at = EXCLUDED.expires_at,
                 source = EXCLUDED.source,
                 updated_at = NOW()",
        )
        .bind(symbol)
        .bind(price)
        .bind(ttl_seconds.max(1))
        .bind(source)
        .execute(&self.pool)
        .await?;
        Ok(())
    }
}
