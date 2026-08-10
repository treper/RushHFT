use crate::hub::{OrderBookHub, ProviderHub, TradeHub};
use crate::model::enums::{PluginStatus, PluginType};
use crate::model::order_book::OrderBook;
use crate::model::provider::Provider;
use crate::model::trade::Trade;
use async_trait::async_trait;
use rust_decimal::Decimal;
use std::sync::Arc;
use time::OffsetDateTime;

#[async_trait]
pub trait Plugin: Send + Sync {
    fn name(&self) -> &str;
    fn version(&self) -> &str;
    fn author(&self) -> &str {
        "RushHFT"
    }
    fn description(&self) -> &str {
        ""
    }
    fn plugin_type(&self) -> PluginType;
    fn status(&self) -> PluginStatus;
    fn plugin_id(&self) -> &str;
    fn emits_metric(&self) -> bool {
        false
    }
    async fn start(&self, ctx: Arc<dyn PluginContext>) -> Result<(), PluginError>;
    async fn stop(&self) -> Result<(), PluginError>;
}

#[async_trait]
pub trait PluginContext: Send + Sync {
    async fn publish_order_book(&self, ob: OrderBook);
    async fn publish_trade(&self, t: Trade);
    async fn publish_provider(&self, p: Provider);
    async fn register_metric(
        &self,
        plugin: &str,
        metric: &str,
        exchange: &str,
        symbol: &str,
        value: Decimal,
        ts: OffsetDateTime,
    );
    fn order_book_hub(&self) -> Arc<OrderBookHub>;
    fn trade_hub(&self) -> Arc<TradeHub>;
    fn provider_hub(&self) -> Arc<ProviderHub>;
}

#[derive(Debug, thiserror::Error)]
pub enum PluginError {
    #[error("plugin error: {0}")]
    Generic(String),
    #[error("plugin not started: {0}")]
    NotStarted(String),
    #[error("plugin already running: {0}")]
    AlreadyRunning(String),
    #[error("plugin start failed: {0}")]
    StartFailed(String),
}

pub mod base_data_retriever;
pub mod base_study;

pub use base_data_retriever::BaseDataRetriever;
pub use base_study::{AggregatedCollection, BaseStudy};

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicBool, Ordering};

    struct MockPlugin {
        id: String,
        started: AtomicBool,
    }

    #[async_trait]
    impl Plugin for MockPlugin {
        fn name(&self) -> &str {
            "Mock"
        }
        fn version(&self) -> &str {
            "0.1.0"
        }
        fn plugin_type(&self) -> PluginType {
            PluginType::Study
        }
        fn status(&self) -> PluginStatus {
            if self.started.load(Ordering::Relaxed) {
                PluginStatus::Started
            } else {
                PluginStatus::Stopped
            }
        }
        fn plugin_id(&self) -> &str {
            &self.id
        }
        async fn start(&self, _ctx: Arc<dyn PluginContext>) -> Result<(), PluginError> {
            self.started.store(true, Ordering::Relaxed);
            Ok(())
        }
        async fn stop(&self) -> Result<(), PluginError> {
            self.started.store(false, Ordering::Relaxed);
            Ok(())
        }
    }

    struct MockCtx {
        ob_hub: Arc<OrderBookHub>,
        t_hub: Arc<TradeHub>,
        p_hub: Arc<ProviderHub>,
    }

    impl MockCtx {
        fn new() -> Self {
            Self {
                ob_hub: Arc::new(OrderBookHub::new()),
                t_hub: Arc::new(TradeHub::new()),
                p_hub: Arc::new(ProviderHub::new()),
            }
        }
    }

    #[async_trait]
    impl PluginContext for MockCtx {
        async fn publish_order_book(&self, _ob: OrderBook) {}
        async fn publish_trade(&self, _t: Trade) {}
        async fn publish_provider(&self, _p: Provider) {}
        async fn register_metric(
            &self,
            _: &str,
            _: &str,
            _: &str,
            _: &str,
            _: Decimal,
            _: OffsetDateTime,
        ) {
        }
        fn order_book_hub(&self) -> Arc<OrderBookHub> {
            self.ob_hub.clone()
        }
        fn trade_hub(&self) -> Arc<TradeHub> {
            self.t_hub.clone()
        }
        fn provider_hub(&self) -> Arc<ProviderHub> {
            self.p_hub.clone()
        }
    }

    #[tokio::test]
    async fn plugin_start_stop_lifecycle() {
        let plugin = MockPlugin {
            id: "mock-1".to_string(),
            started: AtomicBool::new(false),
        };
        let ctx: Arc<dyn PluginContext> = Arc::new(MockCtx::new());

        assert_eq!(plugin.status(), PluginStatus::Stopped);
        plugin.start(ctx).await.unwrap();
        assert_eq!(plugin.status(), PluginStatus::Started);
        plugin.stop().await.unwrap();
        assert_eq!(plugin.status(), PluginStatus::Stopped);
    }
}
