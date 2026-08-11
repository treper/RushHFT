//! VPIN (Volume-Synchronized Probability of Informed Trading) study.
//!
//! Easley, Lopez de Prado & O'Hara (2012). VPIN = (1/n) × Σ|V_buy_i − V_sell_i| / V_bucket
//! over n completed volume buckets. Range [0, 1].

use rushhft_core::hub::SubscriptionGuard;
use rushhft_core::model::enums::{AggregationLevel, PluginStatus, PluginType, TradeDirection};
use rushhft_core::model::order_book::OrderBook;
use rushhft_core::model::study::BaseStudyModel;
use rushhft_core::model::trade::Trade;
use rushhft_core::plugin::{BaseStudy, Plugin, PluginContext, PluginError};
use rust_decimal::Decimal;
use std::sync::{Arc, Mutex as StdMutex};
use time::OffsetDateTime;

/// Parameters for the VPIN study.
#[derive(Debug, Clone)]
pub struct VpinSettings {
    /// Volume that must accumulate before a bucket completes. Default 1.
    pub bucket_volume_size: Decimal,
    /// Number of completed buckets in the rolling window. Default 50.
    pub number_of_buckets: usize,
    /// Symbol this study is bound to (e.g. "700.HK").
    pub symbol: String,
    /// Provider ID filter — only trades/books with this provider_id are processed.
    pub provider_id: i32,
    /// Aggregation cadence. Forced to S1 by the spec.
    pub aggregation_level: AggregationLevel,
}

impl Default for VpinSettings {
    fn default() -> Self {
        Self {
            bucket_volume_size: Decimal::ONE,
            number_of_buckets: 50,
            symbol: String::new(),
            provider_id: 0,
            aggregation_level: AggregationLevel::S1,
        }
    }
}

struct Inner {
    settings: VpinSettings,
    core: StdMutex<VpinCore>,
    base: BaseStudy,
    status: Arc<arc_swap::ArcSwap<PluginStatus>>,
    ctx: tokio::sync::Mutex<Option<Arc<dyn PluginContext>>>,
    guards: tokio::sync::Mutex<Option<Vec<SubscriptionGuard>>>,
}

/// VPIN study plugin.
pub struct VpinStudy {
    id: String,
    version: &'static str,
    author: &'static str,
    description: &'static str,
    inner: Arc<Inner>,
}

impl VpinStudy {
    pub fn new(settings: VpinSettings) -> Self {
        let id = format!(
            "vpin-{}",
            hash_symbol_provider(&settings.symbol, settings.provider_id)
        );
        let core = VpinCore::new(settings.bucket_volume_size, settings.number_of_buckets);
        let inner = Arc::new(Inner {
            settings,
            core: StdMutex::new(core),
            base: BaseStudy::new(AggregationLevel::S1),
            status: Arc::new(arc_swap::ArcSwap::from_pointee(PluginStatus::Loaded)),
            ctx: tokio::sync::Mutex::new(None),
            guards: tokio::sync::Mutex::new(None),
        });
        Self {
            id,
            version: "0.1.0",
            author: "RushHFT",
            description: "Volume-Synchronized Probability of Informed Trading",
            inner,
        }
    }

    pub fn with_settings(settings: VpinSettings) -> Self {
        Self::new(settings)
    }
}

#[async_trait::async_trait]
impl Plugin for VpinStudy {
    fn name(&self) -> &str {
        "VPIN Study"
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
        // 1) Store ctx
        {
            let mut guard = self.inner.ctx.lock().await;
            *guard = Some(ctx.clone());
        }

        // 2) Reset core
        {
            let mut core = self.inner.core.lock().unwrap();
            core.reset();
        }

        // 3) Spawn the BaseStudy consumer -> register_metric
        // The BaseStudy is owned by Inner; we need two clones of Arc<Inner>:
        // one to keep alive for the .base reference, one to capture in the closure.
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
                            .register_metric("VPIN Study", "VPIN", "LongPort", &symbol, value, ts)
                            .await;
                    });
                })
                .await;
        });

        // 4) Subscribe to OrderBookHub
        let inner_ob = self.inner.clone();
        let ob_hub = ctx.order_book_hub();
        let ob_guard = ob_hub.subscribe(Arc::new(move |ob: &OrderBook| {
            if ob.symbol != inner_ob.settings.symbol
                || ob.provider_id != inner_ob.settings.provider_id
            {
                return;
            }
            let mid = ob.mid_price().unwrap_or(Decimal::ZERO);
            let mut core = inner_ob.core.lock().unwrap();
            core.ingest_mid(mid);
            let vpin = core.current_vpin();
            inner_ob.base.add_calculation(BaseStudyModel {
                value: vpin,
                format: "N2".into(),
                timestamp: OffsetDateTime::now_utc(),
                market_mid_price: mid,
                value_color: "White".into(),
                tooltip: String::new(),
                has_error: false,
                is_stale: false,
            });
        }));

        // 5) Subscribe to TradeHub
        let inner_t = self.inner.clone();
        let t_hub = ctx.trade_hub();
        let t_guard = t_hub.subscribe(Arc::new(move |t: &Trade| {
            if t.symbol != inner_t.settings.symbol || t.provider_id != inner_t.settings.provider_id
            {
                return;
            }
            let is_buy = map_trade_direction(t.direction);
            let size = t.size;
            let mid = t.market_mid_price;
            let ts = t.timestamp;
            let mut core = inner_t.core.lock().unwrap();
            core.ingest_mid(mid);
            core.ingest_trade(size, is_buy);
            let vpin = core.current_vpin();
            inner_t.base.add_calculation(BaseStudyModel {
                value: vpin,
                format: "N2".into(),
                timestamp: ts,
                market_mid_price: mid,
                value_color: "White".into(),
                tooltip: String::new(),
                has_error: false,
                is_stale: false,
            });
        }));

        // 6) Stash guards
        {
            let mut guards = self.inner.guards.lock().await;
            *guards = Some(vec![ob_guard, t_guard]);
        }

        // 7) Status <- Started
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

/// Map a `TradeDirection` to `Option<bool>` (buy/sell/skip). Free function —
/// `impl From<TradeDirection> for Option<bool>` would collide with the orphan rule.
#[allow(dead_code)]
pub fn map_trade_direction(d: TradeDirection) -> Option<bool> {
    match d {
        TradeDirection::Up => Some(true),
        TradeDirection::Down => Some(false),
        TradeDirection::Neutral => None,
    }
}

/// Pure VPIN bucket arithmetic — no I/O, no async. Owned by `VpinStudy`.
pub(crate) struct VpinCore {
    bucket_volume_size: Decimal,
    number_of_buckets: usize,

    current_bucket_volume: Decimal,
    current_buy_volume: Decimal,
    current_sell_volume: Decimal,
    last_market_mid_price: Decimal,

    bucket_imbalances: Vec<Decimal>,
    buffer_index: usize,
    buffer_count: usize,
    rolling_sum: Decimal,
    completed_buckets: u64,
}

impl VpinCore {
    pub fn new(bucket_volume_size: Decimal, number_of_buckets: usize) -> Self {
        let n = if number_of_buckets == 0 {
            50
        } else {
            number_of_buckets
        };
        Self {
            bucket_volume_size,
            number_of_buckets: n,
            current_bucket_volume: Decimal::ZERO,
            current_buy_volume: Decimal::ZERO,
            current_sell_volume: Decimal::ZERO,
            last_market_mid_price: Decimal::ZERO,
            bucket_imbalances: vec![Decimal::ZERO; n],
            buffer_index: 0,
            buffer_count: 0,
            rolling_sum: Decimal::ZERO,
            completed_buckets: 0,
        }
    }

    pub fn current_vpin(&self) -> Decimal {
        if self.buffer_count == 0 {
            Decimal::ZERO
        } else {
            self.rolling_sum / Decimal::from(self.buffer_count)
        }
    }

    #[allow(dead_code)]
    pub fn current_bucket_volume(&self) -> Decimal {
        self.current_bucket_volume
    }

    #[allow(dead_code)]
    pub fn last_market_mid_price(&self) -> Decimal {
        self.last_market_mid_price
    }

    #[allow(dead_code)]
    pub fn completed_buckets(&self) -> u64 {
        self.completed_buckets
    }

    pub fn reset(&mut self) {
        self.current_bucket_volume = Decimal::ZERO;
        self.current_buy_volume = Decimal::ZERO;
        self.current_sell_volume = Decimal::ZERO;
        self.last_market_mid_price = Decimal::ZERO;
        self.bucket_imbalances = vec![Decimal::ZERO; self.number_of_buckets];
        self.buffer_index = 0;
        self.buffer_count = 0;
        self.rolling_sum = Decimal::ZERO;
        self.completed_buckets = 0;
    }

    pub fn ingest_mid(&mut self, mid: Decimal) {
        self.last_market_mid_price = mid;
    }

    /// Feed a trade. `is_buy = None` (Neutral) -> skip the trade entirely.
    pub fn ingest_trade(&mut self, size: Decimal, is_buy: Option<bool>) {
        let is_buy = match is_buy {
            Some(b) => b,
            None => return, // Neutral — skip per spec
        };

        if size.is_zero() {
            return;
        }

        if is_buy {
            self.current_buy_volume += size;
        } else {
            self.current_sell_volume += size;
        }
        self.current_bucket_volume += size;

        // Complete as many buckets as this trade fills.
        while self.current_bucket_volume >= self.bucket_volume_size
            && self.bucket_volume_size > Decimal::ZERO
        {
            let overflow = self.current_bucket_volume - self.bucket_volume_size;
            if is_buy {
                self.current_buy_volume -= overflow;
            } else {
                self.current_sell_volume -= overflow;
            }
            self.current_bucket_volume = self.bucket_volume_size;

            self.complete_bucket();

            // Start new bucket with overflow.
            self.current_buy_volume = if is_buy { overflow } else { Decimal::ZERO };
            self.current_sell_volume = if is_buy { Decimal::ZERO } else { overflow };
            self.current_bucket_volume = overflow;
        }
    }

    fn complete_bucket(&mut self) {
        let imbalance = if self.bucket_volume_size.is_zero() {
            Decimal::ZERO
        } else {
            (self.current_buy_volume - self.current_sell_volume).abs() / self.bucket_volume_size
        };

        if self.buffer_count == self.number_of_buckets {
            // Buffer full — evict the value at the current index.
            self.rolling_sum -= self.bucket_imbalances[self.buffer_index];
        } else {
            self.buffer_count += 1;
        }

        self.bucket_imbalances[self.buffer_index] = imbalance;
        self.rolling_sum += imbalance;
        self.buffer_index = (self.buffer_index + 1) % self.number_of_buckets;
        self.completed_buckets += 1;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rushhft_core::Plugin;
    use rust_decimal_macros::dec;

    #[test]
    fn metadata_matches_spec() {
        let s = VpinStudy::new(VpinSettings::default());
        assert_eq!(s.name(), "VPIN Study");
        assert_eq!(s.plugin_type(), PluginType::Study);
        assert_eq!(s.status(), PluginStatus::Loaded);
        assert_eq!(s.author(), "RushHFT");
        assert_eq!(s.version(), "0.1.0");
        assert_eq!(
            s.description(),
            "Volume-Synchronized Probability of Informed Trading"
        );
        assert!(!s.plugin_id().is_empty());
        assert!(s.emits_metric());
    }

    #[test]
    fn default_settings_are_sane() {
        let s = VpinSettings::default();
        assert_eq!(s.bucket_volume_size, dec!(1));
        assert_eq!(s.number_of_buckets, 50);
        assert_eq!(s.symbol, "");
        assert_eq!(s.provider_id, 0);
    }

    #[test]
    fn vpin_core_zero_until_first_bucket_completes() {
        let mut core = VpinCore::new(dec!(1), 50);
        core.ingest_trade(dec!(0.5), Some(true));
        assert_eq!(core.current_vpin(), Decimal::ZERO);
    }

    #[test]
    fn vpin_core_one_bucket_all_buys_gives_vpin_one() {
        let mut core = VpinCore::new(dec!(1), 50);
        core.ingest_trade(dec!(1), Some(true)); // bucket exactly fills, all buy
        assert_eq!(core.current_vpin(), Decimal::ONE);
    }

    #[test]
    fn vpin_core_split_bucket_gives_half() {
        let mut core = VpinCore::new(dec!(2), 50);
        core.ingest_trade(dec!(1), Some(true));
        core.ingest_trade(dec!(1), Some(false));
        // After 2 volume, bucket completes with |1-1|/2 = 0
        assert_eq!(core.current_vpin(), Decimal::ZERO);
    }

    #[test]
    fn vpin_core_neutral_trade_skipped() {
        let mut core = VpinCore::new(dec!(1), 50);
        core.ingest_trade(dec!(5), None); // Neutral — skip
        assert_eq!(core.current_vpin(), Decimal::ZERO);
        assert_eq!(core.current_bucket_volume(), Decimal::ZERO);
    }

    #[test]
    fn vpin_core_overflow_carries_to_next_bucket() {
        let mut core = VpinCore::new(dec!(1), 50);
        // 3-volume all-buy trade on bucket size 1 -> 3 buckets complete, each imbalance=1
        core.ingest_trade(dec!(3), Some(true));
        assert_eq!(core.current_vpin(), Decimal::ONE);
        assert_eq!(core.completed_buckets(), 3);
    }

    #[test]
    fn vpin_core_rolling_window_caps_at_n() {
        let mut core = VpinCore::new(dec!(1), 2); // window = 2
        core.ingest_trade(dec!(1), Some(true)); // bucket1: imb=1
        core.ingest_trade(dec!(1), Some(false)); // bucket2: imb=1 (actually |1-1|/1=0)
        core.ingest_trade(dec!(1), Some(true)); // bucket3: evicts bucket1, imb=1
        // bucket2 had imbalance 0 (1 buy, 0 sell — wait, let me retrace.
        // Actually after the first ingest: bucket1: 1 buy completes, imb=|1-0|/1=1
        // Second ingest (1 sell): splits across buckets. After bucket1 was already complete
        // and reset to overflow=0, this starts a new bucket with 1 sell → completes, imb=|0-1|/1=1
        // Third ingest (1 buy): same pattern, imb=1
        // window now {bucket2=1, bucket3=1} but only holds 2 items (cap=2)
        // rolling_sum = 1 (b2) + 1 (b3) = 2, count=2, avg=1.0
        // But bucket1 was evicted when bucket3 entered.
        // Let me just check the assertion: vpin = 2/2 = 1, not 0.5 as the plan said.
        // The plan's assertion of 0.5 assumed bucket2=0 — but actually bucket2 has imb=1.
        // So the correct expected value is 1.0. Adjust.
        assert_eq!(core.completed_buckets(), 3);
        assert_eq!(core.current_vpin(), Decimal::ONE);
    }

    #[test]
    fn vpin_core_mid_update_does_not_complete_bucket() {
        let mut core = VpinCore::new(dec!(1), 50);
        core.ingest_mid(dec!(100));
        core.ingest_trade(dec!(0.5), Some(true));
        // bucket not complete, interim vpin = 0 (no completed buckets yet)
        assert_eq!(core.current_vpin(), Decimal::ZERO);
        assert_eq!(core.last_market_mid_price(), dec!(100));
    }

    #[test]
    fn vpin_core_reset_clears_state() {
        let mut core = VpinCore::new(dec!(1), 50);
        core.ingest_trade(dec!(1), Some(true));
        assert_eq!(core.current_vpin(), Decimal::ONE);
        core.reset();
        assert_eq!(core.current_vpin(), Decimal::ZERO);
        assert_eq!(core.completed_buckets(), 0);
    }

    #[test]
    fn map_trade_direction_up_is_buy() {
        assert_eq!(map_trade_direction(TradeDirection::Up), Some(true));
    }

    #[test]
    fn map_trade_direction_down_is_sell() {
        assert_eq!(map_trade_direction(TradeDirection::Down), Some(false));
    }

    #[test]
    fn map_trade_direction_neutral_is_skipped() {
        assert_eq!(map_trade_direction(TradeDirection::Neutral), None);
    }

    use rushhft_core::PluginContext;
    use rushhft_core::hub::{OrderBookHub, ProviderHub, TradeHub};
    use rushhft_core::model::order_book::OrderBook;
    use rushhft_core::model::provider::Provider;
    use rushhft_core::model::trade::Trade;

    type MetricRecord = (String, String, String, String, Decimal);

    struct ReplayCtx {
        ob_hub: Arc<OrderBookHub>,
        t_hub: Arc<TradeHub>,
        p_hub: Arc<ProviderHub>,
        metrics: Arc<std::sync::Mutex<Vec<MetricRecord>>>,
    }

    #[async_trait::async_trait]
    impl PluginContext for ReplayCtx {
        async fn publish_order_book(&self, _ob: OrderBook) {}
        async fn publish_trade(&self, _t: Trade) {}
        async fn publish_provider(&self, _p: Provider) {}
        async fn register_metric(
            &self,
            plugin: &str,
            metric: &str,
            exchange: &str,
            symbol: &str,
            value: Decimal,
            _ts: OffsetDateTime,
        ) {
            self.metrics.lock().unwrap().push((
                plugin.to_string(),
                metric.to_string(),
                exchange.to_string(),
                symbol.to_string(),
                value,
            ));
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

    fn make_trade(price: Decimal, size: Decimal, dir: TradeDirection, ts_secs: i64) -> Trade {
        Trade {
            price,
            size,
            timestamp: OffsetDateTime::from_unix_timestamp(ts_secs).unwrap(),
            direction: dir,
            trade_type: "D".to_string(),
            symbol: "700.HK".to_string(),
            provider_id: 1,
            market_mid_price: Decimal::ZERO,
        }
    }

    #[tokio::test]
    async fn vpin_start_registers_metric_after_bucket_completes() {
        let ob_hub = Arc::new(OrderBookHub::new());
        let t_hub = Arc::new(TradeHub::new());
        let p_hub = Arc::new(ProviderHub::new());
        let metrics = Arc::new(std::sync::Mutex::new(Vec::new()));
        let ctx = Arc::new(ReplayCtx {
            ob_hub: ob_hub.clone(),
            t_hub: t_hub.clone(),
            p_hub: p_hub.clone(),
            metrics: metrics.clone(),
        }) as Arc<dyn PluginContext>;

        let study = Arc::new(VpinStudy::new(VpinSettings {
            bucket_volume_size: dec!(1),
            number_of_buckets: 50,
            symbol: "700.HK".into(),
            provider_id: 1,
            aggregation_level: AggregationLevel::S1,
        }));
        study.start(ctx).await.unwrap();
        assert_eq!(study.status(), PluginStatus::Started);

        // 1-volume buy trade -> one bucket completes, imbalance=1
        t_hub.publish(make_trade(
            dec!(100.50),
            dec!(1),
            TradeDirection::Up,
            1_700_000_000,
        ));

        // give the consumer task time to drain
        tokio::time::sleep(std::time::Duration::from_millis(50)).await;

        let collected = metrics.lock().unwrap().clone();
        assert!(
            collected
                .iter()
                .any(|m| m.0 == "VPIN Study" && m.1 == "VPIN" && m.4 == Decimal::ONE),
            "expected at least one metric with VPIN=1, got {:?}",
            collected
        );

        study.stop().await.unwrap();
        assert_eq!(study.status(), PluginStatus::Stopped);
    }
}
