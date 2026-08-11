//! LOB Imbalance study: (Σ bid_size − Σ ask_size) / (Σ bid_size + Σ ask_size) over top-N levels.
//! Range [−1, 1]. Mirrors OrderFlowAnalysis.Calculate_OrderImbalance from the original.

use rushhft_core::hub::SubscriptionGuard;
use rushhft_core::model::enums::{AggregationLevel, PluginStatus, PluginType};
use rushhft_core::model::order_book::OrderBook;
use rushhft_core::model::study::BaseStudyModel;
use rushhft_core::plugin::{BaseStudy, Plugin, PluginContext, PluginError};
use rust_decimal::Decimal;
use std::sync::Arc;
use time::OffsetDateTime;
use tokio::sync::Mutex;

#[derive(Debug, Clone)]
pub struct LobImbalanceSettings {
    pub symbol: String,
    pub provider_id: i32,
    /// How many levels deep to sum (top of book). Default 5.
    pub levels: usize,
    pub aggregation_level: AggregationLevel,
}

impl Default for LobImbalanceSettings {
    fn default() -> Self {
        Self {
            symbol: String::new(),
            provider_id: 0,
            levels: 5,
            aggregation_level: AggregationLevel::S1,
        }
    }
}

pub struct LobImbalanceStudy {
    id: String,
    version: &'static str,
    author: &'static str,
    description: &'static str,
    inner: Arc<Inner>,
}

struct Inner {
    settings: LobImbalanceSettings,
    base: BaseStudy,
    status: Arc<arc_swap::ArcSwap<PluginStatus>>,
    ctx: Mutex<Option<Arc<dyn PluginContext>>>,
    guards: Mutex<Option<Vec<SubscriptionGuard>>>,
}

impl LobImbalanceStudy {
    pub fn new(settings: LobImbalanceSettings) -> Self {
        let id = format!(
            "lobimb-{}",
            hash_symbol_provider(&settings.symbol, settings.provider_id)
        );
        let inner = Arc::new(Inner {
            settings,
            base: BaseStudy::new(AggregationLevel::S1),
            status: Arc::new(arc_swap::ArcSwap::from_pointee(PluginStatus::Loaded)),
            ctx: Mutex::new(None),
            guards: Mutex::new(None),
        });
        Self {
            id,
            version: "0.1.0",
            author: "RushHFT",
            description: "Top-of-book bid/ask volume imbalance",
            inner,
        }
    }
}

#[async_trait::async_trait]
impl Plugin for LobImbalanceStudy {
    fn name(&self) -> &str {
        "LOB Imbalance Study"
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

        // Consumer -> register_metric
        let inner_for_base = self.inner.clone();
        let inner_for_closure = self.inner.clone();
        let ctx_for_consumer = ctx.clone();
        tokio::spawn(async move {
            inner_for_base
                .base
                .start_consumer(move |item: &BaseStudyModel| {
                    let ctx = ctx_for_consumer.clone();
                    let symbol = inner_for_closure.settings.symbol.clone();
                    let value = item.value;
                    let ts = item.timestamp;
                    tokio::spawn(async move {
                        let _ = ctx
                            .register_metric(
                                "LOB Imbalance Study",
                                "Imbalance",
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

        // Subscribe to OrderBookHub
        let inner_ob = self.inner.clone();
        let ob_hub = ctx.order_book_hub();
        let ob_guard = ob_hub.subscribe(Arc::new(move |ob: &OrderBook| {
            if ob.symbol != inner_ob.settings.symbol
                || ob.provider_id != inner_ob.settings.provider_id
            {
                return;
            }
            let value = compute_imbalance(ob, inner_ob.settings.levels);
            let mid = ob.mid_price().unwrap_or(Decimal::ZERO);
            inner_ob.base.add_calculation(BaseStudyModel {
                value,
                format: "0.0000".into(),
                timestamp: OffsetDateTime::now_utc(),
                market_mid_price: mid,
                value_color: "White".into(),
                tooltip: String::new(),
                has_error: false,
                is_stale: false,
            });
        }));

        {
            let mut guards = self.inner.guards.lock().await;
            *guards = Some(vec![ob_guard]);
        }

        self.inner
            .status
            .store(Arc::new(PluginStatus::Started));
        Ok(())
    }

    async fn stop(&self) -> Result<(), PluginError> {
        self.inner
            .status
            .store(Arc::new(PluginStatus::Stopping));
        {
            let mut guards = self.inner.guards.lock().await;
            *guards = None;
        }
        self.inner
            .status
            .store(Arc::new(PluginStatus::Stopped));
        Ok(())
    }
}

fn hash_symbol_provider(symbol: &str, provider_id: i32) -> String {
    let mut h: u64 = 0xcbf29ce484222325;
    for b in symbol.as_bytes() {
        h ^= *b as u64;
        h = h.wrapping_mul(0x100000001b3);
    }
    h ^= provider_id as u64;
    h = h.wrapping_mul(0x100000001b3);
    format!("{:x}", h)
}

/// Pure function: top-N-levels imbalance = (Σ bid_size − Σ ask_size) / (Σ bid_size + Σ ask_size).
/// Returns 0 when both sums are zero. `levels` is capped at the available depth on each side.
pub fn compute_imbalance(ob: &OrderBook, levels: usize) -> Decimal {
    let bid_sum: Decimal = ob.bids.iter().take(levels).map(|l| l.size).sum();
    let ask_sum: Decimal = ob.asks.iter().take(levels).map(|l| l.size).sum();
    let total = bid_sum + ask_sum;
    if total.is_zero() {
        Decimal::ZERO
    } else {
        (bid_sum - ask_sum) / total
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rushhft_core::Plugin;

    #[test]
    fn metadata_matches_spec() {
        let s = LobImbalanceStudy::new(LobImbalanceSettings::default());
        assert_eq!(s.name(), "LOB Imbalance Study");
        assert_eq!(s.plugin_type(), PluginType::Study);
        assert_eq!(s.status(), PluginStatus::Loaded);
        assert_eq!(s.author(), "RushHFT");
        assert_eq!(s.version(), "0.1.0");
        assert!(!s.plugin_id().is_empty());
        assert!(s.emits_metric());
    }

    #[test]
    fn default_settings_levels_is_five() {
        let s = LobImbalanceSettings::default();
        assert_eq!(s.levels, 5);
    }

    use rushhft_core::model::book_item::BookItem;
    use rushhft_core::model::order_book::OrderBook;
    use rust_decimal_macros::dec;

    fn make_book(bids: Vec<(Decimal, Decimal)>, asks: Vec<(Decimal, Decimal)>) -> OrderBook {
        let mut ob = OrderBook::new("700.HK", 10, 2, 0, 1);
        for (p, s) in bids {
            ob.add_or_update_level(BookItem::new(p, s, true, "700.HK", 1));
        }
        for (p, s) in asks {
            ob.add_or_update_level(BookItem::new(p, s, false, "700.HK", 1));
        }
        ob
    }

    #[test]
    fn imbalance_zero_when_book_empty() {
        let ob = OrderBook::new("700.HK", 10, 2, 0, 1);
        assert_eq!(compute_imbalance(&ob, 5), Decimal::ZERO);
    }

    #[test]
    fn imbalance_all_bids_is_one() {
        let ob = make_book(vec![(dec!(100), dec!(100))], vec![]);
        assert_eq!(compute_imbalance(&ob, 5), Decimal::ONE);
    }

    #[test]
    fn imbalance_all_asks_is_negative_one() {
        let ob = make_book(vec![], vec![(dec!(101), dec!(100))]);
        assert_eq!(compute_imbalance(&ob, 5), Decimal::from(-1));
    }

    #[test]
    fn imbalance_balanced_book_is_zero() {
        let ob = make_book(
            vec![(dec!(100), dec!(100))],
            vec![(dec!(101), dec!(100))],
        );
        assert_eq!(compute_imbalance(&ob, 5), Decimal::ZERO);
    }

    #[test]
    fn imbalance_respects_levels_cap() {
        let bids: Vec<(Decimal, Decimal)> = (0..5)
            .map(|i| (dec!(100) - Decimal::from(i), dec!(100)))
            .collect();
        let asks: Vec<(Decimal, Decimal)> = (0..5)
            .map(|i| (dec!(101) + Decimal::from(i), dec!(100)))
            .collect();
        let ob = make_book(bids, asks);
        // Top 3 each side: 300 vs 300 -> 0
        assert_eq!(compute_imbalance(&ob, 3), Decimal::ZERO);
    }

    #[test]
    fn imbalance_fewer_levels_than_asked_uses_whole_side() {
        let ob = make_book(
            vec![(dec!(100), dec!(100)), (dec!(99), dec!(200))],
            vec![(dec!(101), dec!(150))],
        );
        // levels=5 but only 2 bids, 1 ask -> (300-150)/450 = 1/3
        let expected = Decimal::from(150) / Decimal::from(450);
        assert_eq!(compute_imbalance(&ob, 5), expected);
    }

    use rushhft_core::hub::{OrderBookHub, ProviderHub, TradeHub};
    use rushhft_core::model::provider::Provider;
    use rushhft_core::model::trade::Trade;
    use rushhft_core::PluginContext;

    struct LobReplayCtx {
        ob_hub: Arc<OrderBookHub>,
        t_hub: Arc<TradeHub>,
        p_hub: Arc<ProviderHub>,
        metrics: Arc<std::sync::Mutex<Vec<Decimal>>>,
    }

    #[async_trait::async_trait]
    impl PluginContext for LobReplayCtx {
        async fn publish_order_book(&self, _ob: OrderBook) {}
        async fn publish_trade(&self, _t: Trade) {}
        async fn publish_provider(&self, _p: Provider) {}
        async fn register_metric(
            &self,
            _plugin: &str,
            _metric: &str,
            _exchange: &str,
            _symbol: &str,
            value: Decimal,
            _ts: time::OffsetDateTime,
        ) {
            self.metrics.lock().unwrap().push(value);
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
    async fn lob_imbalance_start_registers_metric_on_book_publish() {
        let ob_hub = Arc::new(OrderBookHub::new());
        let t_hub = Arc::new(TradeHub::new());
        let p_hub = Arc::new(ProviderHub::new());
        let metrics = Arc::new(std::sync::Mutex::new(Vec::new()));
        let ctx = Arc::new(LobReplayCtx {
            ob_hub: ob_hub.clone(),
            t_hub: t_hub.clone(),
            p_hub: p_hub.clone(),
            metrics: metrics.clone(),
        }) as Arc<dyn PluginContext>;

        let study = Arc::new(LobImbalanceStudy::new(LobImbalanceSettings {
            symbol: "700.HK".into(),
            provider_id: 1,
            levels: 5,
            aggregation_level: AggregationLevel::S1,
        }));
        study.start(ctx).await.unwrap();
        assert_eq!(study.status(), PluginStatus::Started);

        // All-bids book -> imbalance = 1
        let ob = make_book(vec![(dec!(100), dec!(100))], vec![]);
        ob_hub.publish(ob);

        tokio::time::sleep(std::time::Duration::from_millis(50)).await;
        let collected = metrics.lock().unwrap().clone();
        assert!(
            collected.contains(&Decimal::ONE),
            "expected imbalance=1 to be registered, got {:?}",
            collected
        );

        study.stop().await.unwrap();
        assert_eq!(study.status(), PluginStatus::Stopped);
    }
}
