use anyhow::{Context, Result};
use async_trait::async_trait;
use chrono::{DateTime, Utc};
use domain::{Role, UserWallet};
use sqlx::{PgPool, Row};
use uuid::Uuid;

#[derive(Debug, Clone)]
pub struct UserProfileData {
    pub id: Uuid,
    pub primary_wallet: String,
    pub wallets: Vec<UserWallet>,
}

#[derive(Debug, Clone)]
pub struct AdminWalletData {
    pub id: Uuid,
    pub address: String,
    pub chain_id: u64,
    pub cached_role: Option<Role>,
    pub cached_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone)]
pub struct AdminUserData {
    pub id: Uuid,
    pub primary_wallet: String,
    pub wallets: Vec<AdminWalletData>,
}

#[async_trait]
pub trait UserRepository: Send + Sync {
    async fn find_profile(&self, user_id: Uuid) -> Result<Option<UserProfileData>>;
    async fn list_admin_users(&self) -> Result<Vec<AdminUserData>>;
    async fn set_primary_wallet(&self, user_id: Uuid, wallet_id: Uuid) -> Result<bool>;
}

#[derive(Clone)]
pub struct PostgresUserRepository {
    pool: PgPool,
}

impl PostgresUserRepository {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }
}

#[async_trait]
impl UserRepository for PostgresUserRepository {
    async fn find_profile(&self, user_id: Uuid) -> Result<Option<UserProfileData>> {
        let user_row = sqlx::query("SELECT id, primary_wallet FROM users WHERE id = $1")
            .bind(user_id)
            .fetch_optional(&self.pool)
            .await?;

        let Some(row) = user_row else {
            return Ok(None);
        };

        let primary_wallet: String = row
            .try_get("primary_wallet")
            .context("invalid primary_wallet column")?;

        let wallet_rows = sqlx::query(
            "SELECT id, address, chain_id FROM wallets WHERE user_id = $1 ORDER BY created_at ASC",
        )
        .bind(user_id)
        .fetch_all(&self.pool)
        .await?;

        let mut wallets = Vec::with_capacity(wallet_rows.len());
        for row in wallet_rows {
            let chain_id_i64: i64 = row.try_get("chain_id").context("invalid chain_id")?;
            let chain_id = u64::try_from(chain_id_i64)
                .map_err(|_| anyhow::anyhow!("invalid chain id {}", chain_id_i64))?;
            wallets.push(UserWallet {
                id: row.try_get("id")?,
                address: row.try_get("address")?,
                chain_id,
            });
        }

        Ok(Some(UserProfileData {
            id: user_id,
            primary_wallet,
            wallets,
        }))
    }

    async fn list_admin_users(&self) -> Result<Vec<AdminUserData>> {
        let rows = sqlx::query(
            r#"
            SELECT
                u.id as user_id,
                u.primary_wallet,
                w.id as wallet_id,
                w.address,
                w.chain_id,
                w.role_cache,
                w.role_cache_updated_at
            FROM users u
            LEFT JOIN wallets w ON w.user_id = u.id
            ORDER BY u.created_at ASC, w.created_at ASC
            "#,
        )
        .fetch_all(&self.pool)
        .await?;

        let mut users = Vec::<AdminUserData>::new();

        for row in rows {
            let user_id: Uuid = row.try_get("user_id")?;
            if !matches!(users.last(), Some(existing) if existing.id == user_id) {
                let primary_wallet: String = row
                    .try_get("primary_wallet")
                    .context("missing primary_wallet")?;
                users.push(AdminUserData {
                    id: user_id,
                    primary_wallet,
                    wallets: Vec::new(),
                });
            }

            let wallet_id: Option<Uuid> = row.try_get("wallet_id")?;
            if let (Some(wallet_id), Some(user_entry)) = (wallet_id, users.last_mut()) {
                let chain_id_i64: i64 =
                    row.try_get("chain_id").context("invalid wallet chain id")?;
                let chain_id = u64::try_from(chain_id_i64)
                    .map_err(|_| anyhow::anyhow!("invalid chain id {}", chain_id_i64))?;
                let role_value: Option<i16> = row.try_get("role_cache")?;
                let cached_role = role_value.map(|value| Role::from_u8(value as u8));
                let cached_at: Option<DateTime<Utc>> = row.try_get("role_cache_updated_at")?;
                user_entry.wallets.push(AdminWalletData {
                    id: wallet_id,
                    address: row.try_get("address").context("invalid wallet address")?,
                    chain_id,
                    cached_role,
                    cached_at,
                });
            }
        }

        Ok(users)
    }

    async fn set_primary_wallet(&self, user_id: Uuid, wallet_id: Uuid) -> Result<bool> {
        let result = sqlx::query(
            r#"
            UPDATE users u
            SET primary_wallet = w.address
            FROM wallets w
            WHERE u.id = $1 AND w.id = $2 AND w.user_id = u.id
            "#,
        )
        .bind(user_id)
        .bind(wallet_id)
        .execute(&self.pool)
        .await?;

        Ok(result.rows_affected() > 0)
    }
}
