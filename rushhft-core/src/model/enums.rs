use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum SessionStatus {
    Connecting,
    Connected,
    ConnectedWithWarnings,
    DisconnectedFailed,
    Disconnected,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum TradeDirection {
    Neutral,
    Down,
    Up,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum LobSide {
    None,
    Bid,
    Ask,
    Both,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum PluginType {
    Unknown,
    Study,
    MultiStudy,
    MarketConnector,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum PluginStatus {
    Loaded,
    Starting,
    Started,
    Stopping,
    Stopped,
    StoppedFailed,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum MdUpdateAction {
    New,
    Change,
    Delete,
    ChangeAdjust,
    Replace,
    None,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum AggregationLevel {
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn session_status_serializes() {
        let json = serde_json::to_string(&SessionStatus::Connected).unwrap();
        assert_eq!(json, "\"Connected\"");
    }

    #[test]
    fn trade_direction_deserializes() {
        let dir: TradeDirection = serde_json::from_str("\"Up\"").unwrap();
        assert_eq!(dir, TradeDirection::Up);
    }

    #[test]
    fn plugin_status_all_variants() {
        let statuses = vec![
            PluginStatus::Loaded,
            PluginStatus::Starting,
            PluginStatus::Started,
            PluginStatus::Stopping,
            PluginStatus::Stopped,
            PluginStatus::StoppedFailed,
        ];
        for s in &statuses {
            let json = serde_json::to_string(s).unwrap();
            let back: PluginStatus = serde_json::from_str(&json).unwrap();
            assert_eq!(*s, back);
        }
    }

    #[test]
    fn aggregation_level_roundtrip() {
        for level in [
            AggregationLevel::None,
            AggregationLevel::S1,
            AggregationLevel::Ms100,
            AggregationLevel::D1,
        ] {
            let json = serde_json::to_string(&level).unwrap();
            let back: AggregationLevel = serde_json::from_str(&json).unwrap();
            assert_eq!(level, back);
        }
    }
}
