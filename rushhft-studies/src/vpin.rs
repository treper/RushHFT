//! VPIN (Volume-Synchronized Probability of Informed Trading) study.
//!
//! Easley, Lopez de Prado & O'Hara (2012). VPIN = (1/n) × Σ|V_buy_i − V_sell_i| / V_bucket
//! over n completed volume buckets. Range [0, 1].

use rushhft_core::model::enums::{AggregationLevel, PluginStatus, PluginType};
use rushhft_core::plugin::{BaseStudy, Plugin, PluginContext, PluginError};
use rust_decimal::Decimal;
use std::sync::Arc;
use tokio::sync::Mutex;

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

/// VPIN study plugin.
pub struct VpinStudy {
    id: String,
    version: &'static str,
    author: &'static str,
    description: &'static str,
    #[allow(dead_code)]
    settings: VpinSettings,
    #[allow(dead_code)]
    base: BaseStudy,
    status: Arc<arc_swap::ArcSwap<PluginStatus>>,
    #[allow(dead_code)]
    ctx: Mutex<Option<Arc<dyn PluginContext>>>,
}

impl VpinStudy {
    pub fn new(settings: VpinSettings) -> Self {
        let id = format!(
            "vpin-{}",
            hash_symbol_provider(&settings.symbol, settings.provider_id)
        );
        Self {
            id,
            version: "0.1.0",
            author: "RushHFT",
            description: "Volume-Synchronized Probability of Informed Trading",
            settings,
            base: BaseStudy::new(AggregationLevel::S1),
            status: Arc::new(arc_swap::ArcSwap::from_pointee(PluginStatus::Loaded)),
            ctx: Mutex::new(None),
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
        **self.status.load()
    }
    fn plugin_id(&self) -> &str {
        &self.id
    }
    fn emits_metric(&self) -> bool {
        true
    }

    async fn start(&self, _ctx: Arc<dyn PluginContext>) -> Result<(), PluginError> {
        // Real implementation lands in Task 5.
        Ok(())
    }

    async fn stop(&self) -> Result<(), PluginError> {
        // Real implementation lands in Task 5.
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

/// Pure VPIN bucket arithmetic — no I/O, no async. Owned by `VpinStudy`.
#[allow(dead_code)]
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

#[allow(dead_code)]
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

    pub fn current_bucket_volume(&self) -> Decimal {
        self.current_bucket_volume
    }

    pub fn last_market_mid_price(&self) -> Decimal {
        self.last_market_mid_price
    }

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
}
