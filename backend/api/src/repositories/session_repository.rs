use anyhow::{Context, Result};
use async_trait::async_trait;
use chrono::{DateTime, Utc};
use domain::SessionInfo;
use sqlx::{PgPool, Row};
use uuid::Uuid;

#[async_trait]
pub trait SessionRepository: Send + Sync {
    async fn list_all(&self) -> Result<Vec<SessionInfo>>;
    async fn revoke(&self, session_id: Uuid) -> Result<bool>;
}

#[derive(Clone)]
pub struct PostgresSessionRepository {
    pool: PgPool,
}

impl PostgresSessionRepository {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }
}

#[async_trait]
impl SessionRepository for PostgresSessionRepository {
    async fn list_all(&self) -> Result<Vec<SessionInfo>> {
        let rows = sqlx::query(
            r#"
            SELECT
                s.id,
                s.user_id,
                s.wallet_id,
                s.created_at,
                s.refreshed_at,
                s.expires_at,
                s.revoked_at,
                w.address as wallet_address,
                u.primary_wallet
            FROM user_sessions s
            JOIN wallets w ON w.id = s.wallet_id
            JOIN users u ON u.id = s.user_id
            ORDER BY s.created_at DESC
            "#,
        )
        .fetch_all(&self.pool)
        .await?;

        rows.into_iter()
            .map(|row| {
                let refreshed_at: DateTime<Utc> = row
                    .try_get("refreshed_at")
                    .context("invalid refreshed_at column")?;
                let expires_at: DateTime<Utc> = row
                    .try_get("expires_at")
                    .context("invalid expires_at column")?;
                let created_at: DateTime<Utc> = row
                    .try_get("created_at")
                    .context("invalid created_at column")?;
                let revoked_at: Option<DateTime<Utc>> = row
                    .try_get("revoked_at")
                    .context("invalid revoked_at column")?;
                Ok(SessionInfo {
                    id: row.try_get("id")?,
                    user_id: row.try_get("user_id")?,
                    wallet_id: row.try_get("wallet_id")?,
                    wallet_address: row.try_get("wallet_address")?,
                    primary_wallet: row.try_get("primary_wallet")?,
                    created_at,
                    refreshed_at,
                    expires_at,
                    revoked_at,
                })
            })
            .collect()
    }

    async fn revoke(&self, session_id: Uuid) -> Result<bool> {
        let result = sqlx::query(
            "UPDATE user_sessions SET revoked_at = NOW() WHERE id = $1 AND revoked_at IS NULL",
        )
        .bind(session_id)
        .execute(&self.pool)
        .await?;
        Ok(result.rows_affected() > 0)
    }
}
