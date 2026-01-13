use async_trait::async_trait;
use chrono::Utc;
use domain::{BacktestResult, Strategy};
use serde::Deserialize;

#[async_trait]
pub trait StrategyService: Send + Sync {
    async fn backtest(&self, strategy: Strategy, prices: Vec<PricePoint>) -> BacktestResult;
}

#[derive(Clone, Default)]
pub struct InMemoryStrategyService;

#[derive(Debug, Clone, Deserialize)]
pub struct PricePoint {
    pub timestamp: chrono::DateTime<Utc>,
    pub price: f64,
}

#[async_trait]
impl StrategyService for InMemoryStrategyService {
    async fn backtest(&self, strategy: Strategy, prices: Vec<PricePoint>) -> BacktestResult {
        let strat_type = strategy.r#type.to_lowercase();
        match strat_type.as_str() {
            "ma_cross" | "ma" => backtest_ma(strategy, prices),
            "volatility" => backtest_volatility(strategy, prices),
            "correlation" => backtest_correlation(strategy, prices),
            _ => backtest_ma(strategy, prices),
        }
    }
}

fn backtest_ma(strategy: Strategy, prices: Vec<PricePoint>) -> BacktestResult {
    let short = strategy
        .params
        .get("short_window")
        .and_then(|v| v.as_u64())
        .unwrap_or(5) as usize;
    let long = strategy
        .params
        .get("long_window")
        .and_then(|v| v.as_u64())
        .unwrap_or(20) as usize;
    let mut equity = 1.0;
    let mut equity_curve = Vec::with_capacity(prices.len());
    let mut window: Vec<f64> = Vec::new();
    let mut prev_position: f64 = 0.0;

    for point in prices {
        window.push(point.price);
        if window.len() > long {
            window.remove(0);
        }
        let short_ma = if window.len() >= short {
            window.iter().rev().take(short).sum::<f64>() / short as f64
        } else {
            point.price
        };
        let long_ma = window.iter().sum::<f64>() / window.len() as f64;
        let position = if short_ma > long_ma { 1.0 } else { 0.0 };

        if let Some(prev_price) = window.get(window.len().saturating_sub(2)) {
            let ret = (point.price - prev_price) / prev_price;
            equity *= 1.0 + ret * prev_position;
        }
        prev_position = position;
        equity_curve.push((point.timestamp, equity));
    }

    let metrics = build_metrics(
        &equity_curve,
        serde_json::json!({
            "short_window": short,
            "long_window": long,
            "type": "ma_cross"
        }),
    );

    BacktestResult {
        strategy_id: strategy.id,
        equity_curve,
        metrics,
        completed_at: Some(Utc::now()),
    }
}

fn backtest_volatility(strategy: Strategy, prices: Vec<PricePoint>) -> BacktestResult {
    let lookback = strategy
        .params
        .get("lookback")
        .and_then(|v| v.as_u64())
        .unwrap_or(20) as usize;
    let mut returns = Vec::new();
    for w in prices.windows(2) {
        let r = (w[1].price - w[0].price) / w[0].price;
        returns.push(r);
    }
    let vol = if returns.len() >= lookback {
        let slice = &returns[returns.len() - lookback..];
        let mean = slice.iter().copied().sum::<f64>() / slice.len() as f64;
        let var =
            slice.iter().map(|r| (r - mean).powi(2)).sum::<f64>() / (slice.len() as f64).max(1.0);
        var.sqrt() * (252.0_f64).sqrt()
    } else {
        0.0
    };

    let equity_curve: Vec<_> = prices
        .iter()
        .scan(1.0, |equity, p| {
            if let Some(prev) = returns.get(returns.len().saturating_sub(1)) {
                *equity *= 1.0 + *prev;
            }
            Some((p.timestamp, *equity))
        })
        .collect();

    let metrics = build_metrics(
        &equity_curve,
        serde_json::json!({
            "annualized_vol": vol,
            "lookback": lookback,
            "type": "volatility"
        }),
    );

    BacktestResult {
        strategy_id: strategy.id,
        equity_curve,
        metrics,
        completed_at: Some(Utc::now()),
    }
}

fn backtest_correlation(strategy: Strategy, prices: Vec<PricePoint>) -> BacktestResult {
    // Placeholder: treat price itself vs a shifted series to compute correlation
    let lag = strategy
        .params
        .get("lag")
        .and_then(|v| v.as_u64())
        .unwrap_or(5) as usize;
    let mut x = Vec::new();
    let mut y = Vec::new();
    for w in prices.windows(lag + 1) {
        x.push(w[lag].price);
        y.push(w[0].price);
    }
    let corr = correlation(&x, &y);
    let equity_curve: Vec<_> = prices
        .iter()
        .enumerate()
        .map(|(i, p)| (p.timestamp, 1.0 + (i as f64) * 0.0 + corr * 0.0))
        .collect();
    let metrics = build_metrics(
        &equity_curve,
        serde_json::json!({
            "lag": lag,
            "correlation": corr,
            "type": "correlation"
        }),
    );
    BacktestResult {
        strategy_id: strategy.id,
        equity_curve,
        metrics,
        completed_at: Some(Utc::now()),
    }
}

fn correlation(x: &[f64], y: &[f64]) -> f64 {
    if x.len() != y.len() || x.is_empty() {
        return 0.0;
    }
    let mean_x = x.iter().copied().sum::<f64>() / x.len() as f64;
    let mean_y = y.iter().copied().sum::<f64>() / y.len() as f64;
    let mut num = 0.0;
    let mut den_x = 0.0;
    let mut den_y = 0.0;
    for (a, b) in x.iter().zip(y.iter()) {
        let dx = a - mean_x;
        let dy = b - mean_y;
        num += dx * dy;
        den_x += dx.powi(2);
        den_y += dy.powi(2);
    }
    if den_x == 0.0 || den_y == 0.0 {
        return 0.0;
    }
    num / (den_x.sqrt() * den_y.sqrt())
}

fn build_metrics(
    equity_curve: &[(chrono::DateTime<Utc>, f64)],
    mut base: serde_json::Value,
) -> serde_json::Value {
    if equity_curve.is_empty() {
        return base;
    }
    let returns: Vec<f64> = equity_curve
        .windows(2)
        .filter_map(|w| {
            let prev = w[0].1;
            let curr = w[1].1;
            if prev > 0.0 {
                Some(curr / prev - 1.0)
            } else {
                None
            }
        })
        .collect();
    let start = equity_curve.first().map(|(_, v)| *v).unwrap_or(1.0);
    let end = equity_curve.last().map(|(_, v)| *v).unwrap_or(start);
    let total_return = if start > 0.0 { end / start - 1.0 } else { 0.0 };

    let mut max_drawdown: f64 = 0.0;
    let mut peak = equity_curve[0].1;
    for &(_, v) in equity_curve.iter() {
        if v > peak {
            peak = v;
        }
        if peak > 0.0 {
            max_drawdown = max_drawdown.min(v / peak - 1.0);
        }
    }

    let mut vol = 0.0;
    let mut sharpe = 0.0;
    if !returns.is_empty() {
        let mean = returns.iter().copied().sum::<f64>() / returns.len() as f64;
        let var = returns.iter().map(|r| (r - mean).powi(2)).sum::<f64>()
            / (returns.len() as f64).max(1.0);
        vol = (var.sqrt()) * (252.0_f64).sqrt(); // assuming daily-ish samples
        if vol > 0.0 {
            sharpe = (mean * 252.0_f64.sqrt()) / vol;
        }
    }

    let days_span = if let (Some(first), Some(last)) = (equity_curve.first(), equity_curve.last()) {
        (last.0 - first.0).num_days().max(1)
    } else {
        1
    } as f64;
    let cagr = if days_span > 0.0 && start > 0.0 {
        (end / start).powf(365.0 / days_span) - 1.0
    } else {
        0.0
    };

    if let Some(obj) = base.as_object_mut() {
        obj.insert("total_return".to_string(), serde_json::json!(total_return));
        obj.insert("max_drawdown".to_string(), serde_json::json!(max_drawdown));
        obj.insert("annualized_vol".to_string(), serde_json::json!(vol));
        obj.insert("sharpe".to_string(), serde_json::json!(sharpe));
        obj.insert("cagr".to_string(), serde_json::json!(cagr));
    }
    base
}
