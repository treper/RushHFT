mod aggregator;
pub use aggregator::{compute_ott, OttCounters};

use rushhft_core::model::enums::{AggregationLevel, PluginStatus, PluginType};
use rushhft_core::plugin::{BaseStudy, Plugin, PluginContext, PluginError};
use rushhft_core::hub::SubscriptionGuard;
use rushhft_core::model::order_book::OrderBook;
use rushhft_core::model::study::BaseStudyModel;
use rust_decimal::Decimal;
use std::sync::Arc;
use time::OffsetDateTime;
use tokio::sync::Mutex;

#[derive(Debug, Clone)]
pub struct OttRatioSettings {
    pub symbol: String,
    pub provider_id: i32,
    pub aggregation_level: AggregationLevel,
}

impl Default for OttRatioSettings {
    fn default() -> Self {
        Self {
            symbol: String::new(),
            provider_id: 0,
            aggregation_level: AggregationLevel::S1,
        }
    }
}

struct Inner {
    settings: OttRatioSettings,
    base: BaseStudy,
    counters: std::sync::Mutex<OttCounters>,
    status: Arc<arc_swap::ArcSwap<PluginStatus>>,
    ctx: Mutex<Option<Arc<dyn PluginContext>>>,
    guards: Mutex<Option<Vec<SubscriptionGuard>>>,
}

pub struct OttRatioStudy {
    id: String,
    version: &'static str,
    author: &'static str,
    description: &'static str,
    inner: Arc<Inner>,
}

impl OttRatioStudy {
    pub fn new(settings: OttRatioSettings) -> Self {
        let id = format!(
            "ott-{:x}",
            (settings.symbol.as_bytes().iter().fold(0xcbf29ce484222325u64, |acc, b| {
                (acc ^ (*b as u64)).wrapping_mul(0x100000001b3)
            })) ^ (settings.provider_id as u64)
        );
        let inner = Arc::new(Inner {
            settings,
            base: BaseStudy::new(AggregationLevel::S1),
            counters: std::sync::Mutex::new(OttCounters::default()),
            status: Arc::new(arc_swap::ArcSwap::from_pointee(PluginStatus::Loaded)),
            ctx: Mutex::new(None),
            guards: Mutex::new(None),
        });
        Self {
            id,
            version: "0.1.0",
            author: "RushHFT",
            description: "Order-to-Trade Ratio (L2 book deltas vs executed trades)",
            inner,
        }
    }
}

#[async_trait::async_trait]
impl Plugin for OttRatioStudy {
    fn name(&self) -> &str { "OTT Ratio Study" }
    fn version(&self) -> &str { self.version }
    fn author(&self) -> &str { self.author }
    fn description(&self) -> &str { self.description }
    fn plugin_type(&self) -> PluginType { PluginType::Study }
    fn status(&self) -> PluginStatus { **self.inner.status.load() }
    fn plugin_id(&self) -> &str { &self.id }
    fn emits_metric(&self) -> bool { true }

    async fn start(&self, ctx: Arc<dyn PluginContext>) -> Result<(), PluginError> {
        {
            let mut g = self.inner.ctx.lock().await;
            *g = Some(ctx.clone());
        }
        let inner = self.inner.clone();
        let inner_closure = self.inner.clone();
        let ctx_for_consumer = ctx.clone();
        tokio::spawn(async move {
            inner
                .base
                .start_consumer(move |item: &BaseStudyModel| {
                    let ctx = ctx_for_consumer.clone();
                    let symbol = inner_closure.settings.symbol.clone();
                    let value = item.value;
                    let ts = item.timestamp;
                    tokio::spawn(async move {
                        let _ = ctx
                            .register_metric("OTT Ratio Study", "OTT", "LongPort", &symbol, value, ts)
                            .await;
                    });
                })
                .await;
        });

        let inner_ob = self.inner.clone();
        let inner_trade = self.inner.clone();
        let ob_hub = ctx.order_book_hub();
        let trade_hub = ctx.trade_hub();

        let ob_guard = ob_hub.subscribe(Arc::new(move |ob: &OrderBook| {
            if ob.symbol != inner_ob.settings.symbol
                || ob.provider_id != inner_ob.settings.provider_id
            {
                return;
            }
            let mut counters = inner_ob.counters.lock().unwrap();
            let value = compute_ott(&mut counters, ob, 0);
            let mid = ob.mid_price().unwrap_or(Decimal::ZERO);
            inner_ob.base.add_calculation(BaseStudyModel {
                value,
                format: "N1".into(),
                timestamp: OffsetDateTime::now_utc(),
                market_mid_price: mid,
                value_color: "White".into(),
                tooltip: String::new(),
                has_error: false,
                is_stale: false,
            });
        }));

        use rushhft_core::model::trade::Trade;
        let trade_guard = trade_hub.subscribe(Arc::new(move |t: &Trade| {
            if t.symbol != inner_trade.settings.symbol
                || t.provider_id != inner_trade.settings.provider_id
            {
                return;
            }
            // Increment trade counter; the next order-book push will compute.
            let mut counters = inner_trade.counters.lock().unwrap();
            counters.trade_count += 1;
        }));

        {
            let mut guards = self.inner.guards.lock().await;
            *guards = Some(vec![ob_guard, trade_guard]);
        }
        self.inner.status.store(Arc::new(PluginStatus::Started));
        Ok(())
    }

    async fn stop(&self) -> Result<(), PluginError> {
        self.inner.status.store(Arc::new(PluginStatus::Stopping));
        {
            let mut guards = self.inner.guards.lock().await;
            *guards = None;
        }
        self.inner.status.store(Arc::new(PluginStatus::Stopped));
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rushhft_core::Plugin;

    #[test]
    fn metadata_matches_spec() {
        let s = OttRatioStudy::new(OttRatioSettings::default());
        assert_eq!(s.name(), "OTT Ratio Study");
        assert_eq!(s.plugin_type(), PluginType::Study);
        assert_eq!(s.status(), PluginStatus::Loaded);
        assert!(s.emits_metric());
        assert!(!s.plugin_id().is_empty());
    }
}
