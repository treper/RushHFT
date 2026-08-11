mod aggregator;
#[allow(unused_imports)]
pub use aggregator::{compute_ott, OttCounters};

// Imports below are used by the plugin impl (Task B2).  Kept here so the
// module is ready for the next task without another edit pass.
#[allow(unused_imports)]
use rushhft_core::model::enums::{AggregationLevel, PluginStatus, PluginType};
#[allow(unused_imports)]
use rushhft_core::plugin::{BaseStudy, Plugin, PluginContext, PluginError};
#[allow(unused_imports)]
use rushhft_core::hub::SubscriptionGuard;
#[allow(unused_imports)]
use rushhft_core::model::order_book::OrderBook;
#[allow(unused_imports)]
use rushhft_core::model::study::BaseStudyModel;
#[allow(unused_imports)]
use rust_decimal::Decimal;
#[allow(unused_imports)]
use std::sync::Arc;
#[allow(unused_imports)]
use time::OffsetDateTime;
#[allow(unused_imports)]
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
