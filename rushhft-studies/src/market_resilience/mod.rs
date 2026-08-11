mod calculator;
#[allow(unused_imports)] // Used by B4 (MR study plugin), not re-exported in B3.
pub use calculator::{MarketResilienceCalculator, MrMetrics};

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
