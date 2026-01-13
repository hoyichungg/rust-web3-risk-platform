use anyhow::{Context, Result};
use async_trait::async_trait;
use domain::Wallet;
use sqlx::{PgPool, Row};
use uuid::Uuid;

#[async_trait]
pub trait WalletRepository: Send + Sync {
    async fn list_by_user(&self, user_id: Uuid) -> Result<Vec<Wallet>>;
    async fn list_all(&self) -> Result<Vec<Wallet>>;
    async fn list_by_chain(&self, chain_id: u64) -> Result<Vec<Wallet>>;
    async fn create_wallet(&self, user_id: Uuid, address: &str, chain_id: u64) -> Result<Wallet>;
    async fn delete_wallet(&self, user_id: Uuid, wallet_id: Uuid) -> Result<bool>;
    async fn find_by_id(&self, wallet_id: Uuid) -> Result<Option<Wallet>>;
}

#[derive(Clone)]
pub struct PostgresWalletRepository {
    pool: PgPool,
}

impl PostgresWalletRepository {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    fn row_to_wallet(row: &sqlx::postgres::PgRow) -> Result<Wallet> {
        let chain_id_i64: i64 = row.try_get("chain_id").context("invalid chain_id column")?;
        let chain_id = u64::try_from(chain_id_i64)
            .map_err(|_| anyhow::anyhow!("invalid chain id {}", chain_id_i64))?;

        Ok(Wallet {
            id: row.try_get("id")?,
            user_id: row.try_get("user_id")?,
            address: row.try_get("address")?,
            chain_id,
        })
    }
}

#[async_trait]
impl WalletRepository for PostgresWalletRepository {
    async fn list_by_user(&self, user_id: Uuid) -> Result<Vec<Wallet>> {
        let rows = sqlx::query("SELECT id, user_id, address, chain_id FROM wallets WHERE user_id = $1 ORDER BY created_at ASC")
            .bind(user_id)
            .fetch_all(&self.pool)
            .await?;
        rows.into_iter()
            .map(|row| Self::row_to_wallet(&row))
            .collect()
    }

    async fn list_all(&self) -> Result<Vec<Wallet>> {
        let rows = sqlx::query("SELECT id, user_id, address, chain_id FROM wallets")
            .fetch_all(&self.pool)
            .await?;
        rows.into_iter()
            .map(|row| Self::row_to_wallet(&row))
            .collect()
    }

    async fn list_by_chain(&self, chain_id: u64) -> Result<Vec<Wallet>> {
        let rows =
            sqlx::query("SELECT id, user_id, address, chain_id FROM wallets WHERE chain_id = $1")
                .bind(i64::try_from(chain_id).context("chain_id too large")?)
                .fetch_all(&self.pool)
                .await?;
        rows.into_iter()
            .map(|row| Self::row_to_wallet(&row))
            .collect()
    }

    async fn create_wallet(&self, user_id: Uuid, address: &str, chain_id: u64) -> Result<Wallet> {
        let wallet_id = Uuid::new_v4();
        sqlx::query(
            "INSERT INTO wallets (id, user_id, address, chain_id) VALUES ($1, $2, LOWER($3), $4)",
        )
        .bind(wallet_id)
        .bind(user_id)
        .bind(address)
        .bind(i64::try_from(chain_id).context("chain_id too large")?)
        .execute(&self.pool)
        .await?;

        Ok(Wallet {
            id: wallet_id,
            user_id,
            address: address.to_lowercase(),
            chain_id,
        })
    }

    async fn delete_wallet(&self, user_id: Uuid, wallet_id: Uuid) -> Result<bool> {
        let result = sqlx::query("DELETE FROM wallets WHERE id = $1 AND user_id = $2")
            .bind(wallet_id)
            .bind(user_id)
            .execute(&self.pool)
            .await?;
        Ok(result.rows_affected() > 0)
    }

    async fn find_by_id(&self, wallet_id: Uuid) -> Result<Option<Wallet>> {
        let row = sqlx::query("SELECT id, user_id, address, chain_id FROM wallets WHERE id = $1")
            .bind(wallet_id)
            .fetch_optional(&self.pool)
            .await?;
        Ok(row.map(|row| Self::row_to_wallet(&row)).transpose()?)
    }
}
