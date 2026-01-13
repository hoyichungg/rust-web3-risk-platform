use std::{
    collections::HashMap,
    net::IpAddr,
    sync::Arc,
    time::{Duration, Instant},
};

use tokio::sync::Mutex;

#[derive(Clone)]
enum NonceLimiterBackend {
    Memory {
        inner: Arc<Mutex<HashMap<IpAddr, Instant>>>,
        window: Duration,
    },
    Redis {
        client: redis::Client,
        window: Duration,
        key_prefix: String,
    },
}

#[derive(Clone)]
pub struct NonceLimiter {
    backend: NonceLimiterBackend,
}

impl NonceLimiter {
    pub async fn new(window: Duration, redis_url: Option<String>) -> anyhow::Result<Self> {
        if let Some(url) = redis_url {
            let client = redis::Client::open(url)?;
            Ok(Self {
                backend: NonceLimiterBackend::Redis {
                    client,
                    window,
                    key_prefix: "nonce:ip:".to_string(),
                },
            })
        } else {
            Ok(Self {
                backend: NonceLimiterBackend::Memory {
                    inner: Arc::new(Mutex::new(HashMap::new())),
                    window,
                },
            })
        }
    }

    pub async fn check(&self, ip: IpAddr) -> Result<(), NonceLimiterError> {
        match &self.backend {
            NonceLimiterBackend::Memory { inner, window } => {
                let mut guard = inner.lock().await;
                let now = Instant::now();
                if let Some(last) = guard.get(&ip) {
                    if now.duration_since(*last) < *window {
                        return Err(NonceLimiterError::RateLimited);
                    }
                }
                guard.insert(ip, now);
                Ok(())
            }
            NonceLimiterBackend::Redis {
                client,
                window,
                key_prefix,
            } => {
                let mut conn = client
                    .get_multiplexed_async_connection()
                    .await
                    .map_err(|err| NonceLimiterError::Backend {
                        _message: format!("redis conn: {err}"),
                    })?;
                let key = format!("{key_prefix}{ip}");
                let set_result: Option<String> = redis::cmd("SET")
                    .arg(&key)
                    .arg("1")
                    .arg("NX")
                    .arg("EX")
                    .arg(window.as_secs() as usize)
                    .query_async(&mut conn)
                    .await
                    .map_err(|err| NonceLimiterError::Backend {
                        _message: format!("redis set: {err}"),
                    })?;
                if set_result.is_some() {
                    Ok(())
                } else {
                    Err(NonceLimiterError::RateLimited)
                }
            }
        }
    }
}

#[derive(Debug)]
pub enum NonceLimiterError {
    RateLimited,
    Backend { _message: String },
}
