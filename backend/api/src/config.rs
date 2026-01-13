use std::{env, time::Duration};

use anyhow::{Context, Result};
use axum_extra::extract::cookie::SameSite;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Erc20TokenConfig {
    pub symbol: String,
    pub address: String,
    pub decimals: u8,
    pub chain_id: u64,
}

#[derive(Clone)]
pub struct AppConfig {
    pub database_url: String,
    pub rpc_url: String,
    pub chain_rpc_urls: HashMap<u64, String>,
    pub chain_ws_urls: HashMap<u64, String>,
    pub role_manager_address: String,
    pub coingecko_api_base: String,
    pub jwt_secret: String,
    pub jwt_audience: String,
    pub jwt_issuer: String,
    pub siwe_domain: String,
    pub siwe_uri: String,
    pub siwe_statement: String,
    pub frontend_origins: Vec<String>,
    pub cookie_secure: bool,
    pub cookie_same_site: SameSite,
    pub access_token_ttl: Duration,
    pub refresh_token_ttl: Duration,
    pub portfolio_sync_interval: Duration,
    pub portfolio_max_concurrency: usize,
    pub portfolio_sync_retries: usize,
    pub ws_trigger_enabled: bool,
    pub nonce_throttle_window: Duration,
    pub role_cache_ttl_default: Duration,
    pub role_cache_ttl_overrides: HashMap<u64, Duration>,
    pub erc20_tokens: Vec<Erc20TokenConfig>,
    pub token_price_ids: HashMap<String, String>,
    pub price_cache_ttl: Duration,
    pub token_prices: HashMap<String, f64>,
    pub redis_url: Option<String>,
    pub port: u16,
    pub portfolio_simulation: bool,
    pub enable_alert_worker: bool,
}

impl AppConfig {
    pub fn from_env() -> Result<Self> {
        let access_token_ttl = parse_duration_seconds("ACCESS_TOKEN_TTL_SECS", 900);
        let refresh_token_ttl = parse_duration_seconds("REFRESH_TOKEN_TTL_SECS", 604800);
        let nonce_throttle = parse_duration_seconds("NONCE_THROTTLE_SECONDS", 5);
        let role_cache_ttl_default = parse_duration_seconds("ROLE_CACHE_TTL_SECS", 300);
        let role_cache_ttl_overrides = parse_chain_ttls("ROLE_CACHE_TTL_OVERRIDES");
        let price_cache_ttl = parse_duration_seconds("PRICE_CACHE_TTL_SECS", 60);
        let portfolio_sync_interval = parse_duration_seconds("PORTFOLIO_SYNC_INTERVAL_SECS", 900);
        let portfolio_max_concurrency = parse_usize("PORTFOLIO_MAX_CONCURRENCY", 4);
        let portfolio_sync_retries = parse_usize("PORTFOLIO_SYNC_RETRIES", 3);
        let ws_trigger_enabled = parse_bool("PORTFOLIO_WS_TRIGGER", true);
        let frontend_origins = parse_origins();
        let erc20_tokens = parse_erc20_tokens("ERC20_TOKENS");
        let token_prices = parse_token_prices("TOKEN_PRICES");
        let token_price_ids = parse_token_price_ids("TOKEN_PRICE_IDS");
        let enable_alert_worker = parse_bool("ENABLE_ALERT_WORKER", true);

        // 讀取 JWT secret 和 cookie 配置
        let jwt_secret = env::var("JWT_SECRET").unwrap_or_else(|_| "dev-secret".to_string());
        let cookie_secure = env::var("COOKIE_SECURE")
            .map(|v| matches!(v.as_str(), "1" | "true" | "TRUE"))
            .unwrap_or(false);

        // 生產環境安全檢查
        let is_production = is_production_environment();
        if is_production {
            if jwt_secret == "dev-secret" {
                anyhow::bail!(
                    "CRITICAL SECURITY ERROR: JWT_SECRET is using default 'dev-secret' in production!\n\
                    This allows anyone to forge authentication tokens.\n\
                    Set a strong random JWT_SECRET in your .env file immediately."
                );
            }
            if jwt_secret.len() < 32 {
                eprintln!(
                    "⚠️  WARNING: JWT_SECRET is too short ({} chars). \
                    Recommended: at least 32 characters for production.",
                    jwt_secret.len()
                );
            }
            if !cookie_secure {
                eprintln!(
                    "⚠️  WARNING: COOKIE_SECURE=false in production!\n\
                    Cookies will be transmitted over unencrypted HTTP, making them vulnerable to interception.\n\
                    Set COOKIE_SECURE=true when deploying behind HTTPS."
                );
            }
        }

        Ok(Self {
            database_url: env::var("DATABASE_URL")
                .context("DATABASE_URL must be set for API server")?,
            rpc_url: env::var("RPC_URL").unwrap_or_else(|_| "http://localhost:8545".to_string()),
            chain_rpc_urls: parse_chain_urls("CHAIN_RPC_URLS"),
            chain_ws_urls: parse_chain_urls("CHAIN_WS_URLS"),
            role_manager_address: env::var("ROLE_MANAGER_ADDRESS").unwrap_or_default(),
            coingecko_api_base: env::var("COINGECKO_API_BASE")
                .unwrap_or_else(|_| "https://api.coingecko.com/api/v3".to_string()),
            jwt_secret,
            jwt_audience: env::var("JWT_AUDIENCE").unwrap_or_else(|_| "rw3p".to_string()),
            jwt_issuer: env::var("JWT_ISSUER").unwrap_or_else(|_| "rw3p-api".to_string()),
            siwe_domain: env::var("SIWE_DOMAIN").unwrap_or_else(|_| "localhost:3000".to_string()),
            siwe_uri: env::var("SIWE_URI").unwrap_or_else(|_| "http://localhost:3000".to_string()),
            siwe_statement: env::var("SIWE_STATEMENT")
                .unwrap_or_else(|_| "Sign in to Rust Web3 Risk Platform".to_string()),
            frontend_origins,
            cookie_secure,
            cookie_same_site: parse_same_site(&env::var("COOKIE_SAMESITE").ok()),
            access_token_ttl,
            refresh_token_ttl,
            portfolio_sync_interval,
            portfolio_max_concurrency,
            portfolio_sync_retries,
            ws_trigger_enabled,
            nonce_throttle_window: nonce_throttle,
            role_cache_ttl_default,
            role_cache_ttl_overrides,
            erc20_tokens,
            token_price_ids,
            price_cache_ttl,
            token_prices,
            redis_url: env::var("REDIS_URL").ok(),
            port: env::var("PORT")
                .unwrap_or_else(|_| "8081".to_string())
                .parse()
                .context("PORT must be a valid u16")?,
            portfolio_simulation: parse_bool("PORTFOLIO_SIMULATION", false),
            enable_alert_worker,
        })
    }
}

fn is_production_environment() -> bool {
    env::var("ENVIRONMENT")
        .or_else(|_| env::var("ENV"))
        .map(|e| {
            let lower = e.to_lowercase();
            lower == "production" || lower == "prod"
        })
        .unwrap_or(false)
}

fn parse_origins() -> Vec<String> {
    if let Ok(list) = env::var("FRONTEND_ORIGINS") {
        split_origins(&list)
    } else if let Ok(origin) = env::var("FRONTEND_ORIGIN") {
        split_origins(&origin)
    } else {
        vec!["http://localhost:3000".to_string()]
    }
}

fn split_origins(raw: &str) -> Vec<String> {
    raw.split(',')
        .filter_map(|item| {
            let trimmed = item.trim();
            if trimmed.is_empty() {
                None
            } else {
                Some(trimmed.to_string())
            }
        })
        .collect()
}

fn parse_duration_seconds(key: &str, default: u64) -> Duration {
    env::var(key)
        .ok()
        .and_then(|v| v.parse::<u64>().ok())
        .map(Duration::from_secs)
        .unwrap_or_else(|| Duration::from_secs(default))
}

fn parse_usize(key: &str, default: usize) -> usize {
    env::var(key)
        .ok()
        .and_then(|v| v.parse::<usize>().ok())
        .filter(|value| *value > 0)
        .unwrap_or(default)
}

fn parse_same_site(value: &Option<String>) -> SameSite {
    match value.as_ref().map(|v| v.trim().to_lowercase()).as_deref() {
        Some("strict") => SameSite::Strict,
        Some("none") => SameSite::None,
        _ => SameSite::Lax,
    }
}

fn parse_chain_ttls(key: &str) -> HashMap<u64, Duration> {
    let raw = match env::var(key) {
        Ok(v) => v,
        Err(_) => return HashMap::new(),
    };

    raw.split(',')
        .filter_map(|item| {
            let trimmed = item.trim();
            if trimmed.is_empty() {
                return None;
            }
            let (chain, ttl) = trimmed.split_once('=').unwrap_or(("", ""));
            if chain.is_empty() || ttl.is_empty() {
                return None;
            }
            let chain_id = chain.trim().parse::<u64>().ok()?;
            let secs = ttl.trim().parse::<u64>().ok()?;
            Some((chain_id, Duration::from_secs(secs)))
        })
        .collect()
}

fn parse_erc20_tokens(key: &str) -> Vec<Erc20TokenConfig> {
    let raw = match env::var(key) {
        Ok(v) => v,
        Err(_) => return Vec::new(),
    };

    raw.split(',')
        .filter_map(|item| {
            let trimmed = item.trim();
            if trimmed.is_empty() {
                return None;
            }
            let parts: Vec<_> = trimmed.split(':').collect();
            if parts.len() < 3 {
                return None;
            }
            let symbol = parts[0].trim().to_string();
            let address = parts[1].trim().to_string();
            let decimals = parts
                .get(2)
                .and_then(|d| d.trim().parse::<u8>().ok())
                .unwrap_or(18);
            let chain_id = parts
                .get(3)
                .and_then(|c| c.trim().parse::<u64>().ok())
                .unwrap_or(1);
            if symbol.is_empty() || address.is_empty() {
                return None;
            }
            Some(Erc20TokenConfig {
                symbol,
                address,
                decimals,
                chain_id,
            })
        })
        .collect()
}

fn parse_token_prices(key: &str) -> HashMap<String, f64> {
    let raw = match env::var(key) {
        Ok(v) => v,
        Err(_) => return HashMap::new(),
    };

    raw.split(',')
        .filter_map(|item| {
            let (symbol, value) = item.split_once('=')?;
            let price = value.trim().parse::<f64>().ok()?;
            let symbol = symbol.trim().to_string();
            if symbol.is_empty() {
                return None;
            }
            Some((symbol, price))
        })
        .collect()
}

fn parse_token_price_ids(key: &str) -> HashMap<String, String> {
    let raw = match env::var(key) {
        Ok(v) => v,
        Err(_) => return HashMap::new(),
    };

    raw.split(',')
        .filter_map(|item| {
            let (symbol, id) = item.split_once(':')?;
            let symbol = symbol.trim().to_uppercase();
            let id = id.trim().to_lowercase();
            if symbol.is_empty() || id.is_empty() {
                return None;
            }
            Some((symbol, id))
        })
        .collect()
}

fn parse_chain_urls(key: &str) -> HashMap<u64, String> {
    let raw = match env::var(key) {
        Ok(v) => v,
        Err(_) => return HashMap::new(),
    };

    raw.split(',')
        .filter_map(|item| {
            let (chain, url) = item.split_once('=')?;
            let chain_id = chain.trim().parse::<u64>().ok()?;
            let url = url.trim();
            if url.is_empty() {
                return None;
            }
            Some((chain_id, url.to_string()))
        })
        .collect()
}

fn parse_bool(key: &str, default: bool) -> bool {
    env::var(key)
        .map(|v| matches!(v.as_str(), "1" | "true" | "TRUE" | "yes" | "on"))
        .unwrap_or(default)
}
