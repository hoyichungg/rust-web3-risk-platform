use async_trait::async_trait;
use domain::AlertRule;
use uuid::Uuid;

#[async_trait]
pub trait AlertService: Send + Sync {
    async fn list_alerts(&self, user_id: Uuid) -> Vec<AlertRule>;
}

#[derive(Clone, Default)]
pub struct InMemoryAlertService;

#[async_trait]
impl AlertService for InMemoryAlertService {
    async fn list_alerts(&self, user_id: Uuid) -> Vec<AlertRule> {
        // 簡化：回傳空列表。未來可加入通知邏輯。
        let _ = user_id;
        vec![]
    }
}

#[async_trait]
pub trait AlertNotifier: Send + Sync {
    async fn notify(&self, rule_id: Uuid, wallet_id: Uuid, message: &str);
}

#[derive(Clone, Default)]
pub struct LoggingNotifier;

#[async_trait]
impl AlertNotifier for LoggingNotifier {
    async fn notify(&self, rule_id: Uuid, wallet_id: Uuid, message: &str) {
        println!(
            "ALERT rule={} wallet={} msg={}",
            rule_id, wallet_id, message
        );
    }
}
