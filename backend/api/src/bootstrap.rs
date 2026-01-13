use std::{sync::Arc, time::Duration};

use crate::{
    config::AppConfig,
    nonce_limiter::NonceLimiter,
    repositories::{
        PostgresAlertRepository, PostgresPortfolioSnapshotRepository, PostgresPriceCacheRepository,
        PostgresPriceHistoryRepository, PostgresSessionRepository, PostgresStrategyRepository,
        PostgresTransactionRepository, PostgresUserRepository, PostgresWalletRepository,
    },
    services::{
        AlertEvaluator, CachedPriceOracle, CoingeckoPriceOracle, DbPortfolioService,
        FallbackPriceOracle, PriceRefresher, RecordingPriceOracle, SimulationConfig,
        StaticPriceOracle, TokenConfig,
    },
    state::AppState,
};
use alert_engine::{InMemoryAlertService, LoggingNotifier};
use anyhow::Result;
use auth::{AuthConfig, OnChainAuthService};
use chrono::Duration as ChronoDuration;
use ethers::providers::{Http, Provider, Ws};
use ethers::types::Address;
use futures_util::TryFutureExt;
use sqlx::PgPool;
use std::str::FromStr;
use strategy_engine::InMemoryStrategyService;

pub async fn build_state(config: &AppConfig) -> Result<AppState> {
    let pool = PgPool::connect(&config.database_url).await?;
    sqlx::migrate!("../migrations").run(&pool).await?;

    let default_provider =
        Provider::<Http>::try_from(config.rpc_url.as_str())?.interval(Duration::from_millis(500));
    let default_provider = Arc::new(default_provider);

    let providers_by_chain = build_providers(&config.chain_rpc_urls).await?;

    let auth_service = OnChainAuthService::new(
        AuthConfig {
            role_manager_address: config.role_manager_address.clone(),
            jwt_secret: config.jwt_secret.clone(),
            jwt_audience: config.jwt_audience.clone(),
            jwt_issuer: config.jwt_issuer.clone(),
            siwe_domain: config.siwe_domain.clone(),
            siwe_uri: config.siwe_uri.clone(),
            siwe_statement: config.siwe_statement.clone(),
            access_token_ttl: chrono_duration(config.access_token_ttl),
            refresh_token_ttl: chrono_duration(config.refresh_token_ttl),
            role_cache_ttl_default: chrono_duration(config.role_cache_ttl_default),
            role_cache_ttl_overrides: config
                .role_cache_ttl_overrides
                .iter()
                .map(|(chain, ttl)| (*chain, chrono_duration(*ttl)))
                .collect(),
        },
        default_provider.clone(),
        pool.clone(),
    )?;

    let wallet_repo = Arc::new(PostgresWalletRepository::new(pool.clone()));
    let portfolio_repo = Arc::new(PostgresPortfolioSnapshotRepository::new(pool.clone()));
    let price_history_repo = Arc::new(PostgresPriceHistoryRepository::new(pool.clone()));
    let price_cache_repo = Arc::new(PostgresPriceCacheRepository::new(pool.clone()));
    let transaction_repo = Arc::new(PostgresTransactionRepository::new(pool.clone()));
    let user_repo = Arc::new(PostgresUserRepository::new(pool.clone()));
    let strategy_repo = Arc::new(PostgresStrategyRepository::new(pool.clone()));
    let alert_repo = Arc::new(PostgresAlertRepository::new(pool.clone()));
    let session_repo = Arc::new(PostgresSessionRepository::new(pool.clone()));
    let coingecko_oracle = Arc::new(CoingeckoPriceOracle::new(
        config.coingecko_api_base.clone(),
        config.token_price_ids.clone(),
        config.price_cache_ttl,
    ));
    let static_oracle = Arc::new(StaticPriceOracle::new(config.token_prices.clone()));
    let recording = Arc::new(RecordingPriceOracle::new(
        Arc::new(FallbackPriceOracle::new(coingecko_oracle, static_oracle)),
        price_history_repo.clone(),
    ));
    let price_oracle: Arc<CachedPriceOracle<_>> = Arc::new(CachedPriceOracle::new(
        recording,
        price_cache_repo.clone(),
        config.price_cache_ttl,
    ));
    let tokens: Vec<TokenConfig> = config
        .erc20_tokens
        .iter()
        .filter_map(|t| {
            Address::from_str(&t.address)
                .ok()
                .map(|address| TokenConfig {
                    symbol: t.symbol.clone(),
                    address,
                    decimals: t.decimals,
                    chain_id: t.chain_id,
                })
        })
        .collect();
    // Ensure BSC Binance-Peg ETH is available even if env parsing failed.
    let mut tokens = tokens;
    if !tokens.iter().any(|t| t.symbol == "ETH" && t.chain_id == 56) {
        if let Ok(addr) = Address::from_str("0x2170ed0880ac9a755fd29b2688956bd959f933f8") {
            tokens.push(TokenConfig {
                symbol: "ETH".to_string(),
                address: addr,
                decimals: 18,
                chain_id: 56,
            });
        }
    }
    let tokens_for_alert = tokens.clone();

    let simulation = if config.portfolio_simulation {
        Some(SimulationConfig::demo())
    } else {
        None
    };

    let portfolio_service = Arc::new(DbPortfolioService::new(
        wallet_repo.clone(),
        portfolio_repo.clone(),
        transaction_repo.clone(),
        default_provider.clone(),
        providers_by_chain,
        build_ws_providers(&config.chain_ws_urls, config.ws_trigger_enabled).await?,
        price_oracle.clone(),
        tokens.clone(),
        simulation,
        config.portfolio_max_concurrency,
        config.portfolio_sync_retries as u32,
        Duration::from_millis(500),
        chrono_duration(config.portfolio_sync_interval),
    ));
    let alert_evaluator = Arc::new(AlertEvaluator::new(
        alert_repo.clone(),
        portfolio_repo.clone(),
        wallet_repo.clone(),
        transaction_repo.clone(),
        Arc::new(LoggingNotifier),
        default_provider.clone(),
        tokens_for_alert,
    ));
    if config.enable_alert_worker {
        alert_evaluator.clone().spawn(Duration::from_secs(60));
    }
    portfolio_service
        .clone()
        .spawn_indexer(config.portfolio_sync_interval);
    if config.ws_trigger_enabled {
        portfolio_service.clone().spawn_ws_listeners();
    }

    let strategy_service = Arc::new(InMemoryStrategyService::default());
    let alert_service = Arc::new(InMemoryAlertService::default());
    let nonce_limiter =
        Arc::new(NonceLimiter::new(config.nonce_throttle_window, config.redis_url.clone()).await?);

    // Warm price cache periodically to reduce即時查價。
    let mut refresh_symbols: std::collections::HashSet<(String, u64)> =
        std::collections::HashSet::new();
    refresh_symbols.insert(("ETH".to_string(), 1));
    for token in &tokens {
        refresh_symbols.insert((token.symbol.clone(), token.chain_id));
    }
    for (symbol, _) in &config.token_price_ids {
        refresh_symbols.insert((symbol.clone(), 1));
    }
    let refresher = Arc::new(PriceRefresher::new(
        price_oracle.clone(),
        refresh_symbols.into_iter().collect(),
        config.price_cache_ttl,
    ));
    refresher.spawn();

    Ok(AppState {
        config: config.clone(),
        db: pool,
        provider: default_provider,
        auth: Arc::new(auth_service),
        portfolio: portfolio_service,
        strategy: strategy_service,
        alerts: alert_service,
        user_repo,
        strategy_repo,
        alert_repo,
        session_repo,
        wallet_repo,
        portfolio_repo,
        price_cache_repo,
        price_history_repo,
        transaction_repo,
        nonce_limiter,
    })
}

fn chrono_duration(value: Duration) -> ChronoDuration {
    ChronoDuration::from_std(value).unwrap_or_else(|_| ChronoDuration::seconds(1))
}

async fn build_providers(
    entries: &std::collections::HashMap<u64, String>,
) -> Result<std::collections::HashMap<u64, Arc<Provider<Http>>>> {
    let mut map = std::collections::HashMap::new();
    for (chain_id, url) in entries {
        let provider =
            Provider::<Http>::try_from(url.as_str())?.interval(Duration::from_millis(500));
        map.insert(*chain_id, Arc::new(provider));
    }
    Ok(map)
}
async fn build_ws_providers(
    entries: &std::collections::HashMap<u64, String>,
    enabled: bool,
) -> Result<std::collections::HashMap<u64, Arc<Provider<Ws>>>> {
    if !enabled || entries.is_empty() {
        return Ok(std::collections::HashMap::new());
    }
    let mut map = std::collections::HashMap::new();
    for (chain_id, url) in entries {
        let provider = Provider::<Ws>::connect(url.clone())
            .map_ok(|p| p.interval(Duration::from_millis(500)))
            .await?;
        map.insert(*chain_id, Arc::new(provider));
    }
    Ok(map)
}
