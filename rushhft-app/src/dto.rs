//! Frontend-facing DTOs. Decimal → string (rust_decimal default),
//! OffsetDateTime → epoch millis (i64).
#![allow(dead_code)]

use rust_decimal::Decimal;
use serde::Serialize;

#[derive(Serialize, Clone, Debug)]
pub struct BookItemDto {
    pub price: Decimal,
    pub size: Decimal,
    pub cumulative_size: Decimal,
    pub is_bid: bool,
    pub broker_ids: Vec<i32>,
}

#[derive(Serialize, Clone, Debug)]
pub struct TradeDto {
    pub price: Decimal,
    pub size: Decimal,
    pub timestamp: i64,
    pub direction: TradeDirectionDto,
    pub trade_type: String,
}

#[derive(Serialize, Clone, Copy, Debug, PartialEq, Eq)]
#[serde(rename_all = "PascalCase")]
pub enum TradeDirectionDto {
    Neutral,
    Down,
    Up,
}

#[derive(Serialize, Clone, Debug)]
pub struct ProviderDto {
    pub id: i32,
    pub name: String,
    pub status: SessionStatusDto,
}

#[derive(Serialize, Clone, Copy, Debug, PartialEq, Eq)]
#[serde(rename_all = "PascalCase")]
pub enum SessionStatusDto {
    Connecting,
    Connected,
    ConnectedWithWarnings,
    DisconnectedFailed,
    Disconnected,
}

#[derive(Serialize, Clone, Debug)]
pub struct QuoteStatsDto {
    pub last_done: Decimal,
    pub open: Decimal,
    pub high: Decimal,
    pub low: Decimal,
    pub volume: i64,
    pub turnover: Decimal,
    pub trade_status: TradeStatusDto,
    pub timestamp: i64,
}

#[derive(Serialize, Clone, Copy, Debug, PartialEq, Eq)]
#[serde(rename_all = "PascalCase")]
pub enum TradeStatusDto {
    Normal,
    Halted,
    Closing,
}

#[derive(Serialize, Clone, Debug)]
pub struct StudyValueDto {
    pub name: String,
    pub value: Decimal,
    pub format: String,
    pub value_color: String,
    pub tooltip: String,
    pub has_error: bool,
    pub is_stale: bool,
    pub timestamp: i64,
}

#[derive(Serialize, Clone, Debug)]
pub struct SnapshotDto {
    pub symbol: String,
    pub bids: Vec<BookItemDto>,
    pub asks: Vec<BookItemDto>,
    pub spread: Decimal,
    pub mid_price: Decimal,
    pub last_updated: i64,
    pub sequence: i64,
    pub provider_status: SessionStatusDto,
    pub studies: Vec<StudyValueDto>,
    pub recent_trades: Vec<TradeDto>,
    pub quote_stats: Option<QuoteStatsDto>,
}

#[derive(Serialize, Clone, Debug)]
pub struct StudyDescriptorDto {
    pub plugin_id: String,
    pub name: String,
    pub version: String,
    pub description: String,
    pub plugin_type: PluginTypeDto,
    pub status: PluginStatusDto,
    pub emits_metric: bool,
}

#[derive(Serialize, Clone, Copy, Debug, PartialEq, Eq)]
#[serde(rename_all = "PascalCase")]
pub enum PluginTypeDto {
    Unknown,
    Study,
    MultiStudy,
    MarketConnector,
}

#[derive(Serialize, Clone, Copy, Debug, PartialEq, Eq)]
#[serde(rename_all = "PascalCase")]
pub enum PluginStatusDto {
    Loaded,
    Starting,
    Started,
    Stopping,
    Stopped,
    StoppedFailed,
}

#[derive(Serialize, Clone, Debug)]
pub struct SettingsDto {
    pub app_key: String,
    pub app_secret_masked: String,
    pub access_token_masked: String,
    pub default_symbols: Vec<String>,
    pub depth_levels: usize,
    pub aggregation_level: AggregationLevelDto,
    pub log_level: String,
}

#[derive(Serialize, Clone, Copy, Debug, PartialEq, Eq)]
#[serde(rename_all = "PascalCase")]
pub enum AggregationLevelDto {
    None,
    Ms1,
    Ms10,
    Ms100,
    Ms500,
    S1,
    S3,
    S5,
    D1,
}

#[derive(Serialize, Clone, Debug)]
pub struct NotificationPayload {
    pub source: String,
    pub message: String,
    pub level: NotificationLevelDto,
    pub category: NotificationCategoryDto,
    pub timestamp: i64,
    pub exception: Option<String>,
}

#[derive(Serialize, Clone, Copy, Debug, PartialEq, Eq)]
#[serde(rename_all = "PascalCase")]
pub enum NotificationLevelDto {
    Info,
    Warning,
    Error,
}

#[derive(Serialize, Clone, Copy, Debug, PartialEq, Eq)]
#[serde(rename_all = "PascalCase")]
pub enum NotificationCategoryDto {
    Plugin,
    TriggerEngine,
    System,
}

#[derive(Serialize, Clone, Debug)]
pub struct TriggerRuleDto {
    pub rule_id: i64,
    pub name: String,
    pub is_enabled: bool,
    pub conditions: Vec<TriggerConditionDto>,
    pub actions: Vec<TriggerActionDto>,
}

#[derive(Serialize, Clone, Debug)]
pub struct TriggerConditionDto {
    pub condition_id: i64,
    pub plugin: String,
    pub metric: String,
    pub exchange: String,
    pub symbol: String,
    pub operator: String,
    pub threshold: Decimal,
    pub window_seconds: Option<i32>,
}

#[derive(Serialize, Clone, Debug)]
pub struct TriggerActionDto {
    pub action_type: String,
    pub cooldown_seconds: i32,
    pub rest_url: Option<String>,
    pub rest_method: Option<String>,
    pub rest_body: Option<String>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use rust_decimal_macros::dec;

    #[test]
    fn snapshot_dto_serializes_decimal_as_string() {
        let snap = SnapshotDto {
            symbol: "700.HK".into(),
            bids: vec![BookItemDto {
                price: dec!(100.50),
                size: dec!(500),
                cumulative_size: dec!(500),
                is_bid: true,
                broker_ids: vec![1001],
            }],
            asks: vec![],
            spread: dec!(0.10),
            mid_price: dec!(100.55),
            last_updated: 1_700_000_000_000,
            sequence: 1,
            provider_status: SessionStatusDto::Connected,
            studies: vec![],
            recent_trades: vec![],
            quote_stats: None,
        };
        let json = serde_json::to_string(&snap).unwrap();
        // Decimal should appear as string "100.50", not number 100.5
        assert!(json.contains("\"price\":\"100.50\""), "got: {}", json);
        assert!(json.contains("\"symbol\":\"700.HK\""));
    }

    #[test]
    fn plugin_status_dto_serializes_pascal_case() {
        let json = serde_json::to_string(&PluginStatusDto::Started).unwrap();
        assert_eq!(json, "\"Started\"");
    }

    #[test]
    fn trade_direction_dto_pascal_case() {
        assert_eq!(
            serde_json::to_string(&TradeDirectionDto::Up).unwrap(),
            "\"Up\""
        );
    }

    #[test]
    fn notification_payload_round_trips() {
        let p = NotificationPayload {
            source: "VPIN Study".into(),
            message: "toxicity high".into(),
            level: NotificationLevelDto::Warning,
            category: NotificationCategoryDto::Plugin,
            timestamp: 1_700_000_000_000,
            exception: None,
        };
        let json = serde_json::to_string(&p).unwrap();
        assert!(json.contains("\"level\":\"Warning\""));
        assert!(json.contains("\"category\":\"Plugin\""));
    }
}
