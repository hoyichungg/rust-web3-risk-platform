use std::sync::Arc;
use std::time::Duration;

use alert_engine::AlertNotifier;
use chrono::{Duration as ChronoDuration, Utc};
use domain::{AlertRule, Wallet};
use ethers::providers::Middleware;
use ethers::{
    providers::{Http, Provider},
    types::{Address, BlockNumber, H256},
};
use tokio::time::sleep;
use tracing::{info, warn};
use uuid::Uuid;

use crate::repositories::{
    AlertRepository, PortfolioSnapshotRepository, TransactionRepository, WalletRepository,
};
use crate::services::TokenConfig;

#[derive(Clone)]
pub struct AlertEvaluator<R, P, W, T, N>
where
    R: AlertRepository + 'static,
    P: PortfolioSnapshotRepository + 'static,
    W: WalletRepository + 'static,
    T: TransactionRepository + 'static,
    N: AlertNotifier + 'static,
{
    alert_repo: Arc<R>,
    portfolio_repo: Arc<P>,
    wallet_repo: Arc<W>,
    tx_repo: Arc<T>,
    notifier: Arc<N>,
    provider: Arc<Provider<Http>>,
    tokens: Vec<TokenConfig>,
}

impl<R, P, W, T, N> AlertEvaluator<R, P, W, T, N>
where
    R: AlertRepository + 'static,
    P: PortfolioSnapshotRepository + 'static,
    W: WalletRepository + 'static,
    T: TransactionRepository + 'static,
    N: AlertNotifier + 'static,
{
    pub fn new(
        alert_repo: Arc<R>,
        portfolio_repo: Arc<P>,
        wallet_repo: Arc<W>,
        tx_repo: Arc<T>,
        notifier: Arc<N>,
        provider: Arc<Provider<Http>>,
        tokens: Vec<TokenConfig>,
    ) -> Self {
        Self {
            alert_repo,
            portfolio_repo,
            wallet_repo,
            tx_repo,
            notifier,
            provider,
            tokens,
        }
    }

    pub fn spawn(self: Arc<Self>, interval: Duration) {
        tokio::spawn(async move {
            loop {
                if let Err(err) = self.run_once().await {
                    warn!(error = %err, "alert evaluator run failed");
                }
                sleep(interval).await;
            }
        });
    }

    async fn run_once(&self) -> anyhow::Result<()> {
        let since_flow = Utc::now() - ChronoDuration::hours(24);
        let users_rules = self.collect_rules_grouped().await?;
        for (user_id, rules) in users_rules {
            let wallets = self.wallet_repo.list_by_user(user_id).await?;
            for wallet in wallets {
                let history = self
                    .portfolio_repo
                    .history_by_wallet(wallet.id, 2)
                    .await?
                    .into_iter()
                    .rev()
                    .collect::<Vec<_>>(); // oldest -> newest
                if history.len() < 2 {
                    continue;
                }
                let prev = &history[0];
                let latest = &history[1];
                let drop_pct = if prev.total_usd_value <= 0.0 {
                    0.0
                } else {
                    ((prev.total_usd_value - latest.total_usd_value) / prev.total_usd_value) * 100.0
                };
                let exposure_map = compute_exposure(&latest.positions, latest.total_usd_value);
                let net_outflow = self
                    .tx_repo
                    .net_flow_since(wallet.id, since_flow)
                    .await
                    .unwrap_or(0.0);
                let approval_count = self
                    .recent_approvals(&wallet.address, 2_000)
                    .await
                    .unwrap_or(0);
                for rule in rules.iter().filter(|r| r.enabled) {
                    match rule.r#type.as_str() {
                        "tvl_drop_pct" if drop_pct >= rule.threshold => {
                            let message = format!(
                                "Wallet {} TVL dropped {:.2}% ({} -> {})",
                                wallet.address,
                                drop_pct,
                                prev.total_usd_value,
                                latest.total_usd_value
                            );
                            self.fire(&wallet, rule, &message).await?;
                        }
                        "exposure_pct" => {
                            for (symbol, pct) in exposure_map.iter() {
                                if *pct >= rule.threshold {
                                    let message = format!(
                                        "Wallet {} {} exposure {:.2}%",
                                        wallet.address, symbol, pct
                                    );
                                    self.fire(&wallet, rule, &message).await?;
                                }
                            }
                        }
                        "net_outflow_pct" => {
                            if latest.total_usd_value > 0.0 {
                                let pct = (net_outflow / latest.total_usd_value) * 100.0;
                                if pct >= rule.threshold {
                                    let message = format!(
                                        "Wallet {} net outflow {:.2}% (~${:.2}) past 24h",
                                        wallet.address, pct, net_outflow
                                    );
                                    self.fire(&wallet, rule, &message).await?;
                                }
                            }
                        }
                        "approval_spike" => {
                            if (approval_count as f64) >= rule.threshold {
                                let message = format!(
                                    "Wallet {} saw {} approvals in recent blocks",
                                    wallet.address, approval_count
                                );
                                self.fire(&wallet, rule, &message).await?;
                            }
                        }
                        "tvl_below" => {
                            if latest.total_usd_value <= rule.threshold {
                                let message = format!(
                                    "Wallet {} TVL below ${:.2}",
                                    wallet.address, latest.total_usd_value
                                );
                                self.fire(&wallet, rule, &message).await?;
                            }
                        }
                        _ => {}
                    };
                }
            }
        }
        Ok(())
    }

    async fn collect_rules_grouped(&self) -> anyhow::Result<Vec<(Uuid, Vec<AlertRule>)>> {
        let users = self.alert_repo.list_user_ids().await?;
        let mut grouped = Vec::with_capacity(users.len());
        for user_id in users {
            let rules = self.alert_repo.list_rules(user_id).await?;
            grouped.push((user_id, rules));
        }
        Ok(grouped)
    }

    async fn fire(&self, wallet: &Wallet, rule: &AlertRule, message: &str) -> anyhow::Result<()> {
        if let Some(last) = self
            .alert_repo
            .last_trigger_at(rule.id, wallet.id)
            .await
            .ok()
            .flatten()
        {
            let cooldown = ChronoDuration::seconds(rule.cooldown_secs.max(0));
            if Utc::now() < last + cooldown {
                return Ok(());
            }
        }
        self.alert_repo
            .insert_trigger(rule.id, wallet.id, message)
            .await?;
        self.notifier.notify(rule.id, wallet.id, message).await;
        info!(wallet = %wallet.address, rule = %rule.id, "alert triggered");
        Ok(())
    }

    async fn recent_approvals(
        &self,
        wallet_address: &str,
        lookback_blocks: u64,
    ) -> anyhow::Result<usize> {
        if self.tokens.is_empty() {
            return Ok(0);
        }
        let wallet = wallet_address.parse::<Address>()?;
        let current = self.provider.get_block_number().await?.as_u64();
        let from_block = current.saturating_sub(lookback_blocks);
        let approval_sig: H256 = H256::from_slice(&ethers::utils::keccak256(
            "Approval(address,address,uint256)",
        ));
        let wallet_topic = H256::from_slice(wallet.as_bytes());
        let token_addresses: Vec<Address> = self.tokens.iter().map(|t| t.address).collect();
        let filter = ethers::types::Filter::new()
            .address(token_addresses)
            .topic0(approval_sig)
            .topic1(wallet_topic)
            .from_block(BlockNumber::Number(from_block.into()))
            .to_block(BlockNumber::Number(current.into()));
        let logs = self.provider.get_logs(&filter).await.unwrap_or_default();
        Ok(logs.len())
    }
}

fn compute_exposure(
    positions: &[domain::Position],
    total: f64,
) -> std::collections::HashMap<String, f64> {
    let mut map = std::collections::HashMap::new();
    if total <= 0.0 {
        return map;
    }
    for p in positions {
        let pct = (p.usd_value / total) * 100.0;
        map.insert(p.asset_symbol.clone(), pct);
    }
    map
}
