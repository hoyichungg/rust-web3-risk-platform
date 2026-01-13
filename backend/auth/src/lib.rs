use std::collections::HashMap;
use std::str::FromStr;
use std::sync::Arc;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use async_trait::async_trait;
use chrono::{DateTime, Duration as ChronoDuration, Utc};
use domain::{LoginRequest, LoginResponse, NonceResponse, Role, Wallet};
use ethers::contract::abigen;
use ethers::providers::{Http, Middleware, Provider};
use ethers::types::{Address, Signature};
use ethers::utils::hash_message;
use jsonwebtoken::{DecodingKey, EncodingKey, Header, Validation, decode, encode};
use rand::{Rng, distributions::Alphanumeric, thread_rng};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use sqlx::{PgPool, Row};
use thiserror::Error;
use tracing::{debug, warn};
use uuid::Uuid;

const NONCE_TTL: Duration = Duration::from_secs(300);
const SIWE_SUFFIX: &str = " wants you to sign in with your Ethereum account:";

abigen!(
    RoleManagerContract,
    r#"[
        function getRole(address user) external view returns (uint8)
    ]"#
);

#[derive(Debug, Clone)]
pub struct AuthConfig {
    pub role_manager_address: String,
    pub jwt_secret: String,
    pub jwt_audience: String,
    pub jwt_issuer: String,
    pub siwe_domain: String,
    pub siwe_uri: String,
    pub siwe_statement: String,
    pub access_token_ttl: ChronoDuration,
    pub refresh_token_ttl: ChronoDuration,
    pub role_cache_ttl_default: ChronoDuration,
    pub role_cache_ttl_overrides: std::collections::HashMap<u64, ChronoDuration>,
}

impl Default for AuthConfig {
    fn default() -> Self {
        Self {
            role_manager_address: String::new(),
            jwt_secret: "dev-secret".to_string(),
            jwt_audience: "rw3p".to_string(),
            jwt_issuer: "rw3p-api".to_string(),
            siwe_domain: "localhost:3000".to_string(),
            siwe_uri: "http://localhost:3000".to_string(),
            siwe_statement: "Sign in to Rust Web3 Risk Platform".to_string(),
            access_token_ttl: ChronoDuration::seconds(900),
            refresh_token_ttl: ChronoDuration::seconds(604800),
            role_cache_ttl_default: ChronoDuration::seconds(300),
            role_cache_ttl_overrides: std::collections::HashMap::new(),
        }
    }
}

#[derive(Debug, Error)]
pub enum AuthError {
    #[error("invalid signature")]
    InvalidSignature,
    #[error("nonce not found")]
    NonceNotFound,
    #[error("nonce expired")]
    NonceExpired,
    #[error("invalid token")]
    InvalidToken,
    #[error("invalid refresh token")]
    RefreshTokenInvalid,
    #[error("invalid SIWE message")]
    InvalidMessage,
    #[error("wallet already linked")]
    WalletAlreadyLinked,
    #[error("internal error: {0}")]
    Internal(String),
}

pub type AuthResult<T> = Result<T, AuthError>;

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct JwtClaims {
    pub sub: String,
    pub role: Role,
    pub aud: String,
    pub iss: String,
    pub exp: usize,
    pub iat: usize,
    pub session_id: Uuid,
    pub user_id: Uuid,
    pub wallet_id: Uuid,
}

struct SessionTokens {
    session_id: Uuid,
    refresh_token: String,
}

#[async_trait]
pub trait AuthService: Send + Sync {
    async fn issue_nonce(&self) -> AuthResult<NonceResponse>;
    async fn login(&self, payload: LoginRequest) -> AuthResult<LoginResponse>;
    async fn validate_token(&self, token: &str) -> AuthResult<JwtClaims>;
    async fn logout(&self, session_id: Uuid) -> AuthResult<()>;
    async fn refresh_session(&self, refresh_token: &str) -> AuthResult<LoginResponse>;
    async fn link_wallet(&self, user_id: Uuid, payload: LoginRequest) -> AuthResult<Wallet>;
    async fn refresh_role_cache(&self, address: &str, chain_id: u64) -> AuthResult<Role>;
}

#[derive(Clone)]
pub struct OnChainAuthService<M>
where
    M: Middleware + 'static,
{
    config: AuthConfig,
    pool: PgPool,
    role_manager: Option<RoleManagerContract<M>>,
}

impl OnChainAuthService<Provider<Http>> {
    pub fn new(
        config: AuthConfig,
        provider: Arc<Provider<Http>>,
        pool: PgPool,
    ) -> AuthResult<Self> {
        let role_manager = parse_contract_address(&config.role_manager_address)?
            .map(|address| RoleManagerContract::new(address, provider));

        if role_manager.is_none() {
            warn!("ROLE_MANAGER_ADDRESS not set. Falling back to Viewer role for all logins");
        }

        Ok(Self {
            config,
            pool,
            role_manager,
        })
    }
}

impl<M> OnChainAuthService<M>
where
    M: Middleware + 'static,
{
    fn generate_refresh_token() -> String {
        thread_rng()
            .sample_iter(&Alphanumeric)
            .take(64)
            .map(char::from)
            .collect()
    }

    fn hash_refresh_token(token: &str) -> String {
        let mut hasher = Sha256::new();
        hasher.update(token.as_bytes());
        hex::encode(hasher.finalize())
    }

    fn role_cache_ttl_for_chain(&self, chain_id: u64) -> ChronoDuration {
        self.config
            .role_cache_ttl_overrides
            .get(&chain_id)
            .cloned()
            .unwrap_or(self.config.role_cache_ttl_default)
    }

    fn build_jwt(
        &self,
        wallet_address: &str,
        role: Role,
        session_id: Uuid,
        user_id: Uuid,
        wallet_id: Uuid,
        expires_at: DateTime<Utc>,
    ) -> AuthResult<String> {
        let issued_at = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_err(|err| AuthError::Internal(format!("time error: {err}")))?;
        let iat = issued_at.as_secs() as usize;
        let exp = expires_at
            .timestamp()
            .try_into()
            .map_err(|err| AuthError::Internal(format!("token expiration overflow: {err}")))?;

        let claims = JwtClaims {
            sub: wallet_address.to_lowercase(),
            role,
            aud: self.config.jwt_audience.clone(),
            iss: self.config.jwt_issuer.clone(),
            exp,
            iat,
            session_id,
            user_id,
            wallet_id,
        };

        encode(
            &Header::default(),
            &claims,
            &EncodingKey::from_secret(self.config.jwt_secret.as_bytes()),
        )
        .map_err(|err| AuthError::Internal(format!("failed to encode jwt: {err}")))
    }

    fn verify_signature(&self, message: &str, signature: &str) -> AuthResult<Address> {
        let signature = signature
            .parse::<Signature>()
            .map_err(|_| AuthError::InvalidSignature)?;
        let digest = hash_message(message);
        signature
            .recover(digest)
            .map_err(|_| AuthError::InvalidSignature)
    }

    async fn resolve_role_from_chain(&self, wallet: Address) -> AuthResult<Role> {
        if let Some(contract) = &self.role_manager {
            let role_value = contract
                .get_role(wallet)
                .call()
                .await
                .map_err(|err| AuthError::Internal(format!("role lookup failed: {err}")))?;
            Ok(Role::from_u8(role_value))
        } else {
            Ok(Role::Viewer)
        }
    }

    async fn resolve_role_cached(&self, wallet: Address, chain_id: u64) -> AuthResult<Role> {
        let address = format!("{:#x}", wallet).to_lowercase();
        if let Some((role, updated_at)) = self.load_role_cache(&address).await? {
            if updated_at + self.role_cache_ttl_for_chain(chain_id) > Utc::now() {
                return Ok(role);
            }
        }
        let role = self.resolve_role_from_chain(wallet).await?;
        self.store_role_cache(&address, role).await?;
        Ok(role)
    }

    async fn load_role_cache(&self, address: &str) -> AuthResult<Option<(Role, DateTime<Utc>)>> {
        let row = sqlx::query(
            "SELECT role_cache, role_cache_updated_at FROM wallets WHERE LOWER(address) = LOWER($1) LIMIT 1",
        )
        .bind(address)
        .fetch_optional(&self.pool)
        .await
        .map_err(|err| AuthError::Internal(format!("failed to load role cache: {err}")))?;

        if let Some(row) = row {
            let role_value: Option<i16> = row
                .try_get("role_cache")
                .map_err(|err| AuthError::Internal(format!("invalid role cache row: {err}")))?;
            let updated_at: Option<DateTime<Utc>> = row
                .try_get("role_cache_updated_at")
                .map_err(|err| AuthError::Internal(format!("invalid role cache row: {err}")))?;
            if let (Some(value), Some(updated_at)) = (role_value, updated_at) {
                let role = Role::from_u8(value as u8);
                return Ok(Some((role, updated_at)));
            }
        }

        Ok(None)
    }

    async fn store_role_cache(&self, address: &str, role: Role) -> AuthResult<()> {
        let role_value: i16 = role.as_u8() as i16;
        sqlx::query(
            "UPDATE wallets SET role_cache = $2, role_cache_updated_at = NOW() WHERE LOWER(address) = LOWER($1)",
        )
        .bind(address)
        .bind(role_value)
        .execute(&self.pool)
        .await
        .map(|_| ())
        .map_err(|err| AuthError::Internal(format!("failed to persist role cache: {err}")))
    }

    async fn find_wallet_owner(&self, address: &str) -> AuthResult<Option<Uuid>> {
        let row =
            sqlx::query("SELECT user_id FROM wallets WHERE LOWER(address) = LOWER($1) LIMIT 1")
                .bind(address)
                .fetch_optional(&self.pool)
                .await
                .map_err(|err| {
                    AuthError::Internal(format!("failed to lookup wallet owner: {err}"))
                })?;
        if let Some(row) = row {
            let user_id = row
                .try_get("user_id")
                .map_err(|err| AuthError::Internal(format!("invalid wallet owner row: {err}")))?;
            Ok(Some(user_id))
        } else {
            Ok(None)
        }
    }

    async fn load_wallet_by_address(&self, address: &str) -> AuthResult<Option<Wallet>> {
        let row = sqlx::query("SELECT id, user_id, address, chain_id FROM wallets WHERE LOWER(address) = LOWER($1) LIMIT 1")
            .bind(address)
            .fetch_optional(&self.pool)
            .await
            .map_err(|err| AuthError::Internal(format!("failed to load wallet: {err}")))?;

        if let Some(row) = row {
            let chain_id_i64: i64 = row
                .try_get("chain_id")
                .map_err(|err| AuthError::Internal(format!("invalid wallet row: {err}")))?;
            let chain_id = u64::try_from(chain_id_i64)
                .map_err(|err| AuthError::Internal(format!("invalid chain id: {err}")))?;
            return Ok(Some(Wallet {
                id: row
                    .try_get("id")
                    .map_err(|err| AuthError::Internal(format!("invalid wallet row: {err}")))?,
                user_id: row
                    .try_get("user_id")
                    .map_err(|err| AuthError::Internal(format!("invalid wallet row: {err}")))?,
                address: row
                    .try_get("address")
                    .map_err(|err| AuthError::Internal(format!("invalid wallet row: {err}")))?,
                chain_id,
            }));
        }

        Ok(None)
    }

    async fn persist_nonce(&self, nonce: &str, expires_at: DateTime<Utc>) -> AuthResult<()> {
        sqlx::query(
            "INSERT INTO auth_nonces (nonce, expires_at) VALUES ($1, $2)
             ON CONFLICT (nonce) DO UPDATE SET expires_at = EXCLUDED.expires_at, consumed_at = NULL",
        )
        .bind(nonce)
        .bind(expires_at)
        .execute(&self.pool)
        .await
        .map(|_| ())
        .map_err(|err| AuthError::Internal(format!("failed to persist nonce: {err}")))
    }

    async fn ensure_nonce_valid(&self, nonce: &str) -> AuthResult<()> {
        let row = sqlx::query("SELECT expires_at, consumed_at FROM auth_nonces WHERE nonce = $1")
            .bind(nonce)
            .fetch_optional(&self.pool)
            .await
            .map_err(|err| AuthError::Internal(format!("failed to load nonce: {err}")))?;

        let Some(row) = row else {
            return Err(AuthError::NonceNotFound);
        };

        let expires_at: DateTime<Utc> = row
            .try_get("expires_at")
            .map_err(|err| AuthError::Internal(format!("invalid nonce row: {err}")))?;
        let consumed_at: Option<DateTime<Utc>> = row
            .try_get("consumed_at")
            .map_err(|err| AuthError::Internal(format!("invalid nonce row: {err}")))?;

        if consumed_at.is_some() {
            return Err(AuthError::NonceExpired);
        }
        if expires_at < Utc::now() {
            return Err(AuthError::NonceExpired);
        }

        Ok(())
    }

    async fn consume_nonce(&self, nonce: &str) -> AuthResult<()> {
        let result = sqlx::query(
            "UPDATE auth_nonces SET consumed_at = NOW() WHERE nonce = $1 AND consumed_at IS NULL",
        )
        .bind(nonce)
        .execute(&self.pool)
        .await
        .map_err(|err| AuthError::Internal(format!("failed to consume nonce: {err}")))?;

        if result.rows_affected() == 0 {
            return Err(AuthError::NonceExpired);
        }
        Ok(())
    }

    async fn upsert_user_wallet(&self, wallet: Address, chain_id: u64) -> AuthResult<(Uuid, Uuid)> {
        let address_str = format!("{:#x}", wallet).to_lowercase();

        if let Some(row) =
            sqlx::query("SELECT id, user_id FROM wallets WHERE LOWER(address) = LOWER($1) LIMIT 1")
                .bind(&address_str)
                .fetch_optional(&self.pool)
                .await
                .map_err(|err| AuthError::Internal(format!("failed to query wallet: {err}")))?
        {
            let wallet_id: Uuid = row
                .try_get("id")
                .map_err(|err| AuthError::Internal(format!("invalid wallet row: {err}")))?;
            let user_id: Uuid = row
                .try_get("user_id")
                .map_err(|err| AuthError::Internal(format!("invalid wallet row: {err}")))?;
            return Ok((user_id, wallet_id));
        }

        let user_id = Uuid::new_v4();
        let wallet_id = Uuid::new_v4();
        let chain_id_i64: i64 = chain_id
            .try_into()
            .map_err(|err| AuthError::Internal(format!("invalid chain id: {err}")))?;

        let mut tx = self
            .pool
            .begin()
            .await
            .map_err(|err| AuthError::Internal(format!("failed to start tx: {err}")))?;

        sqlx::query("INSERT INTO users (id, primary_wallet) VALUES ($1, $2)")
            .bind(user_id)
            .bind(&address_str)
            .execute(&mut *tx)
            .await
            .map_err(|err| AuthError::Internal(format!("failed to insert user: {err}")))?;

        sqlx::query("INSERT INTO wallets (id, user_id, address, chain_id) VALUES ($1, $2, $3, $4)")
            .bind(wallet_id)
            .bind(user_id)
            .bind(&address_str)
            .bind(chain_id_i64)
            .execute(&mut *tx)
            .await
            .map_err(|err| AuthError::Internal(format!("failed to insert wallet: {err}")))?;

        tx.commit()
            .await
            .map_err(|err| AuthError::Internal(format!("failed to commit user: {err}")))?;

        Ok((user_id, wallet_id))
    }

    async fn create_session(&self, user_id: Uuid, wallet_id: Uuid) -> AuthResult<SessionTokens> {
        let session_id = Uuid::new_v4();
        let refresh_token = Self::generate_refresh_token();
        let refresh_hash = Self::hash_refresh_token(&refresh_token);
        let refresh_expires_at = Utc::now() + self.config.refresh_token_ttl;
        sqlx::query(
            "INSERT INTO user_sessions (id, user_id, wallet_id, expires_at, refresh_token_hash, refreshed_at)
             VALUES ($1, $2, $3, $4, $5, NOW())",
        )
        .bind(session_id)
        .bind(user_id)
        .bind(wallet_id)
        .bind(refresh_expires_at)
        .bind(refresh_hash)
        .execute(&self.pool)
        .await
        .map_err(|err| AuthError::Internal(format!("failed to store session: {err}")))?;
        Ok(SessionTokens {
            session_id,
            refresh_token,
        })
    }

    async fn ensure_session_active(&self, session_id: Uuid) -> AuthResult<()> {
        let row = sqlx::query("SELECT expires_at, revoked_at FROM user_sessions WHERE id = $1")
            .bind(session_id)
            .fetch_optional(&self.pool)
            .await
            .map_err(|err| AuthError::Internal(format!("failed to load session: {err}")))?;

        let Some(row) = row else {
            return Err(AuthError::InvalidToken);
        };

        let expires_at: DateTime<Utc> = row
            .try_get("expires_at")
            .map_err(|err| AuthError::Internal(format!("invalid session row: {err}")))?;
        let revoked_at: Option<DateTime<Utc>> = row
            .try_get("revoked_at")
            .map_err(|err| AuthError::Internal(format!("invalid session row: {err}")))?;

        if revoked_at.is_some() || expires_at < Utc::now() {
            return Err(AuthError::InvalidToken);
        }

        Ok(())
    }

    async fn revoke_session(&self, session_id: Uuid) -> AuthResult<()> {
        sqlx::query("UPDATE user_sessions SET revoked_at = NOW() WHERE id = $1")
            .bind(session_id)
            .execute(&self.pool)
            .await
            .map_err(|err| AuthError::Internal(format!("failed to revoke session: {err}")))?;
        Ok(())
    }

    async fn parse_siwe_message(&self, message: &str) -> AuthResult<ParsedSiweMessage> {
        ParsedSiweMessage::parse(message)
    }

    fn validate_siwe_context(&self, parsed: &ParsedSiweMessage) -> AuthResult<()> {
        if !self.config.siwe_domain.is_empty()
            && parsed.domain.to_lowercase() != self.config.siwe_domain.to_lowercase()
        {
            return Err(AuthError::InvalidMessage);
        }

        if !self.config.siwe_uri.is_empty()
            && parsed.uri.to_lowercase() != self.config.siwe_uri.to_lowercase()
        {
            return Err(AuthError::InvalidMessage);
        }

        if !self.config.siwe_statement.is_empty() {
            let expected = self.config.siwe_statement.trim();
            let actual = parsed.statement.as_deref().unwrap_or("").trim();
            if !expected.is_empty() && expected != actual {
                return Err(AuthError::InvalidMessage);
            }
        }

        let now = Utc::now();
        let ttl = chrono::Duration::from_std(NONCE_TTL)
            .unwrap_or_else(|_| chrono::Duration::seconds(300));
        if parsed.issued_at + ttl < now {
            return Err(AuthError::NonceExpired);
        }

        Ok(())
    }
}

#[async_trait]
impl<M> AuthService for OnChainAuthService<M>
where
    M: Middleware + 'static,
{
    async fn issue_nonce(&self) -> AuthResult<NonceResponse> {
        let nonce = Uuid::new_v4().simple().to_string();
        let expires_at = Utc::now()
            + chrono::Duration::from_std(NONCE_TTL)
                .unwrap_or_else(|_| chrono::Duration::seconds(300));
        self.persist_nonce(&nonce, expires_at).await?;
        Ok(NonceResponse { nonce })
    }

    async fn login(&self, payload: LoginRequest) -> AuthResult<LoginResponse> {
        let parsed = self.parse_siwe_message(&payload.message).await?;
        self.validate_siwe_context(&parsed)?;
        self.ensure_nonce_valid(&parsed.nonce).await?;

        let recovered = self.verify_signature(&payload.message, &payload.signature)?;
        if recovered != parsed.address {
            return Err(AuthError::InvalidSignature);
        }

        let role = self
            .resolve_role_cached(parsed.address, parsed.chain_id)
            .await?;
        let (user_id, wallet_id) = self
            .upsert_user_wallet(parsed.address, parsed.chain_id)
            .await?;
        let address = format!("{:#x}", parsed.address).to_lowercase();
        let access_expires_at = Utc::now() + self.config.access_token_ttl;
        let refresh_tokens = self.create_session(user_id, wallet_id).await?;
        self.store_role_cache(&address, role).await.ok();
        self.consume_nonce(&parsed.nonce).await?;

        debug!(
            "login ok wallet={} role={:?}",
            format!("{:#x}", parsed.address),
            role
        );

        let token = self.build_jwt(
            &address,
            role,
            refresh_tokens.session_id,
            user_id,
            wallet_id,
            access_expires_at,
        )?;
        Ok(LoginResponse {
            token,
            refresh_token: refresh_tokens.refresh_token,
            role,
        })
    }

    async fn validate_token(&self, token: &str) -> AuthResult<JwtClaims> {
        let mut validation = Validation::new(jsonwebtoken::Algorithm::HS256);
        validation.set_audience(&[self.config.jwt_audience.clone()]);
        validation.iss = Some(
            std::iter::once(self.config.jwt_issuer.clone())
                .collect::<std::collections::HashSet<String>>(),
        );

        let claims = decode::<JwtClaims>(
            token,
            &DecodingKey::from_secret(self.config.jwt_secret.as_bytes()),
            &validation,
        )
        .map(|data| data.claims)
        .map_err(|_| AuthError::InvalidToken)?;

        self.ensure_session_active(claims.session_id).await?;
        Ok(claims)
    }

    async fn logout(&self, session_id: Uuid) -> AuthResult<()> {
        self.revoke_session(session_id).await
    }

    async fn refresh_session(&self, refresh_token: &str) -> AuthResult<LoginResponse> {
        let refresh_hash = Self::hash_refresh_token(refresh_token);
        let mut tx = self
            .pool
            .begin()
            .await
            .map_err(|err| AuthError::Internal(format!("failed to start refresh tx: {err}")))?;

        let session_row = sqlx::query(
            "SELECT id, user_id, wallet_id, expires_at, revoked_at FROM user_sessions
             WHERE refresh_token_hash = $1 FOR UPDATE",
        )
        .bind(&refresh_hash)
        .fetch_optional(&mut *tx)
        .await
        .map_err(|err| AuthError::Internal(format!("failed to load session: {err}")))?;

        let Some(row) = session_row else {
            return Err(AuthError::RefreshTokenInvalid);
        };

        let revoked_at: Option<DateTime<Utc>> = row
            .try_get("revoked_at")
            .map_err(|err| AuthError::Internal(format!("invalid session row: {err}")))?;
        if revoked_at.is_some() {
            return Err(AuthError::RefreshTokenInvalid);
        }

        let expires_at: DateTime<Utc> = row
            .try_get("expires_at")
            .map_err(|err| AuthError::Internal(format!("invalid session row: {err}")))?;
        if expires_at < Utc::now() {
            return Err(AuthError::RefreshTokenInvalid);
        }

        let session_id: Uuid = row
            .try_get("id")
            .map_err(|err| AuthError::Internal(format!("invalid session row: {err}")))?;
        let user_id: Uuid = row
            .try_get("user_id")
            .map_err(|err| AuthError::Internal(format!("invalid session row: {err}")))?;
        let wallet_id: Uuid = row
            .try_get("wallet_id")
            .map_err(|err| AuthError::Internal(format!("invalid session row: {err}")))?;

        // Revoke the old session so the previous JWT/refresh pair can no longer be used.
        sqlx::query("UPDATE user_sessions SET revoked_at = NOW() WHERE id = $1")
            .bind(session_id)
            .execute(&mut *tx)
            .await
            .map_err(|err| AuthError::Internal(format!("failed to revoke session: {err}")))?;

        // Insert a brand new session with a new id + refresh token (rolling sessions).
        let new_session_id = Uuid::new_v4();
        let new_refresh_token = Self::generate_refresh_token();
        let new_refresh_hash = Self::hash_refresh_token(&new_refresh_token);
        let new_expires_at = Utc::now() + self.config.refresh_token_ttl;

        sqlx::query(
            "INSERT INTO user_sessions (id, user_id, wallet_id, expires_at, refresh_token_hash, refreshed_at)
             VALUES ($1, $2, $3, $4, $5, NOW())",
        )
        .bind(new_session_id)
        .bind(user_id)
        .bind(wallet_id)
        .bind(new_expires_at)
        .bind(new_refresh_hash)
        .execute(&mut *tx)
        .await
        .map_err(|err| AuthError::Internal(format!("failed to create rotated session: {err}")))?;

        tx.commit()
            .await
            .map_err(|err| AuthError::Internal(format!("failed to commit refresh: {err}")))?;

        let wallet_row = sqlx::query("SELECT address, chain_id FROM wallets WHERE id = $1")
            .bind(wallet_id)
            .fetch_optional(&self.pool)
            .await
            .map_err(|err| AuthError::Internal(format!("failed to load wallet: {err}")))?;

        let Some(wallet_row) = wallet_row else {
            return Err(AuthError::Internal("wallet not found for session".into()));
        };

        let wallet_address: String = wallet_row
            .try_get("address")
            .map_err(|err| AuthError::Internal(format!("invalid wallet row: {err}")))?;
        let chain_id_i64: i64 = wallet_row
            .try_get("chain_id")
            .map_err(|err| AuthError::Internal(format!("invalid wallet row: {err}")))?;
        let chain_id = u64::try_from(chain_id_i64)
            .map_err(|err| AuthError::Internal(format!("invalid chain id: {err}")))?;
        let parsed_address = Address::from_str(&wallet_address)
            .map_err(|_| AuthError::Internal("invalid wallet address stored".into()))?;
        let role = self.resolve_role_cached(parsed_address, chain_id).await?;
        self.store_role_cache(&wallet_address, role).await.ok();

        let access_expires = Utc::now() + self.config.access_token_ttl;
        let token = self.build_jwt(
            &wallet_address,
            role,
            new_session_id,
            user_id,
            wallet_id,
            access_expires,
        )?;

        Ok(LoginResponse {
            token,
            refresh_token: new_refresh_token,
            role,
        })
    }

    async fn link_wallet(&self, user_id: Uuid, payload: LoginRequest) -> AuthResult<Wallet> {
        let parsed = self.parse_siwe_message(&payload.message).await?;
        self.validate_siwe_context(&parsed)?;
        self.ensure_nonce_valid(&parsed.nonce).await?;

        let recovered = self.verify_signature(&payload.message, &payload.signature)?;
        if recovered != parsed.address {
            return Err(AuthError::InvalidSignature);
        }

        let address = format!("{:#x}", parsed.address).to_lowercase();
        if let Some(owner_id) = self.find_wallet_owner(&address).await? {
            if owner_id != user_id {
                return Err(AuthError::WalletAlreadyLinked);
            }
            self.consume_nonce(&parsed.nonce).await?;
            if let Some(wallet) = self.load_wallet_by_address(&address).await? {
                return Ok(wallet);
            }
            return Err(AuthError::Internal("wallet missing after lookup".into()));
        }

        let wallet_id = Uuid::new_v4();
        let chain_id_i64: i64 = parsed
            .chain_id
            .try_into()
            .map_err(|err| AuthError::Internal(format!("invalid chain id: {err}")))?;

        sqlx::query("INSERT INTO wallets (id, user_id, address, chain_id) VALUES ($1, $2, $3, $4)")
            .bind(wallet_id)
            .bind(user_id)
            .bind(&address)
            .bind(chain_id_i64)
            .execute(&self.pool)
            .await
            .map_err(|err| AuthError::Internal(format!("failed to link wallet: {err}")))?;

        // 若尚未設定 primary_wallet，將新綁定的地址作為 primary。
        sqlx::query(
            "UPDATE users SET primary_wallet = $2 WHERE id = $1 AND (primary_wallet IS NULL OR primary_wallet = '')",
        )
        .bind(user_id)
        .bind(&address)
        .execute(&self.pool)
        .await
        .ok();

        let role = self
            .resolve_role_cached(parsed.address, parsed.chain_id)
            .await?;
        self.store_role_cache(&address, role).await.ok();
        self.consume_nonce(&parsed.nonce).await?;

        Ok(Wallet {
            id: wallet_id,
            user_id,
            address,
            chain_id: parsed.chain_id,
        })
    }

    async fn refresh_role_cache(&self, address: &str, _chain_id: u64) -> AuthResult<Role> {
        let parsed_address = Address::from_str(address).map_err(|_| AuthError::InvalidMessage)?; // reuse error type
        let role = self.resolve_role_from_chain(parsed_address).await?;
        self.store_role_cache(address, role).await?;

        // Also bump role_cache_updated_at by capturing ttl logic via store cache.
        // Return the latest role for caller usage.
        Ok(role)
    }
}

fn parse_contract_address(value: &str) -> AuthResult<Option<Address>> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        return Ok(None);
    }

    let lower = trimmed.to_lowercase();
    if lower == "0x0000000000000000000000000000000000000000" {
        return Ok(None);
    }

    Address::from_str(trimmed)
        .map(Some)
        .map_err(|err| AuthError::Internal(format!("invalid RoleManager address: {err}")))
}

struct ParsedSiweMessage {
    domain: String,
    address: Address,
    statement: Option<String>,
    uri: String,
    chain_id: u64,
    nonce: String,
    issued_at: DateTime<Utc>,
}

impl ParsedSiweMessage {
    fn parse(message: &str) -> AuthResult<Self> {
        let mut lines = message.lines();

        let header = lines.next().ok_or(AuthError::InvalidMessage)?;
        let domain = header
            .strip_suffix(SIWE_SUFFIX)
            .ok_or(AuthError::InvalidMessage)?
            .trim()
            .to_string();

        let address_line = lines
            .next()
            .ok_or(AuthError::InvalidMessage)?
            .trim()
            .to_string();
        let address = Address::from_str(&address_line).map_err(|_| AuthError::InvalidMessage)?;

        // Expect blank line separator
        while let Some(line) = lines.next() {
            if line.trim().is_empty() {
                break;
            } else {
                return Err(AuthError::InvalidMessage);
            }
        }

        let mut statement_lines = Vec::new();
        for line in &mut lines {
            if line.trim().is_empty() {
                break;
            }
            statement_lines.push(line.to_string());
        }

        let statement = statement_lines
            .iter()
            .map(|line| line.trim_end_matches('\r').to_string())
            .collect::<Vec<_>>()
            .join("\n");
        let statement = if statement.trim().is_empty() {
            None
        } else {
            Some(statement)
        };

        let mut fields: HashMap<String, String> = HashMap::new();
        for line in lines {
            if line.trim().is_empty() {
                continue;
            }
            if let Some((key, value)) = line.split_once(':') {
                fields.insert(key.trim().to_string(), value.trim().to_string());
            }
        }

        let uri = fields
            .get("URI")
            .ok_or(AuthError::InvalidMessage)?
            .to_string();
        let version = fields
            .get("Version")
            .ok_or(AuthError::InvalidMessage)?
            .to_string();
        let chain_id: u64 = fields
            .get("Chain ID")
            .ok_or(AuthError::InvalidMessage)?
            .parse()
            .map_err(|_| AuthError::InvalidMessage)?;
        let nonce = fields
            .get("Nonce")
            .ok_or(AuthError::InvalidMessage)?
            .to_string();
        let issued_at_raw = fields
            .get("Issued At")
            .ok_or(AuthError::InvalidMessage)?
            .to_string();
        let issued_at = DateTime::parse_from_rfc3339(&issued_at_raw)
            .map_err(|_| AuthError::InvalidMessage)?
            .with_timezone(&Utc);

        if version.trim() != "1" {
            return Err(AuthError::InvalidMessage);
        }

        Ok(Self {
            domain,
            address,
            statement,
            uri,
            chain_id,
            nonce,
            issued_at,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::{DateTime, Utc};

    fn sample_message() -> String {
        let domain = "example.com";
        let address = "0x000000000000000000000000000000000000dEaD";
        let statement = "Sign in to Rust Web3 Risk Platform";
        let uri = "https://example.com";
        let chain_id = 1;
        let nonce = "abc123";
        let issued_at = "2024-01-01T00:00:00Z";

        format!(
            "{domain} wants you to sign in with your Ethereum account:
{address}

{statement}

URI: {uri}
Version: 1
Chain ID: {chain_id}
Nonce: {nonce}
Issued At: {issued_at}"
        )
    }

    #[test]
    fn parse_siwe_message_ok() {
        let message = sample_message();
        let parsed = ParsedSiweMessage::parse(&message).expect("should parse");
        assert_eq!(parsed.domain, "example.com");
        assert_eq!(parsed.uri, "https://example.com");
        assert_eq!(parsed.chain_id, 1);
        assert_eq!(parsed.nonce, "abc123");
        assert_eq!(
            format!("{:#x}", parsed.address),
            "0x000000000000000000000000000000000000dead"
        );
        assert!(parsed.statement.is_some());
        assert_eq!(
            parsed.issued_at,
            DateTime::parse_from_rfc3339("2024-01-01T00:00:00Z")
                .unwrap()
                .with_timezone(&Utc)
        );
    }

    #[test]
    fn parse_siwe_message_rejects_missing_fields() {
        let mut message = sample_message();
        // Remove the nonce line to simulate an invalid SIWE payload.
        message = message.replace("Nonce: abc123\n", "");
        assert!(ParsedSiweMessage::parse(&message).is_err());
    }

    #[test]
    fn parse_siwe_message_rejects_invalid_version() {
        let mut message = sample_message();
        message = message.replace("Version: 1", "Version: 2");
        assert!(matches!(
            ParsedSiweMessage::parse(&message),
            Err(AuthError::InvalidMessage)
        ));
    }

    #[test]
    fn parse_siwe_message_keeps_multiline_statement() {
        let mut message = sample_message();
        message = message.replace(
            "Sign in to Rust Web3 Risk Platform",
            "Sign in\nWith multiple lines",
        );

        let parsed = ParsedSiweMessage::parse(&message).expect("should parse multiline statement");
        assert_eq!(
            parsed.statement.as_deref(),
            Some("Sign in\nWith multiple lines")
        );
    }
}
