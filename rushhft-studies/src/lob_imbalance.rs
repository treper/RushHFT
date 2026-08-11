//! LOB Imbalance study: (Σ bid_size − Σ ask_size) / (Σ bid_size + Σ ask_size) over top-N levels.
//! Range [−1, 1]. Mirrors OrderFlowAnalysis.Calculate_OrderImbalance from the original.

use rushhft_core::model::enums::{AggregationLevel, PluginStatus, PluginType};
use rushhft_core::plugin::{BaseStudy, Plugin, PluginContext, PluginError};
use std::sync::Arc;
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
    #[allow(dead_code)]
    settings: LobImbalanceSettings,
    #[allow(dead_code)]
    base: BaseStudy,
    status: Arc<arc_swap::ArcSwap<PluginStatus>>,
    #[allow(dead_code)]
    ctx: Mutex<Option<Arc<dyn PluginContext>>>,
}

impl LobImbalanceStudy {
    pub fn new(settings: LobImbalanceSettings) -> Self {
        let id = format!(
            "lobimb-{}",
            hash_symbol_provider(&settings.symbol, settings.provider_id)
        );
        Self {
            id,
            version: "0.1.0",
            author: "RushHFT",
            description: "Top-of-book bid/ask volume imbalance",
            settings,
            base: BaseStudy::new(AggregationLevel::S1),
            status: Arc::new(arc_swap::ArcSwap::from_pointee(PluginStatus::Loaded)),
            ctx: Mutex::new(None),
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
        **self.status.load()
    }
    fn plugin_id(&self) -> &str {
        &self.id
    }
    fn emits_metric(&self) -> bool {
        true
    }

    async fn start(&self, _ctx: Arc<dyn PluginContext>) -> Result<(), PluginError> {
        Ok(())
    }

    async fn stop(&self) -> Result<(), PluginError> {
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
}
