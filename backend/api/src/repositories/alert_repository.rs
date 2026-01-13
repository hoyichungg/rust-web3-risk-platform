use anyhow::{Context, Result};
use async_trait::async_trait;
use chrono::{DateTime, Utc};
use domain::AlertRule;
use sqlx::{PgPool, Row};
use uuid::Uuid;

use serde::Serialize;

#[derive(Clone, Debug, Serialize)]
pub struct AlertTrigger {
    pub id: Uuid,
    pub rule_id: Uuid,
    pub wallet_id: Uuid,
    pub message: String,
    pub created_at: DateTime<Utc>,
}

#[async_trait]
pub trait AlertRepository: Send + Sync {
    async fn list_user_ids(&self) -> Result<Vec<Uuid>>;
    async fn list_rules(&self, user_id: Uuid) -> Result<Vec<AlertRule>>;
    async fn create_rule(&self, rule: &AlertRule) -> Result<()>;
    async fn update_rule(&self, rule: &AlertRule) -> Result<bool>;
    async fn delete_rule(&self, rule_id: Uuid, user_id: Uuid) -> Result<bool>;
    async fn insert_trigger(&self, rule_id: Uuid, wallet_id: Uuid, message: &str) -> Result<()>;
    async fn list_triggers(&self, user_id: Uuid, limit: i64) -> Result<Vec<AlertTrigger>>;
    async fn last_trigger_at(
        &self,
        rule_id: Uuid,
        wallet_id: Uuid,
    ) -> Result<Option<DateTime<Utc>>>;
}

#[derive(Clone)]
pub struct PostgresAlertRepository {
    pool: PgPool,
}

impl PostgresAlertRepository {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }
}

#[async_trait]
impl AlertRepository for PostgresAlertRepository {
    async fn list_user_ids(&self) -> Result<Vec<Uuid>> {
        let rows = sqlx::query("SELECT DISTINCT user_id FROM alert_rules")
            .fetch_all(&self.pool)
            .await?;
        rows.into_iter()
            .map(|row| row.try_get("user_id").context("invalid user_id column"))
            .collect()
    }

    async fn list_rules(&self, user_id: Uuid) -> Result<Vec<AlertRule>> {
        let rows = sqlx::query(
            "SELECT id, user_id, type, threshold, enabled, cooldown_secs FROM alert_rules
             WHERE user_id = $1
             ORDER BY created_at DESC",
        )
        .bind(user_id)
        .fetch_all(&self.pool)
        .await?;

        rows.into_iter()
            .map(|row| {
                Ok(AlertRule {
                    id: row.try_get("id")?,
                    user_id: row.try_get("user_id")?,
                    r#type: row.try_get("type")?,
                    threshold: row.try_get::<f64, _>("threshold")?,
                    enabled: row.try_get("enabled")?,
                    cooldown_secs: row.try_get::<i64, _>("cooldown_secs").unwrap_or(300),
                })
            })
            .collect()
    }

    async fn create_rule(&self, rule: &AlertRule) -> Result<()> {
        sqlx::query(
            "INSERT INTO alert_rules (id, user_id, type, threshold, enabled, cooldown_secs)
             VALUES ($1, $2, $3, $4, $5, $6)",
        )
        .bind(rule.id)
        .bind(rule.user_id)
        .bind(&rule.r#type)
        .bind(rule.threshold)
        .bind(rule.enabled)
        .bind(rule.cooldown_secs)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    async fn update_rule(&self, rule: &AlertRule) -> Result<bool> {
        let res = sqlx::query(
            "UPDATE alert_rules SET type = $3, threshold = $4, enabled = $5, cooldown_secs = $6
             WHERE id = $1 AND user_id = $2",
        )
        .bind(rule.id)
        .bind(rule.user_id)
        .bind(&rule.r#type)
        .bind(rule.threshold)
        .bind(rule.enabled)
        .bind(rule.cooldown_secs)
        .execute(&self.pool)
        .await?;
        Ok(res.rows_affected() > 0)
    }

    async fn delete_rule(&self, rule_id: Uuid, user_id: Uuid) -> Result<bool> {
        let res = sqlx::query("DELETE FROM alert_rules WHERE id = $1 AND user_id = $2")
            .bind(rule_id)
            .bind(user_id)
            .execute(&self.pool)
            .await?;
        Ok(res.rows_affected() > 0)
    }

    async fn insert_trigger(&self, rule_id: Uuid, wallet_id: Uuid, message: &str) -> Result<()> {
        sqlx::query(
            "INSERT INTO alert_triggers (id, rule_id, wallet_id, message)
             VALUES ($1, $2, $3, $4)",
        )
        .bind(Uuid::new_v4())
        .bind(rule_id)
        .bind(wallet_id)
        .bind(message)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    async fn list_triggers(&self, user_id: Uuid, limit: i64) -> Result<Vec<AlertTrigger>> {
        let rows = sqlx::query(
            r#"
            SELECT t.id, t.rule_id, t.wallet_id, t.message, t.created_at
            FROM alert_triggers t
            JOIN alert_rules r ON r.id = t.rule_id
            WHERE r.user_id = $1
            ORDER BY t.created_at DESC
            LIMIT $2
            "#,
        )
        .bind(user_id)
        .bind(limit.max(1))
        .fetch_all(&self.pool)
        .await?;

        rows.into_iter()
            .map(|row| {
                Ok(AlertTrigger {
                    id: row.try_get("id")?,
                    rule_id: row.try_get("rule_id")?,
                    wallet_id: row.try_get("wallet_id")?,
                    message: row.try_get("message")?,
                    created_at: row.try_get("created_at")?,
                })
            })
            .collect()
    }

    async fn last_trigger_at(
        &self,
        rule_id: Uuid,
        wallet_id: Uuid,
    ) -> Result<Option<DateTime<Utc>>> {
        let row = sqlx::query(
            "SELECT created_at FROM alert_triggers
             WHERE rule_id = $1 AND wallet_id = $2
             ORDER BY created_at DESC
             LIMIT 1",
        )
        .bind(rule_id)
        .bind(wallet_id)
        .fetch_optional(&self.pool)
        .await?;
        Ok(row.map(|r| {
            r.try_get("created_at")
                .unwrap_or_else(|_| DateTime::<Utc>::MIN_UTC)
        }))
    }
}
