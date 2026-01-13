use std::env;

use anyhow::{Context, Result};
use chrono::{Duration, Utc};
use serde_json::json;
use sqlx::{PgPool, postgres::PgPoolOptions};
use uuid::Uuid;

#[tokio::main]
async fn main() -> Result<()> {
    dotenvy::dotenv().ok();
    let database_url = env::var("DATABASE_URL").context("DATABASE_URL is not set")?;
    let pool = PgPoolOptions::new()
        .max_connections(5)
        .connect(&database_url)
        .await?;

    sqlx::migrate!("../migrations").run(&pool).await?;

    let wallet_address = env::var("DEV_SEED_WALLET_ADDRESS")
        .unwrap_or_else(|_| "0xf39Fd6e51aad88F6F4ce6aB8827279cffFb92266".to_string());
    let chain_id: u64 = env::var("DEV_SEED_CHAIN_ID")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(31337);

    seed_demo(&pool, &wallet_address, chain_id).await?;
    println!(
        "Seeded demo data for wallet {} on chain {} (dev only).",
        wallet_address, chain_id
    );
    Ok(())
}

async fn seed_demo(pool: &PgPool, wallet_address: &str, chain_id: u64) -> Result<()> {
    let user_id = Uuid::new_v5(&Uuid::NAMESPACE_OID, b"rw3p-dev-user");
    let wallet_namespace = format!("{wallet_address}:{chain_id}");
    let wallet_id = Uuid::new_v5(&user_id, wallet_namespace.as_bytes());

    let mut tx = pool.begin().await?;

    // Clean previous dev seed data for this user/wallet to keep results stable.
    sqlx::query("DELETE FROM alert_triggers WHERE wallet_id = $1")
        .bind(wallet_id)
        .execute(&mut *tx)
        .await?;
    sqlx::query("DELETE FROM alert_rules WHERE user_id = $1")
        .bind(user_id)
        .execute(&mut *tx)
        .await?;
    sqlx::query("DELETE FROM wallet_transactions WHERE wallet_id = $1")
        .bind(wallet_id)
        .execute(&mut *tx)
        .await?;
    sqlx::query("DELETE FROM portfolio_daily_snapshots WHERE wallet_id = $1")
        .bind(wallet_id)
        .execute(&mut *tx)
        .await?;
    sqlx::query("DELETE FROM portfolio_snapshots WHERE wallet_id = $1")
        .bind(wallet_id)
        .execute(&mut *tx)
        .await?;
    sqlx::query("DELETE FROM portfolio_indexer_runs WHERE wallet_id = $1")
        .bind(wallet_id)
        .execute(&mut *tx)
        .await?;
    sqlx::query(
        "DELETE FROM strategy_backtests WHERE strategy_id IN (SELECT id FROM strategies WHERE user_id = $1)",
    )
    .bind(user_id)
    .execute(&mut *tx)
    .await?;
    sqlx::query("DELETE FROM strategies WHERE user_id = $1")
        .bind(user_id)
        .execute(&mut *tx)
        .await?;
    sqlx::query("DELETE FROM user_sessions WHERE user_id = $1")
        .bind(user_id)
        .execute(&mut *tx)
        .await?;

    // Upsert user + wallet so SIWE 後的資料可以直接對應這個 primary wallet。
    sqlx::query(
        "INSERT INTO users (id, primary_wallet) VALUES ($1, $2)
         ON CONFLICT (id) DO UPDATE SET primary_wallet = EXCLUDED.primary_wallet",
    )
    .bind(user_id)
    .bind(wallet_address)
    .execute(&mut *tx)
    .await?;

    let wallet_id: Uuid = sqlx::query_scalar(
        "INSERT INTO wallets (id, user_id, address, chain_id)
         VALUES ($1, $2, $3, $4)
         ON CONFLICT (address, chain_id) DO UPDATE SET user_id = EXCLUDED.user_id
         RETURNING id",
    )
    .bind(wallet_id)
    .bind(user_id)
    .bind(wallet_address)
    .bind(chain_id as i64)
    .fetch_one(&mut *tx)
    .await?;

    // Demo portfolio snapshots
    struct Pos {
        symbol: &'static str,
        amount: f64,
        usd: f64,
    }
    let base_positions = vec![
        Pos {
            symbol: "ETH",
            amount: 1.8,
            usd: 5700.0,
        },
        Pos {
            symbol: "USDC",
            amount: 12_000.0,
            usd: 12_000.0,
        },
        Pos {
            symbol: "WETH",
            amount: 0.75,
            usd: 2_400.0,
        },
    ];
    let now = Utc::now();
    let history = vec![(2, 0.92), (1, 0.97), (0, 1.0)];

    for (days_ago, factor) in history {
        let positions_json: Vec<_> = base_positions
            .iter()
            .map(|p| {
                let usd_value = (p.usd * factor * 100.0).round() / 100.0;
                let amount = (p.amount * factor * 1_000_000.0).round() / 1_000_000.0;
                json!({
                    "asset_symbol": p.symbol,
                    "amount": amount,
                    "usd_value": usd_value
                })
            })
            .collect();
        let total_usd: f64 = positions_json
            .iter()
            .filter_map(|v| v.get("usd_value").and_then(|v| v.as_f64()))
            .sum();
        let ts = now - Duration::days(days_ago);

        sqlx::query(
            "INSERT INTO portfolio_snapshots (id, wallet_id, total_usd_value, snapshot_time, positions)
             VALUES ($1, $2, $3, $4, $5)",
        )
        .bind(Uuid::new_v4())
        .bind(wallet_id)
        .bind(total_usd)
        .bind(ts)
        .bind(serde_json::to_value(&positions_json)?)
        .execute(&mut *tx)
        .await?;

        sqlx::query(
            "INSERT INTO portfolio_daily_snapshots (id, wallet_id, day, total_usd_value, positions)
             VALUES ($1, $2, $3, $4, $5)
             ON CONFLICT (wallet_id, day) DO UPDATE
             SET total_usd_value = EXCLUDED.total_usd_value,
                 positions = EXCLUDED.positions,
                 updated_at = NOW()",
        )
        .bind(Uuid::new_v4())
        .bind(wallet_id)
        .bind(ts.date_naive())
        .bind(total_usd)
        .bind(serde_json::to_value(&positions_json)?)
        .execute(&mut *tx)
        .await?;
    }

    // Demo transactions
    let txs = vec![
        (
            "0xdeadbeef0001",
            "deposit",
            18_000.0,
            now - Duration::days(3),
            1,
            0,
        ),
        (
            "0xdeadbeef0002",
            "swap",
            -1_500.0,
            now - Duration::days(1),
            2,
            0,
        ),
    ];
    for (hash, direction, usd_value, ts, block_number, log_index) in txs {
        sqlx::query(
            "INSERT INTO wallet_transactions (
                id, wallet_id, chain_id, tx_hash, block_number, log_index,
                asset_symbol, amount, usd_value, direction, from_address, to_address, block_timestamp, raw
            ) VALUES ($1,$2,$3,$4,$5,$6,$7,$8,$9,$10,$11,$12,$13,$14)",
        )
        .bind(Uuid::new_v4())
        .bind(wallet_id)
        .bind(chain_id as i64)
        .bind(hash)
        .bind(block_number as i64)
        .bind(log_index as i64)
        .bind("ETH")
        .bind(usd_value / 3000.0)
        .bind(usd_value)
        .bind(direction)
        .bind(wallet_address)
        .bind("0x0000000000000000000000000000000000000000")
        .bind(ts)
        .bind(json!({ "note": "dev seed" }))
        .execute(&mut *tx)
        .await?;
    }

    // Demo alerts + triggers
    let alert_rule_id = Uuid::new_v4();
    sqlx::query(
        "INSERT INTO alert_rules (id, user_id, type, threshold, enabled, cooldown_secs)
         VALUES ($1, $2, $3, $4, $5, $6)",
    )
    .bind(alert_rule_id)
    .bind(user_id)
    .bind("tvl_drop_pct")
    .bind(12.5_f64)
    .bind(true)
    .bind(600_i64)
    .execute(&mut *tx)
    .await?;

    sqlx::query(
        "INSERT INTO alert_triggers (id, rule_id, wallet_id, message)
         VALUES ($1, $2, $3, $4)",
    )
    .bind(Uuid::new_v4())
    .bind(alert_rule_id)
    .bind(wallet_id)
    .bind("TVL 下跌超過 12.5%（近 24h）")
    .execute(&mut *tx)
    .await?;

    // Demo price history (30 天 ETH/WETH/USDC)，方便策略與圖表測試。
    sqlx::query("DELETE FROM price_history WHERE source = 'dev_seed'")
        .execute(&mut *tx)
        .await?;
    for i in 0..30 {
        let ts = now - Duration::days(29 - i);
        let eth_price = 2900.0 + (i as f64 * 25.0);
        let weth_price = 2880.0 + (i as f64 * 24.0);
        for (symbol, price) in [("ETH", eth_price), ("WETH", weth_price), ("USDC", 1.0_f64)] {
            sqlx::query(
                "INSERT INTO price_history (id, symbol, price, price_ts, source)
                 VALUES ($1, $2, $3, $4, 'dev_seed')
                 ON CONFLICT (symbol, price_ts) DO NOTHING",
            )
            .bind(Uuid::new_v4())
            .bind(symbol)
            .bind(price)
            .bind(ts)
            .execute(&mut *tx)
            .await?;
        }
    }

    tx.commit().await?;
    Ok(())
}
