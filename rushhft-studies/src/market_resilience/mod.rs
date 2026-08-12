mod calculator;
#[allow(unused_imports)] // MrMetrics used implicitly via calc.metrics() return type
pub use calculator::{MarketResilienceCalculator, MrMetrics};

use rushhft_core::hub::SubscriptionGuard;
use rushhft_core::model::enums::{AggregationLevel, PluginStatus, PluginType};
use rushhft_core::model::order_book::OrderBook;
use rushhft_core::model::study::BaseStudyModel;
use rushhft_core::plugin::{BaseStudy, Plugin, PluginContext, PluginError};
use rust_decimal::prelude::ToPrimitive;
use rust_decimal::Decimal;
use std::sync::Arc;
use std::sync::Mutex as StdMutex;
use time::OffsetDateTime;
use tokio::sync::Mutex;

#[derive(Debug, Clone)]
pub struct MarketResilienceSettings {
    pub symbol: String,
    pub provider_id: i32,
    pub aggregation_level: rushhft_core::model::enums::AggregationLevel,
}

impl Default for MarketResilienceSettings {
    fn default() -> Self {
        use rushhft_core::model::enums::AggregationLevel;
        Self {
            symbol: String::new(),
            provider_id: 0,
            aggregation_level: AggregationLevel::S1,
        }
    }
}

struct Inner {
    settings: MarketResilienceSettings,
    base: BaseStudy,
    calc: StdMutex<MarketResilienceCalculator>,
    status: Arc<arc_swap::ArcSwap<PluginStatus>>,
    ctx: Mutex<Option<Arc<dyn PluginContext>>>,
    guards: Mutex<Option<Vec<SubscriptionGuard>>>,
}

pub struct MarketResilienceStudy {
    id: String,
    version: &'static str,
    author: &'static str,
    description: &'static str,
    inner: Arc<Inner>,
}

impl MarketResilienceStudy {
    pub fn new(settings: MarketResilienceSettings) -> Self {
        let id = format!(
            "mr-{:x}",
            (settings.symbol.as_bytes().iter().fold(0xcbf29ce484222325u64, |acc, b| {
                (acc ^ (*b as u64)).wrapping_mul(0x100000001b3)
            })) ^ (settings.provider_id as u64)
        );
        let inner = Arc::new(Inner {
            settings,
            base: BaseStudy::new(AggregationLevel::S1),
            calc: StdMutex::new(MarketResilienceCalculator::new()),
            status: Arc::new(arc_swap::ArcSwap::from_pointee(PluginStatus::Loaded)),
            ctx: Mutex::new(None),
            guards: Mutex::new(None),
        });
        Self {
            id,
            version: "0.1.0",
            author: "RushHFT",
            description: "Spread/depth shock detection + 90% recovery time",
            inner,
        }
    }
}

#[async_trait::async_trait]
impl Plugin for MarketResilienceStudy {
    fn name(&self) -> &str {
        "Market Resilience Study"
    }
    fn version(&self) -> &str {
        self.version
    }
    fn author(&self) -> &str {
        self.author
    }
    fn description(&self) -> &str {
        self.description
    }
    fn plugin_type(&self) -> PluginType {
        PluginType::Study
    }
    fn status(&self) -> PluginStatus {
        **self.inner.status.load()
    }
    fn plugin_id(&self) -> &str {
        &self.id
    }
    fn emits_metric(&self) -> bool {
        true
    }

    async fn start(&self, ctx: Arc<dyn PluginContext>) -> Result<(), PluginError> {
        {
            let mut g = self.inner.ctx.lock().await;
            *g = Some(ctx.clone());
        }

        let inner_for_base = self.inner.clone();
        let ctx_for_consumer = ctx.clone();
        tokio::spawn(async move {
            inner_for_base
                .base
                .start_consumer(move |item: &BaseStudyModel| {
                    let ctx = ctx_for_consumer.clone();
                    let symbol = ctx.current_symbol();
                    let value = item.value;
                    let ts = item.timestamp;
                    tokio::spawn(async move {
                        let _ = ctx
                            .register_metric(
                                "Market Resilience Study",
                                "MR_SpreadRecovery",
                                "LongPort",
                                &symbol,
                                value,
                                ts,
                            )
                            .await;
                    });
                })
                .await;
        });

        let inner_ob = self.inner.clone();
        let ctx_for_ob = ctx.clone();
        let ob_hub = ctx.order_book_hub();
        let ob_guard = ob_hub.subscribe(Arc::new(move |ob: &OrderBook| {
            if ob.symbol != ctx_for_ob.current_symbol()
                || ob.provider_id != inner_ob.settings.provider_id
            {
                return;
            }
            let spread = ob.spread().and_then(|s| s.to_f64()).unwrap_or(0.0);
            let bid_depth = ob
                .bids
                .first()
                .map(|l| l.size.to_f64().unwrap_or(0.0))
                .unwrap_or(0.0);
            let ask_depth = ob
                .asks
                .first()
                .map(|l| l.size.to_f64().unwrap_or(0.0))
                .unwrap_or(0.0);
            let mut calc = inner_ob.calc.lock().unwrap();
            calc.observe(spread, bid_depth, ask_depth, OffsetDateTime::now_utc());
            let m = calc.metrics();
            let value = Decimal::from_f64_retain(m.spread_recovery_ms.unwrap_or(0.0))
                .unwrap_or(Decimal::ZERO);
            let is_stale = m.spread_recovery_ms.is_none();
            inner_ob.base.add_calculation(BaseStudyModel {
                value,
                format: "N0".into(),
                timestamp: OffsetDateTime::now_utc(),
                market_mid_price: ob.mid_price().unwrap_or(Decimal::ZERO),
                value_color: "White".into(),
                tooltip: String::new(),
                has_error: false,
                is_stale,
            });
        }));

        {
            let mut guards = self.inner.guards.lock().await;
            *guards = Some(vec![ob_guard]);
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
mod plugin_tests {
    use super::*;
    use rushhft_core::Plugin;

    #[test]
    fn metadata_matches_spec() {
        let s = MarketResilienceStudy::new(MarketResilienceSettings::default());
        assert_eq!(s.name(), "Market Resilience Study");
        assert_eq!(s.plugin_type(), rushhft_core::model::enums::PluginType::Study);
        assert_eq!(s.status(), rushhft_core::model::enums::PluginStatus::Loaded);
        assert!(s.emits_metric());
        assert!(!s.plugin_id().is_empty());
    }
}
