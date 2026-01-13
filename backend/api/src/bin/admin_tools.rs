use std::env;

use api::{bootstrap::build_state, config::AppConfig, telemetry};
use uuid::Uuid;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    dotenvy::dotenv().ok();
    telemetry::init_tracing()?;

    let mut config = AppConfig::from_env()?;
    // 避免在工具模式下啟動多餘背景任務
    config.enable_alert_worker = false;
    let state = build_state(&config).await?;

    let mut args = env::args().skip(1);
    let cmd = args.next().unwrap_or_default();

    match cmd.as_str() {
        "session-list" => {
            let sessions = state.session_repo.list_all().await?;
            for s in sessions {
                println!(
                    "{} user={} wallet={} revoked={:?} expires_at={}",
                    s.id, s.user_id, s.wallet_id, s.revoked_at, s.expires_at
                );
            }
        }
        "session-revoke" => {
            let id = args
                .next()
                .ok_or_else(|| anyhow::anyhow!("missing session id"))?;
            let session_id = Uuid::parse_str(&id)
                .map_err(|_| anyhow::anyhow!("invalid session id: {id}"))?;
            let revoked = state.session_repo.revoke(session_id).await?;
            if revoked {
                println!("revoked session {session_id}");
            } else {
                println!("session {session_id} not found or already revoked");
            }
        }
        "roles-refresh" => {
            let wallets = state.wallet_repo.list_all().await?;
            let mut refreshed = 0usize;
            for wallet in wallets {
                if let Ok(role) = state
                    .auth
                    .refresh_role_cache(&wallet.address, wallet.chain_id)
                    .await
                {
                    refreshed += 1;
                    println!(
                        "refreshed wallet={} chain={} role={:?}",
                        wallet.address, wallet.chain_id, role
                    );
                }
            }
            println!("done. refreshed {refreshed} wallets");
        }
        _ => {
            eprintln!(
                "Usage: cargo run -p api --bin admin_tools -- <command>\n\
                 Commands:\n  session-list\n  session-revoke <session_id>\n  roles-refresh"
            );
        }
    }

    Ok(())
}
