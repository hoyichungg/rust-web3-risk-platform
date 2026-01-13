use tracing_subscriber::{EnvFilter, Registry, layer::SubscriberExt, util::SubscriberInitExt};

/// 初始化 Tracing，設定 JSON 格式輸出以便 Loki 解析
pub fn init_tracing() -> anyhow::Result<()> {
    // 讀取 RUST_LOG 環境變數，預設為 info
    let env_filter = EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info"));

    // 設定 JSON 格式層
    let fmt_layer = tracing_subscriber::fmt::layer()
        .json()
        .flatten_event(true) // 將欄位展平到根 JSON 物件中
        .with_current_span(true); // 包含 Span 資訊 (Trace ID)

    Registry::default()
        .with(env_filter)
        .with(fmt_layer)
        .try_init()?;

    Ok(())
}
