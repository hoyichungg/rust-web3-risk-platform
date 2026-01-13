use std::{
    collections::HashMap,
    str::FromStr,
    sync::Arc,
    time::{Duration, Instant},
};

use anyhow::{Context, Result};
use async_trait::async_trait;
use chrono::{Duration as ChronoDuration, Utc};
use domain::{PortfolioSnapshot, Position, PriceHistoryPoint, Wallet, WalletTransaction};
use ethers::{
    contract::abigen,
    providers::{Http, Middleware, Provider, Ws},
    types::{Address, BlockNumber, Filter, H256, Log, TxHash, U64, U256},
    utils::format_units,
};
use futures_util::StreamExt;
use indexer::PortfolioService;
use reqwest::Client;
use tokio::sync::{RwLock, Semaphore};
use tokio::time::sleep;
use tracing::{info, warn};
use uuid::Uuid;

use crate::repositories::{
    PortfolioSnapshotRepository, PriceCacheRepository, PriceHistoryRepository,
    TransactionRepository, WalletRepository,
};
use strategy_engine::PricePoint;

#[async_trait]
pub trait PriceOracle: Send + Sync {
    async fn price_usd(&self, asset_symbol: &str, chain_id: u64) -> Result<f64>;
}

#[derive(Clone)]
#[allow(dead_code)]
pub struct MockPriceOracle {
    pub eth_price: f64,
}

#[derive(Clone)]
pub struct StaticPriceOracle {
    prices: HashMap<String, f64>,
}

impl MockPriceOracle {
    pub fn new(eth_price: f64) -> Self {
        Self { eth_price }
    }
}

#[async_trait]
impl PriceOracle for MockPriceOracle {
    async fn price_usd(&self, asset_symbol: &str, _chain_id: u64) -> Result<f64> {
        Ok(match asset_symbol {
            "ETH" => self.eth_price,
            _ => 1.0,
        })
    }
}

impl StaticPriceOracle {
    pub fn new(mut prices: HashMap<String, f64>) -> Self {
        // ⚠️ 確保至少有基本的 fallback 價格
        if !prices.contains_key("ETH") {
            prices.insert("ETH".to_string(), 3000.0);
        }
        if !prices.contains_key("BNB") {
            prices.insert("BNB".to_string(), 600.0);
        }
        if !prices.contains_key("USDC") {
            prices.insert("USDC".to_string(), 1.0);
        }
        eprintln!("=== StaticPriceOracle initialized ===");
        eprintln!("Configured prices: {:?}", prices);
        eprintln!("=====================================");
        Self { prices }
    }
}

#[async_trait]
impl PriceOracle for StaticPriceOracle {
    async fn price_usd(&self, asset_symbol: &str, _chain_id: u64) -> Result<f64> {
        let fallback = if asset_symbol.eq_ignore_ascii_case("ETH") {
            3000.0
        } else {
            1.0
        };
        let price = self.prices.get(asset_symbol).copied().unwrap_or(fallback);
        eprintln!("StaticPriceOracle: {} -> ${}", asset_symbol, price);
        Ok(price)
    }
}

#[derive(Clone)]
pub struct TokenConfig {
    pub symbol: String,
    pub address: Address,
    pub decimals: u8,
    pub chain_id: u64,
}

#[derive(Clone)]
pub struct SimulatedAsset {
    pub symbol: &'static str,
    pub base_amount: f64,
    pub base_price: f64,
    pub volatility: f64,
}

#[derive(Clone)]
pub struct SimulationConfig {
    pub assets: Vec<SimulatedAsset>,
}

impl SimulationConfig {
    pub fn demo() -> Self {
        Self {
            assets: vec![
                SimulatedAsset {
                    symbol: "SIM_BTC",
                    base_amount: 1.2,
                    base_price: 65000.0,
                    volatility: 0.08,
                },
                SimulatedAsset {
                    symbol: "SIM_USDC",
                    base_amount: 25000.0,
                    base_price: 1.0,
                    volatility: 0.01,
                },
                SimulatedAsset {
                    symbol: "SIM_LP",
                    base_amount: 420.0,
                    base_price: 50.0,
                    volatility: 0.12,
                },
            ],
        }
    }
}

abigen!(
    Erc20Token,
    r#"[
        function balanceOf(address owner) view returns (uint256)
    ]"#,
);

#[derive(Clone)]
pub struct CoingeckoPriceOracle {
    client: Client,
    api_base: String,
    ids: HashMap<String, String>,
    cache: Arc<RwLock<HashMap<String, CachedPrice>>>,
    ttl: Duration,
}

#[derive(Clone, Copy)]
struct CachedPrice {
    price: f64,
    fetched_at: Instant,
}

impl CoingeckoPriceOracle {
    pub fn new(api_base: String, ids: HashMap<String, String>, ttl: Duration) -> Self {
        Self {
            client: Client::new(),
            api_base: api_base.trim_end_matches('/').to_string(),
            ids,
            cache: Arc::new(RwLock::new(HashMap::new())),
            ttl,
        }
    }

    fn coingecko_id(&self, symbol: &str) -> String {
        let upper = symbol.to_uppercase();
        if let Some(mapped) = self.ids.get(&upper) {
            return mapped.clone();
        }
        match upper.as_str() {
            "ETH" | "WETH" => "ethereum".to_string(),
            "BTC" => "bitcoin".to_string(),
            "WBTC" => "wrapped-bitcoin".to_string(),
            "USDC" => "usd-coin".to_string(),
            "USDT" => "tether".to_string(),
            "DAI" => "dai".to_string(),
            _ => upper.to_lowercase(),
        }
    }

    async fn cached_price(&self, symbol: &str) -> Option<(f64, bool)> {
        let cache = self.cache.read().await;
        cache.get(symbol).map(|entry| {
            let fresh = entry.fetched_at.elapsed() <= self.ttl;
            (entry.price, fresh)
        })
    }

    async fn store_price(&self, symbol: &str, price: f64) {
        let mut cache = self.cache.write().await;
        cache.insert(
            symbol.to_string(),
            CachedPrice {
                price,
                fetched_at: Instant::now(),
            },
        );
    }
}

#[async_trait]
impl PriceOracle for CoingeckoPriceOracle {
    async fn price_usd(&self, asset_symbol: &str, _chain_id: u64) -> Result<f64> {
        let symbol = asset_symbol.to_uppercase();
        if let Some((price, true)) = self.cached_price(&symbol).await {
            return Ok(price);
        }
        let stale_price = self.cached_price(&symbol).await.map(|(p, _)| p);
        let id = self.coingecko_id(&symbol);
        let url = format!("{}/simple/price", self.api_base);
        let resp = self
            .client
            .get(url)
            .query(&[("ids", id.as_str()), ("vs_currencies", "usd")])
            .send()
            .await
            .context("coingecko request failed")?;
        let status = resp.status();
        if !status.is_success() {
            if let Some(price) = stale_price {
                return Ok(price);
            }
            return Err(anyhow::anyhow!("coingecko returned status {}", status));
        }
        let body: serde_json::Value = resp
            .json()
            .await
            .context("failed to decode coingecko price response")?;
        let price = body
            .get(&id)
            .and_then(|entry| entry.get("usd"))
            .and_then(|value| value.as_f64())
            .ok_or_else(|| anyhow::anyhow!("coingecko price missing for {symbol} ({id})"))?;
        self.store_price(&symbol, price).await;
        Ok(price)
    }
}

impl CoingeckoPriceOracle {
    pub async fn fetch_market_chart(&self, symbol: &str, days: u32) -> Result<Vec<PricePoint>> {
        let id = self.coingecko_id(symbol);
        let url = format!("{}/coins/{}/market_chart", self.api_base, id);
        let days_str = days.to_string();
        let resp = self
            .client
            .get(url)
            .query(&[
                ("vs_currency", "usd"),
                ("days", days_str.as_str()),
                ("interval", "hourly"),
            ])
            .send()
            .await
            .context("coingecko market_chart request failed")?;
        let status = resp.status();
        if !status.is_success() {
            return Err(anyhow::anyhow!("coingecko market_chart status {}", status));
        }
        let body: serde_json::Value = resp
            .json()
            .await
            .context("decode market_chart response failed")?;
        let prices = body
            .get("prices")
            .and_then(|v| v.as_array())
            .ok_or_else(|| anyhow::anyhow!("coingecko market_chart missing prices"))?;

        let mut points = Vec::with_capacity(prices.len());
        for entry in prices {
            if let Some(arr) = entry.as_array() {
                if arr.len() >= 2 {
                    if let (Some(ts_ms), Some(price)) = (arr[0].as_i64(), arr[1].as_f64()) {
                        if let Some(ts) = chrono::DateTime::<Utc>::from_timestamp_millis(ts_ms) {
                            points.push(PricePoint {
                                timestamp: ts,
                                price,
                            });
                        }
                    }
                }
            }
        }
        Ok(points)
    }
}

#[derive(Clone)]
pub struct FallbackPriceOracle<P, F>
where
    P: PriceOracle,
    F: PriceOracle,
{
    primary: Arc<P>,
    fallback: Arc<F>,
}

impl<P, F> FallbackPriceOracle<P, F>
where
    P: PriceOracle,
    F: PriceOracle,
{
    pub fn new(primary: Arc<P>, fallback: Arc<F>) -> Self {
        Self { primary, fallback }
    }
}

#[async_trait]
impl<P, F> PriceOracle for FallbackPriceOracle<P, F>
where
    P: PriceOracle,
    F: PriceOracle,
{
    async fn price_usd(&self, asset_symbol: &str, chain_id: u64) -> Result<f64> {
        match self.primary.price_usd(asset_symbol, chain_id).await {
            Ok(price) if price > 0.0 => {
                eprintln!("FallbackOracle: primary OK {} for {}", price, asset_symbol);
                Ok(price)
            }
            Ok(_) => {
                eprintln!(
                    "FallbackOracle: primary returned 0 for {}, trying fallback",
                    asset_symbol
                );
                // Primary 返回 0,嘗試 fallback
                match self.fallback.price_usd(asset_symbol, chain_id).await {
                    Ok(price) if price > 0.0 => {
                        eprintln!("FallbackOracle: fallback OK {} for {}", price, asset_symbol);
                        Ok(price)
                    }
                    Ok(_) => {
                        eprintln!(
                            "FallbackOracle: fallback also returned 0 for {}",
                            asset_symbol
                        );
                        Err(anyhow::anyhow!("fallback returned 0 for {}", asset_symbol))
                    }
                    Err(fallback_err) => {
                        eprintln!(
                            "FallbackOracle: fallback Err for {}: {}",
                            asset_symbol, fallback_err
                        );
                        Err(anyhow::anyhow!(
                            "price lookup failed for {}: fallback error: {}",
                            asset_symbol,
                            fallback_err
                        ))
                    }
                }
            }
            Err(primary_err) => {
                eprintln!(
                    "FallbackOracle: primary Err for {}: {}, trying fallback",
                    asset_symbol, primary_err
                );
                match self.fallback.price_usd(asset_symbol, chain_id).await {
                    Ok(price) if price > 0.0 => {
                        eprintln!("FallbackOracle: fallback OK {} for {}", price, asset_symbol);
                        Ok(price)
                    }
                    Ok(_) => Err(anyhow::anyhow!("fallback returned 0 for {}", asset_symbol)),
                    Err(fallback_err) => Err(anyhow::anyhow!(
                        "price lookup failed for {}: fallback error: {}",
                        asset_symbol,
                        fallback_err
                    )),
                }
            }
        }
    }
}

/// Wraps another price oracle and把最新價格落到 price_history，避免每次同步後圖表還是缺資料。
#[derive(Clone)]
pub struct RecordingPriceOracle<O> {
    inner: Arc<O>,
    history_repo: Arc<dyn PriceHistoryRepository>,
    source: String,
    min_interval: Duration,
}

impl<O> RecordingPriceOracle<O> {
    pub fn new(inner: Arc<O>, history_repo: Arc<dyn PriceHistoryRepository>) -> Self {
        Self {
            inner,
            history_repo,
            source: "oracle".to_string(),
            min_interval: Duration::from_secs(60),
        }
    }

    #[allow(dead_code)]
    pub fn with_source(mut self, source: impl Into<String>) -> Self {
        self.source = source.into();
        self
    }
}

#[async_trait]
impl<O> PriceOracle for RecordingPriceOracle<O>
where
    O: PriceOracle,
{
    async fn price_usd(&self, asset_symbol: &str, chain_id: u64) -> Result<f64> {
        let price = self.inner.price_usd(asset_symbol, chain_id).await?;
        let symbol = asset_symbol.to_uppercase();
        let now = Utc::now();

        let should_persist = match self
            .history_repo
            .latest_timestamp(&symbol, Some(chain_id))
            .await
        {
            Ok(Some(latest)) => (now - latest).num_seconds() >= self.min_interval.as_secs() as i64,
            Ok(None) => true,
            Err(err) => {
                warn!(error = %err, %symbol, "price history lookup failed");
                false
            }
        };

        if should_persist {
            let point = PriceHistoryPoint {
                id: Uuid::new_v4(),
                symbol: symbol.clone(),
                price,
                price_ts: now,
                source: self.source.clone(),
                chain_id: Some(chain_id),
            };
            if let Err(err) = self.history_repo.upsert_points(&[point]).await {
                warn!(error = %err, %symbol, "price history persist failed");
            }
        }

        Ok(price)
    }
}

/// Cache oracle results into Postgres price_cache 以降低外部 API 請求。
#[derive(Clone)]
pub struct CachedPriceOracle<O> {
    inner: Arc<O>,
    cache_repo: Arc<dyn PriceCacheRepository>,
    ttl: Duration,
}

impl<O> CachedPriceOracle<O> {
    pub fn new(inner: Arc<O>, cache_repo: Arc<dyn PriceCacheRepository>, ttl: Duration) -> Self {
        Self {
            inner,
            cache_repo,
            ttl: ttl.max(Duration::from_secs(10)),
        }
    }
}

#[async_trait]
impl<O> PriceOracle for CachedPriceOracle<O>
where
    O: PriceOracle,
{
    async fn price_usd(&self, asset_symbol: &str, chain_id: u64) -> Result<f64> {
        let symbol = asset_symbol.to_uppercase();
        if let Ok(Some(price)) = self.cache_repo.get_cached_price(&symbol, Utc::now()).await {
            if price > 0.0 {
                eprintln!(
                    "CachedPriceOracle: returning cached {} for {}",
                    price, symbol
                );
                return Ok(price);
            } else {
                eprintln!("CachedPriceOracle: ignoring cached 0 for {}", symbol);
            }
        }
        eprintln!("CachedPriceOracle: fetching fresh price for {}", symbol);
        let price = self.inner.price_usd(&symbol, chain_id).await?;
        eprintln!("CachedPriceOracle: got {} for {}, caching", price, symbol);
        if let Err(err) = self
            .cache_repo
            .upsert_price(&symbol, price, self.ttl.as_secs() as i64, "coingecko")
            .await
        {
            warn!(error = %err, %symbol, "price cache upsert failed");
        }
        Ok(price)
    }
}

pub struct DbPortfolioService<RW, RS, O>
where
    RW: WalletRepository + 'static,
    RS: PortfolioSnapshotRepository + 'static,
    O: PriceOracle + 'static,
{
    snapshot_min_interval: ChronoDuration,
    wallet_repo: Arc<RW>,
    snapshot_repo: Arc<RS>,
    tx_repo: Arc<dyn TransactionRepository>,
    default_provider: Arc<Provider<Http>>,
    providers_by_chain: HashMap<u64, Arc<Provider<Http>>>,
    ws_providers_by_chain: HashMap<u64, Arc<Provider<Ws>>>,
    oracle: Arc<O>,
    tokens: Vec<TokenConfig>,
    simulation: Option<SimulationConfig>,
    max_concurrency: usize,
    max_retries: u32,
    retry_backoff: Duration,
}

impl<RW, RS, O> DbPortfolioService<RW, RS, O>
where
    RW: WalletRepository + 'static,
    RS: PortfolioSnapshotRepository + 'static,
    O: PriceOracle + 'static,
{
    pub fn new(
        wallet_repo: Arc<RW>,
        snapshot_repo: Arc<RS>,
        tx_repo: Arc<dyn crate::repositories::TransactionRepository>,
        default_provider: Arc<Provider<Http>>,
        providers_by_chain: HashMap<u64, Arc<Provider<Http>>>,
        ws_providers_by_chain: HashMap<u64, Arc<Provider<Ws>>>,
        oracle: Arc<O>,
        tokens: Vec<TokenConfig>,
        simulation: Option<SimulationConfig>,
        max_concurrency: usize,
        max_retries: u32,
        retry_backoff: Duration,
        snapshot_min_interval: ChronoDuration,
    ) -> Self {
        Self {
            snapshot_min_interval,
            wallet_repo,
            snapshot_repo,
            tx_repo,
            default_provider,
            providers_by_chain,
            ws_providers_by_chain,
            oracle,
            tokens,
            simulation,
            max_concurrency: max_concurrency.max(1),
            max_retries: max_retries.max(1),
            retry_backoff,
        }
    }

    pub fn spawn_indexer(self: Arc<Self>, interval: Duration) {
        tokio::spawn(async move {
            loop {
                if let Err(err) = self.clone().sync_all_wallets(None).await {
                    warn!(error = %err, "portfolio sync failed");
                }
                sleep(interval).await;
            }
        });
    }

    pub fn spawn_ws_listeners(self: Arc<Self>) {
        for (chain_id, provider) in self.ws_providers_by_chain.clone() {
            let service = self.clone();
            tokio::spawn(async move {
                match provider.subscribe_blocks().await {
                    Ok(mut stream) => {
                        while let Some(_block) = stream.next().await {
                            if let Err(err) = service.clone().sync_all_wallets(Some(chain_id)).await
                            {
                                warn!(error = %err, chain_id, "ws-triggered sync failed");
                            }
                        }
                    }
                    Err(err) => warn!(error = %err, chain_id, "ws subscribe failed"),
                }
            });
        }
    }

    async fn sync_all_wallets(self: Arc<Self>, chain_filter: Option<u64>) -> Result<()> {
        let wallets = if let Some(chain_id) = chain_filter {
            self.wallet_repo.list_by_chain(chain_id).await?
        } else {
            self.wallet_repo.list_all().await?
        };
        let filter_val = chain_filter.unwrap_or(0);
        tracing::info!(
            chain_filter = filter_val,
            wallet_count = wallets.len(),
            max_concurrency = self.max_concurrency,
            "starting portfolio sync"
        );
        let semaphore = Arc::new(Semaphore::new(self.max_concurrency));
        let mut handles = Vec::with_capacity(wallets.len());

        for wallet in wallets {
            let permit = semaphore.clone().acquire_owned().await?;
            let service = self.clone();
            handles.push(tokio::spawn(async move {
                let _permit = permit;
                let result = service.sync_wallet_with_retry(&wallet).await;
                (wallet, result)
            }));
        }

        for handle in handles {
            match handle.await {
                Ok((wallet, result)) => match result {
                    Ok(_) => {
                        let _ = self
                            .snapshot_repo
                            .log_indexer_run(wallet.id, "ok", None)
                            .await;
                    }
                    Err(err) => {
                        warn!(
                            error = %err,
                            wallet_id = %wallet.id,
                            "failed to sync wallet"
                        );
                        let _ = self
                            .snapshot_repo
                            .log_indexer_run(wallet.id, "error", Some(&err.to_string()))
                            .await;
                    }
                },
                Err(join_err) => warn!(error = %join_err, "indexer task join error"),
            };
        }
        Ok(())
    }

    async fn sync_wallet_with_retry(&self, wallet: &Wallet) -> Result<()> {
        let mut attempt: u32 = 0;
        loop {
            attempt += 1;
            match self.sync_wallet(wallet).await {
                Ok(_) => return Ok(()),
                Err(err) if attempt < self.max_retries => {
                    warn!(
                        error = %err,
                        wallet_id = %wallet.id,
                        attempt,
                        "sync wallet failed, retrying"
                    );
                    sleep(self.retry_backoff * attempt).await;
                }
                Err(err) => return Err(err),
            }
        }
    }

    fn provider_for_chain(&self, chain_id: u64) -> Arc<Provider<Http>> {
        let provider = self
            .providers_by_chain
            .get(&chain_id)
            .cloned()
            .unwrap_or_else(|| {
                eprintln!(
                    "WARNING: No provider for chain_id {}, using default",
                    chain_id
                );
                self.default_provider.clone()
            });
        eprintln!(
            "provider_for_chain({}) -> using provider (has chain-specific: {})",
            chain_id,
            self.providers_by_chain.contains_key(&chain_id)
        );
        provider
    }

    async fn sync_transactions(
        &self,
        wallet: &Wallet,
        provider: Arc<Provider<Http>>,
        latest_block: u64,
    ) -> Result<()> {
        let last_block = self
            .tx_repo
            .last_tx_block(wallet.id)
            .await?
            .unwrap_or_else(|| latest_block as i64);
        let start_block = if last_block <= 0 {
            latest_block.saturating_sub(500)
        } else {
            (last_block as u64).saturating_add(1)
        };
        if start_block > latest_block {
            self.tx_repo
                .update_last_tx_block(wallet.id, wallet.chain_id, latest_block as i64)
                .await
                .ok();
            return Ok(());
        }

        let wallet_address = Address::from_str(&wallet.address)?;
        let token_map: HashMap<Address, &TokenConfig> = self
            .tokens
            .iter()
            .filter(|t| t.chain_id == wallet.chain_id)
            .filter_map(|t| Some((t.address, t)))
            .collect();
        if token_map.is_empty() {
            self.tx_repo
                .update_last_tx_block(wallet.id, wallet.chain_id, latest_block as i64)
                .await
                .ok();
            return Ok(());
        }

        let token_addresses: Vec<Address> = token_map.keys().cloned().collect();
        let from_logs = self
            .fetch_transfer_logs(
                provider.clone(),
                &token_addresses,
                wallet_address,
                start_block,
                latest_block,
                true,
            )
            .await?;
        let to_logs = self
            .fetch_transfer_logs(
                provider.clone(),
                &token_addresses,
                wallet_address,
                start_block,
                latest_block,
                false,
            )
            .await?;

        let mut all_logs = Vec::new();
        all_logs.extend(from_logs);
        all_logs.extend(to_logs);

        if all_logs.is_empty() {
            self.tx_repo
                .update_last_tx_block(wallet.id, wallet.chain_id, latest_block as i64)
                .await
                .ok();
            return Ok(());
        }

        let mut txs = Vec::with_capacity(all_logs.len());
        let mut block_cache: HashMap<u64, chrono::DateTime<Utc>> = HashMap::new();

        for log in all_logs {
            let token = match token_map.get(&log.address) {
                Some(t) => t,
                None => continue,
            };
            let from_addr = topic_to_address(log.topics.get(1));
            let to_addr = topic_to_address(log.topics.get(2));
            let direction = if to_addr.eq_ignore_ascii_case(&wallet.address) {
                "in"
            } else {
                "out"
            };
            let amount_raw = U256::from_big_endian(log.data.as_ref());
            let amount: f64 = format_units(amount_raw, token.decimals as i32)?
                .parse()
                .unwrap_or(0.0);
            let price = self
                .oracle
                .price_usd(&token.symbol, wallet.chain_id)
                .await
                .unwrap_or(0.0);
            let block_number = log
                .block_number
                .unwrap_or_else(|| U64::from(latest_block))
                .as_u64();
            let block_ts = self
                .block_timestamp(provider.clone(), block_number, &mut block_cache)
                .await
                .unwrap_or_else(|_| Utc::now());
            txs.push(WalletTransaction {
                id: Uuid::new_v4(),
                wallet_id: wallet.id,
                chain_id: wallet.chain_id,
                tx_hash: format_hash(log.transaction_hash),
                block_number: block_number as i64,
                log_index: log.log_index.unwrap_or_default().as_u64() as i64,
                asset_symbol: token.symbol.clone(),
                amount,
                usd_value: amount * price,
                direction: direction.to_string(),
                from_address: from_addr,
                to_address: to_addr,
                block_timestamp: block_ts,
            });
        }

        self.tx_repo.insert_transactions(&txs).await?;
        self.tx_repo
            .update_last_tx_block(wallet.id, wallet.chain_id, latest_block as i64)
            .await
            .ok();
        Ok(())
    }

    async fn fetch_transfer_logs(
        &self,
        provider: Arc<Provider<Http>>,
        token_addresses: &[Address],
        wallet: Address,
        from_block: u64,
        to_block: u64,
        match_from: bool,
    ) -> Result<Vec<Log>> {
        let transfer_sig: H256 = H256::from_slice(&ethers::utils::keccak256(
            "Transfer(address,address,uint256)",
        ));
        let wallet_topic = H256::from_slice(wallet.as_bytes());
        let mut filter = Filter::new()
            .address(token_addresses.to_vec())
            .topic0(transfer_sig)
            .from_block(BlockNumber::Number(from_block.into()))
            .to_block(BlockNumber::Number(to_block.into()));

        if match_from {
            filter = filter.topic1(wallet_topic);
        } else {
            filter = filter.topic2(wallet_topic);
        }

        let logs = provider.get_logs(&filter).await?;
        Ok(logs)
    }

    async fn block_timestamp(
        &self,
        provider: Arc<Provider<Http>>,
        block_number: u64,
        cache: &mut HashMap<u64, chrono::DateTime<Utc>>,
    ) -> Result<chrono::DateTime<Utc>> {
        if let Some(ts) = cache.get(&block_number) {
            return Ok(*ts);
        }
        let block = provider
            .get_block(BlockNumber::Number(block_number.into()))
            .await?
            .ok_or_else(|| anyhow::anyhow!("block not found"))?;
        let ts = chrono::DateTime::<Utc>::from_timestamp(block.timestamp.as_u64() as i64, 0)
            .ok_or_else(|| anyhow::anyhow!("invalid block timestamp"))?;
        cache.insert(block_number, ts);
        Ok(ts)
    }

    async fn sync_wallet(&self, wallet: &Wallet) -> Result<()> {
        tracing::info!(
            wallet_id = %wallet.id,
            chain_id = wallet.chain_id,
            address = %wallet.address,
            "sync_wallet start"
        );
        if let Some(latest) = self.snapshot_repo.latest_by_wallet(wallet.id).await? {
            if Utc::now() - latest.timestamp < self.snapshot_min_interval {
                tracing::info!(
                    wallet_id = %wallet.id,
                    chain_id = wallet.chain_id,
                    last_snapshot = %latest.timestamp,
                    "skip sync (within min interval)"
                );
                return Ok(());
            }
        }

        let address = Address::from_str(&wallet.address)?;
        let provider = self.provider_for_chain(wallet.chain_id);
        let latest_block = provider.get_block_number().await?.as_u64();
        let balance = provider.get_balance(address, None).await?;
        let eth_amount: f64 = format_units(balance, 18)?.parse().unwrap_or(0.0);
        tracing::info!(
            wallet_id = %wallet.id,
            chain_id = wallet.chain_id,
            native_amount = eth_amount,
            "native balance fetched (pre-price)"
        );
        let price = self.oracle.price_usd("ETH", wallet.chain_id).await?;
        let usd_value = eth_amount * price;
        tracing::info!(
            wallet_id = %wallet.id,
            chain_id = wallet.chain_id,
            native_amount = eth_amount,
            native_price = price,
            "native balance fetched"
        );
        let timestamp = Utc::now();
        // ⚠️ BSC 的原生幣是 BNB,不是 ETH
        let native_symbol = if wallet.chain_id == 56 { "BNB" } else { "ETH" };
        let mut positions = Vec::new();
        // 只有餘額 > 0 才添加原生幣
        if eth_amount > 0.0 {
            positions.push(Position {
                asset_symbol: native_symbol.to_string(),
                amount: eth_amount,
                usd_value,
            });
        }
        for token in self.tokens.iter().filter(|t| t.chain_id == wallet.chain_id) {
            // ⚠️ 使用原始 eth_call 繞過 ethers-rs ABI 解碼問題
            use ethers::core::types::Bytes;

            let selector = &ethers::utils::keccak256("balanceOf(address)")[..4];
            let mut data = Vec::from(selector);
            data.extend_from_slice(&[0u8; 12]); // 填充到 32 bytes
            data.extend_from_slice(address.as_bytes());

            tracing::info!(
                wallet_id = %wallet.id,
                token = %token.symbol,
                call_data_len = data.len(),
                "calling balanceOf"
            );

            let tx = ethers::types::transaction::eip2718::TypedTransaction::Legacy(
                ethers::types::TransactionRequest {
                    to: Some(token.address.into()),
                    data: Some(Bytes::from(data)),
                    ..Default::default()
                },
            );

            let result = match provider.call(&tx, None).await {
                Ok(bytes) => bytes,
                Err(err) => {
                    warn!(
                        error = %err,
                        wallet_id = %wallet.id,
                        token = %token.symbol,
                        token_address = %token.address,
                        "erc20 balance_of call failed, skipping"
                    );
                    continue;
                }
            };

            tracing::info!(
                wallet_id = %wallet.id,
                token = %token.symbol,
                result_len = result.len(),
                "balanceOf result"
            );

            let balance = U256::from_big_endian(&result);
            let amount: f64 = format_units(balance, token.decimals as i32)?
                .parse()
                .unwrap_or(0.0);
            tracing::info!(
                wallet_id = %wallet.id,
                chain_id = wallet.chain_id,
                token = %token.symbol,
                token_address = %token.address,
                amount,
                "erc20 balance fetched"
            );
            if amount == 0.0 {
                continue;
            }

            // ⚠️ BSC 上的 Wrapped Token 用更清晰的名稱,避免與原生 ETH 混淆
            let display_symbol = if wallet.chain_id == 56 && token.symbol == "ETH" {
                "WETH (BSC)".to_string()
            } else {
                token.symbol.clone()
            };

            // 使用原始 symbol 查詢價格 (不用 display_symbol)
            let price = self
                .oracle
                .price_usd(&token.symbol, wallet.chain_id)
                .await
                .unwrap_or_else(|err| {
                    warn!(
                        error = %err,
                        symbol = %token.symbol,
                        "price lookup failed, using 0"
                    );
                    0.0
                });

            tracing::info!(
                wallet_id = %wallet.id,
                symbol = %token.symbol,
                price,
                amount,
                usd_value = amount * price,
                "position calculated"
            );

            positions.push(Position {
                asset_symbol: display_symbol,
                amount,
                usd_value: amount * price,
            });
        }
        if let Some(sim) = &self.simulation {
            for asset in &sim.assets {
                let factor = self.simulation_wave(wallet.id, timestamp, asset.volatility);
                let price = (asset.base_price * factor).max(asset.base_price * 0.4);
                let amount =
                    (asset.base_amount * (1.0 + (factor - 1.0) * 0.5)).max(asset.base_amount * 0.4);
                positions.push(Position {
                    asset_symbol: asset.symbol.to_string(),
                    amount,
                    usd_value: amount * price,
                });
            }
        }
        let total_usd_value = positions.iter().map(|p| p.usd_value).sum();
        let snapshot = PortfolioSnapshot {
            wallet_id: wallet.id,
            positions,
            total_usd_value,
            timestamp,
        };
        self.snapshot_repo.insert_snapshot(&snapshot).await?;
        let day = snapshot.timestamp.date_naive();
        self.snapshot_repo
            .upsert_daily_snapshot(
                wallet.id,
                day,
                snapshot.total_usd_value,
                &snapshot.positions,
            )
            .await
            .ok();
        self.tx_repo
            .update_last_daily_snapshot(wallet.id, wallet.chain_id, day)
            .await
            .ok();

        // Transaction indexing - 失敗不影響 snapshot
        if let Err(err) = self
            .sync_transactions(wallet, provider.clone(), latest_block)
            .await
        {
            warn!(
                error = %err,
                wallet_id = %wallet.id,
                "transaction sync failed, continuing"
            );
        }
        info!(
            wallet_id = %wallet.id,
            chain_id = wallet.chain_id,
            usd_value = total_usd_value,
            "portfolio snapshot updated"
        );
        Ok(())
    }
}

#[async_trait]
impl<RW, RS, O> PortfolioService for DbPortfolioService<RW, RS, O>
where
    RW: WalletRepository + 'static,
    RS: PortfolioSnapshotRepository + 'static,
    O: PriceOracle + 'static,
{
    async fn latest_snapshot(&self, wallet_id: Uuid) -> Option<PortfolioSnapshot> {
        self.snapshot_repo
            .latest_by_wallet(wallet_id)
            .await
            .ok()
            .flatten()
    }
}

impl<RW, RS, O> DbPortfolioService<RW, RS, O>
where
    RW: WalletRepository + 'static,
    RS: PortfolioSnapshotRepository + 'static,
    O: PriceOracle + 'static,
{
    fn simulation_wave(
        &self,
        wallet_id: Uuid,
        timestamp: chrono::DateTime<Utc>,
        volatility: f64,
    ) -> f64 {
        let phase = (wallet_id.as_u128() % 360) as f64;
        let t = timestamp.timestamp() as f64 / 180.0;
        let wave = (t + phase.to_radians()).sin();
        (1.0 + wave * volatility).max(0.2)
    }
}

fn topic_to_address(topic: Option<&H256>) -> String {
    topic
        .and_then(|t| {
            let bytes = t.as_bytes();
            // H256 是 32 bytes,地址在後 20 bytes (跳過前 12 bytes)
            if bytes.len() >= 32 {
                Some(Address::from_slice(&bytes[12..32]))
            } else {
                None
            }
        })
        .map(|addr| format!("{:#x}", addr))
        .unwrap_or_default()
        .to_lowercase()
}

fn format_hash(hash: Option<TxHash>) -> String {
    hash.map(|h| format!("{:#x}", h))
        .unwrap_or_else(|| "".to_string())
}

/// 背景刷新價格，讓 cache/price_history 持續有新值。
pub struct PriceRefresher<O>
where
    O: PriceOracle + 'static,
{
    oracle: Arc<O>,
    symbols: Vec<(String, u64)>,
    interval: Duration,
}

impl<O> PriceRefresher<O>
where
    O: PriceOracle + 'static,
{
    pub fn new(oracle: Arc<O>, symbols: Vec<(String, u64)>, interval: Duration) -> Self {
        Self {
            oracle,
            symbols,
            interval: interval.max(Duration::from_secs(30)),
        }
    }

    pub fn spawn(self: Arc<Self>) {
        tokio::spawn(async move {
            loop {
                for (symbol, chain_id) in &self.symbols {
                    if let Err(err) = self.oracle.price_usd(symbol, *chain_id).await {
                        warn!(error = %err, %symbol, chain_id = %chain_id, "price refresh failed");
                    }
                }
                sleep(self.interval).await;
            }
        });
    }
}
