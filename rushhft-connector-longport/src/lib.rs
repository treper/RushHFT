//! LongPort connector for RushHFT.
//!
//! Thin wrapper around the `longport` SDK crate that implements
//! `rushhft_core::Plugin` and maps `PushEvent` pushes to normalized
//! `rushhft_core` domain models.

#[derive(Debug, Clone)]
pub struct ConnectorSettings {
    pub app_key: String,
    pub app_secret: String,
    pub access_token: String,
    pub symbols: Vec<String>,
    pub depth_levels: usize,
    pub price_decimal_places: u8,
    pub size_decimal_places: u8,
    pub provider_id: i32,
    pub sub_flags: longport::quote::SubFlags,
}

impl Default for ConnectorSettings {
    fn default() -> Self {
        Self {
            app_key: String::new(),
            app_secret: String::new(),
            access_token: String::new(),
            symbols: vec!["700.HK".to_string()],
            depth_levels: 10,
            price_decimal_places: 2,
            size_decimal_places: 0,
            provider_id: 1,
            sub_flags: longport::quote::SubFlags::DEPTH
                | longport::quote::SubFlags::BROKER
                | longport::quote::SubFlags::TRADE
                | longport::quote::SubFlags::QUOTE,
        }
    }
}

impl ConnectorSettings {
    pub fn from_settings(s: &rushhft_core::Settings) -> Self {
        Self {
            app_key: s.app_key.clone(),
            app_secret: s.app_secret.clone(),
            access_token: s.access_token.clone(),
            symbols: s.default_symbols.clone(),
            depth_levels: s.depth_levels,
            price_decimal_places: 2,
            size_decimal_places: 0,
            provider_id: 1,
            sub_flags: longport::quote::SubFlags::DEPTH
                | longport::quote::SubFlags::BROKER
                | longport::quote::SubFlags::TRADE
                | longport::quote::SubFlags::QUOTE,
        }
    }
}

#[derive(Debug, Clone)]
pub struct QuoteStats {
    pub last_done: rust_decimal::Decimal,
    pub open: rust_decimal::Decimal,
    pub high: rust_decimal::Decimal,
    pub low: rust_decimal::Decimal,
    pub volume: i64,
    pub turnover: rust_decimal::Decimal,
    pub trade_status: String,
    pub timestamp: time::OffsetDateTime,
}

impl From<longport::quote::PushQuote> for QuoteStats {
    fn from(q: longport::quote::PushQuote) -> Self {
        Self {
            last_done: q.last_done,
            open: q.open,
            high: q.high,
            low: q.low,
            volume: q.volume,
            turnover: q.turnover,
            trade_status: format!("{:?}", q.trade_status),
            timestamp: q.timestamp,
        }
    }
}

/// Map a `longport::quote::TradeDirection` to the core `TradeDirection`.
///
/// Implemented as a free function rather than a `From` impl because the
/// orphan rule forbids `impl From<ForeignType> for OtherForeignType` —
/// neither `longport::quote::TradeDirection` nor `rushhft_core::TradeDirection`
/// is defined in this crate.
pub fn map_trade_direction(
    d: longport::quote::TradeDirection,
) -> rushhft_core::TradeDirection {
    match d {
        longport::quote::TradeDirection::Neutral => rushhft_core::TradeDirection::Neutral,
        longport::quote::TradeDirection::Down => rushhft_core::TradeDirection::Down,
        longport::quote::TradeDirection::Up => rushhft_core::TradeDirection::Up,
    }
}

use std::sync::atomic::AtomicBool;
use std::sync::Arc;

use dashmap::DashMap;
use rushhft_core::plugin::BaseDataRetriever;

#[allow(clippy::type_complexity)]
#[allow(dead_code)]
struct Inner {
    settings: ConnectorSettings,
    local_books: DashMap<String, rushhft_core::OrderBook>,
    quote_stats: DashMap<String, QuoteStats>,
    stop_flag: AtomicBool,
    quote_ctx: tokio::sync::Mutex<Option<Arc<longport::QuoteContext>>>,
    ctx: tokio::sync::Mutex<Option<Arc<dyn rushhft_core::plugin::PluginContext>>>,
    status: arc_swap::ArcSwap<rushhft_core::PluginStatus>,
}

#[allow(dead_code)]
pub struct LongPortConnector {
    id: String,
    version: String,
    author: String,
    description: String,
    inner: Arc<Inner>,
    base: BaseDataRetriever,
}

impl LongPortConnector {
    pub fn new(settings: ConnectorSettings) -> Self {
        let id = format!(
            "{:x}",
            fnv1a_64(&format!(
                "LongPortConnector{}{}{}",
                settings.provider_id, settings.app_key, settings.symbols.join(",")
            ))
        );
        Self {
            id,
            version: "0.1.0".to_string(),
            author: "RushHFT".to_string(),
            description: "LongPort OpenAPI connector (HK/US equities)".to_string(),
            inner: Arc::new(Inner {
                settings,
                local_books: DashMap::new(),
                quote_stats: DashMap::new(),
                stop_flag: AtomicBool::new(false),
                quote_ctx: tokio::sync::Mutex::new(None),
                ctx: tokio::sync::Mutex::new(None),
                status: arc_swap::ArcSwap::from_pointee(rushhft_core::PluginStatus::Loaded),
            }),
            base: BaseDataRetriever::new_default(),
        }
    }

    pub fn from_settings(s: &rushhft_core::Settings) -> Self {
        Self::new(ConnectorSettings::from_settings(s))
    }

    pub fn quote_stats(&self, symbol: &str) -> Option<QuoteStats> {
        self.inner.quote_stats.get(symbol).map(|e| e.clone())
    }

    pub fn local_book(&self, symbol: &str) -> Option<rushhft_core::OrderBook> {
        self.inner.local_books.get(symbol).map(|e| e.clone())
    }
}

/// FNV-1a 64-bit hash — stable, non-cryptographic identifier for plugin_id.
fn fnv1a_64(s: &str) -> u64 {
    let mut hash: u64 = 0xcbf29ce484222325;
    for b in s.as_bytes() {
        hash ^= *b as u64;
        hash = hash.wrapping_mul(0x100000001b3);
    }
    hash
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_settings_have_longport_sub_flags() {
        let s = ConnectorSettings::default();
        assert!(s.sub_flags.contains(longport::quote::SubFlags::DEPTH));
        assert!(s.sub_flags.contains(longport::quote::SubFlags::BROKER));
        assert!(s.sub_flags.contains(longport::quote::SubFlags::TRADE));
        assert!(s.sub_flags.contains(longport::quote::SubFlags::QUOTE));
        assert_eq!(s.depth_levels, 10);
        assert_eq!(s.provider_id, 1);
    }

    #[test]
    fn from_settings_maps_core_fields() {
        let mut core = rushhft_core::Settings::default();
        core.app_key = "key".into();
        core.app_secret = "secret".into();
        core.access_token = "tok".into();
        core.default_symbols = vec!["700.HK".into(), "AAPL.US".into()];
        core.depth_levels = 20;
        let cs = ConnectorSettings::from_settings(&core);
        assert_eq!(cs.app_key, "key");
        assert_eq!(cs.access_token, "tok");
        assert_eq!(cs.symbols, vec!["700.HK", "AAPL.US"]);
        assert_eq!(cs.depth_levels, 20);
    }

    #[test]
    fn quote_stats_from_push_quote() {
        use rust_decimal_macros::dec;
        let q = longport::quote::PushQuote {
            last_done: dec!(350.00),
            open: dec!(345.00),
            high: dec!(352.00),
            low: dec!(344.00),
            timestamp: time::OffsetDateTime::from_unix_timestamp(1_700_000_000).unwrap(),
            volume: 1_000_000,
            turnover: dec!(350_000_000),
            trade_status: longport::quote::TradeStatus::Normal,
            trade_session: longport::quote::TradeSession::Intraday,
            current_volume: 5_000,
            current_turnover: dec!(1_750_000),
        };
        let stats: QuoteStats = q.into();
        assert_eq!(stats.last_done, dec!(350.00));
        assert_eq!(stats.high, dec!(352.00));
        assert_eq!(stats.volume, 1_000_000);
        assert_eq!(stats.timestamp.unix_timestamp(), 1_700_000_000);
        assert!(stats.trade_status.contains("Normal"));
    }

    #[test]
    fn trade_direction_mapping() {
        use rushhft_core::TradeDirection;
        assert_eq!(
            map_trade_direction(longport::quote::TradeDirection::Up),
            TradeDirection::Up
        );
        assert_eq!(
            map_trade_direction(longport::quote::TradeDirection::Down),
            TradeDirection::Down
        );
        assert_eq!(
            map_trade_direction(longport::quote::TradeDirection::Neutral),
            TradeDirection::Neutral
        );
    }

    #[test]
    fn connector_new_has_loaded_status() {
        let c = LongPortConnector::new(ConnectorSettings::default());
        assert_eq!(**c.inner.status.load(), rushhft_core::PluginStatus::Loaded);
        assert!(!c.id.is_empty());
        assert_eq!(c.version, "0.1.0");
        assert_eq!(c.author, "RushHFT");
    }

    #[test]
    fn connector_local_book_empty_initially() {
        let c = LongPortConnector::new(ConnectorSettings::default());
        assert!(c.local_book("700.HK").is_none());
        assert!(c.quote_stats("700.HK").is_none());
    }

    #[test]
    fn connector_id_is_stable_for_same_settings() {
        let s = ConnectorSettings::default();
        let c1 = LongPortConnector::new(s.clone());
        let c2 = LongPortConnector::new(s);
        assert_eq!(c1.id, c2.id);
    }
}
