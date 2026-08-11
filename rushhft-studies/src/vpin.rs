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
}
