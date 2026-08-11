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

    pub async fn on_depth(&self, symbol: &str, d: longport::quote::PushDepth) {
        Self::on_depth_inner(&self.inner, symbol, d).await;
    }

    pub async fn on_brokers(&self, symbol: &str, b: longport::quote::PushBrokers) {
        Self::on_brokers_inner(&self.inner, symbol, b).await;
    }

    pub async fn on_trade(&self, symbol: &str, t: longport::quote::PushTrades) {
        Self::on_trade_inner(&self.inner, symbol, t).await;
    }

    pub async fn on_quote(&self, symbol: &str, q: longport::quote::PushQuote) {
        Self::on_quote_inner(&self.inner, symbol, q).await;
    }

    async fn on_quote_inner(
        inner: &Arc<Inner>,
        symbol: &str,
        q: longport::quote::PushQuote,
    ) {
        let stats: QuoteStats = q.into();
        inner.quote_stats.insert(symbol.to_string(), stats);
    }

    async fn handle_push_event(inner: &Arc<Inner>, event: longport::quote::PushEvent) {
        let symbol = event.symbol;
        match event.detail {
            longport::quote::PushEventDetail::Depth(d) => {
                Self::on_depth_inner(inner, &symbol, d).await;
            }
            longport::quote::PushEventDetail::Brokers(b) => {
                Self::on_brokers_inner(inner, &symbol, b).await;
            }
            longport::quote::PushEventDetail::Trade(t) => {
                Self::on_trade_inner(inner, &symbol, t).await;
            }
            longport::quote::PushEventDetail::Quote(q) => {
                Self::on_quote_inner(inner, &symbol, q).await;
            }
            longport::quote::PushEventDetail::Candlestick(_) => {}
        }
    }

    async fn internal_start(inner: Arc<Inner>) -> Result<(), rushhft_core::PluginError> {
        let settings = &inner.settings;
        if settings.app_key.is_empty() {
            return Err(rushhft_core::PluginError::StartFailed(
                "missing app_key".to_string(),
            ));
        }

        let config = longport::Config::from_apikey(
            settings.app_key.clone(),
            settings.app_secret.clone(),
            settings.access_token.clone(),
        );
        let (quote_ctx, mut receiver) = longport::QuoteContext::new(Arc::new(config));
        let quote_ctx = Arc::new(quote_ctx);

        let symbols: Vec<&str> = settings.symbols.iter().map(|s| s.as_str()).collect();
        quote_ctx
            .subscribe(symbols.iter().copied(), settings.sub_flags)
            .await
            .map_err(|e| {
                rushhft_core::PluginError::StartFailed(format!("subscribe failed: {}", e))
            })?;

        *inner.quote_ctx.lock().await = Some(quote_ctx);

        // Spawn consumer task.
        let inner2 = inner.clone();
        tokio::spawn(async move {
            tracing::info!("LongPort consumer task started");
            loop {
                match receiver.recv().await {
                    Some(event) => Self::handle_push_event(&inner2, event).await,
                    None => break,
                }
            }
            tracing::info!("LongPort consumer task stopped");
            inner2
                .status
                .store(Arc::new(rushhft_core::PluginStatus::Stopped));
        });

        Ok(())
    }

    async fn on_trade_inner(
        inner: &Arc<Inner>,
        symbol: &str,
        t: longport::quote::PushTrades,
    ) {
        let provider_id = inner.settings.provider_id;
        let mid_price = inner
            .local_books
            .get(symbol)
            .and_then(|b| b.mid_price())
            .unwrap_or(rust_decimal::Decimal::ZERO);

        let ctx = { inner.ctx.lock().await.clone() };
        let Some(ctx) = ctx else { return };

        for trade in t.trades {
            let normalized = rushhft_core::Trade {
                price: trade.price,
                size: rust_decimal::Decimal::from(trade.volume),
                timestamp: trade.timestamp,
                direction: map_trade_direction(trade.direction),
                trade_type: trade.trade_type,
                symbol: symbol.to_string(),
                provider_id,
                market_mid_price: mid_price,
            };
            ctx.publish_trade(normalized).await;
        }
    }

    async fn on_brokers_inner(
        inner: &Arc<Inner>,
        symbol: &str,
        b: longport::quote::PushBrokers,
    ) {
        let book_for_publish = {
            let Some(mut book_ref) = inner.local_books.get_mut(symbol) else {
                return; // No depth yet — brokers cannot be merged.
            };
            let book = book_ref.value_mut();
            for broker_entry in b.ask_brokers {
                let idx = (broker_entry.position as usize).saturating_sub(1);
                if idx < book.asks.len() {
                    book.asks[idx].broker_ids = broker_entry.broker_ids;
                }
            }
            for broker_entry in b.bid_brokers {
                let idx = (broker_entry.position as usize).saturating_sub(1);
                if idx < book.bids.len() {
                    book.bids[idx].broker_ids = broker_entry.broker_ids;
                }
            }
            book.clone()
        }; // book_ref (DashMap RefMut) dropped here — safe to await.

        let ctx = { inner.ctx.lock().await.clone() };
        if let Some(ctx) = ctx {
            ctx.publish_order_book(book_for_publish).await;
        }
    }

    async fn on_depth_inner(
        inner: &Arc<Inner>,
        symbol: &str,
        d: longport::quote::PushDepth,
    ) {
        let settings = &inner.settings;
        let provider_id = settings.provider_id;

        // Preserve existing broker_ids per price level before replacing.
        let mut broker_map: std::collections::HashMap<
            rust_decimal::Decimal,
            Vec<i32>,
        > = std::collections::HashMap::new();
        if let Some(book) = inner.local_books.get(symbol) {
            for item in book.bids.iter().chain(book.asks.iter()) {
                if !item.broker_ids.is_empty() {
                    broker_map.insert(item.price, item.broker_ids.clone());
                }
            }
        }

        let mut book = rushhft_core::OrderBook::new(
            symbol,
            settings.depth_levels,
            settings.price_decimal_places,
            settings.size_decimal_places,
            provider_id,
        );

        for depth in d.bids {
            if let Some(price) = depth.price {
                let size = rust_decimal::Decimal::from(depth.volume);
                let mut item = rushhft_core::BookItem::new(
                    price, size, true, symbol, provider_id,
                );
                if let Some(brokers) = broker_map.get(&price) {
                    item.broker_ids = brokers.clone();
                }
                book.add_or_update_level(item);
            }
        }
        for depth in d.asks {
            if let Some(price) = depth.price {
                let size = rust_decimal::Decimal::from(depth.volume);
                let mut item = rushhft_core::BookItem::new(
                    price, size, false, symbol, provider_id,
                );
                if let Some(brokers) = broker_map.get(&price) {
                    item.broker_ids = brokers.clone();
                }
                book.add_or_update_level(item);
            }
        }

        let book_for_publish = book.clone();
        inner.local_books.insert(symbol.to_string(), book);

        let ctx = { inner.ctx.lock().await.clone() };
        if let Some(ctx) = ctx {
            ctx.publish_order_book(book_for_publish).await;
        }
    }
}

#[async_trait::async_trait]
impl rushhft_core::plugin::Plugin for LongPortConnector {
    fn name(&self) -> &str {
        "LongPort Connector"
    }
    fn version(&self) -> &str {
        &self.version
    }
    fn author(&self) -> &str {
        &self.author
    }
    fn description(&self) -> &str {
        &self.description
    }
    fn plugin_type(&self) -> rushhft_core::PluginType {
        rushhft_core::PluginType::MarketConnector
    }
    fn status(&self) -> rushhft_core::PluginStatus {
        **self.inner.status.load()
    }
    fn plugin_id(&self) -> &str {
        &self.id
    }

    async fn start(
        &self,
        ctx: Arc<dyn rushhft_core::plugin::PluginContext>,
    ) -> Result<(), rushhft_core::PluginError> {
        use rushhft_core::model::provider::Provider;
        use rushhft_core::model::enums::SessionStatus;

        let cur = **self.inner.status.load();
        if cur == rushhft_core::PluginStatus::Started
            || cur == rushhft_core::PluginStatus::Starting
        {
            return Err(rushhft_core::PluginError::AlreadyRunning(
                self.name().to_string(),
            ));
        }
        self.inner
            .status
            .store(Arc::new(rushhft_core::PluginStatus::Starting));

        // Early credential check (avoids burning reconnect attempts).
        if self.inner.settings.app_key.is_empty() {
            self.inner
                .status
                .store(Arc::new(rushhft_core::PluginStatus::StoppedFailed));
            ctx.publish_provider(Provider {
                id: self.inner.settings.provider_id,
                name: "LongPort".to_string(),
                status: SessionStatus::DisconnectedFailed,
            })
            .await;
            return Err(rushhft_core::PluginError::StartFailed(
                "missing app_key".to_string(),
            ));
        }

        *self.inner.ctx.lock().await = Some(ctx.clone());

        let inner = self.inner.clone();
        let result = self
            .base
            .start_with_reconnect(ctx.clone(), move || {
                let inner = inner.clone();
                Box::pin(async move { Self::internal_start(inner).await })
            })
            .await;

        let provider_id = self.inner.settings.provider_id;
        match &result {
            Ok(()) => {
                self.inner
                    .status
                    .store(Arc::new(rushhft_core::PluginStatus::Started));
                ctx.publish_provider(Provider {
                    id: provider_id,
                    name: "LongPort".to_string(),
                    status: SessionStatus::Connected,
                })
                .await;
            }
            Err(e) => {
                self.inner
                    .status
                    .store(Arc::new(rushhft_core::PluginStatus::StoppedFailed));
                tracing::error!(error = %e, "LongPort connector failed to start");
                ctx.publish_provider(Provider {
                    id: provider_id,
                    name: "LongPort".to_string(),
                    status: SessionStatus::DisconnectedFailed,
                })
                .await;
            }
        }
        result
    }

    async fn stop(&self) -> Result<(), rushhft_core::PluginError> {
        use rushhft_core::model::provider::Provider;
        use rushhft_core::model::enums::SessionStatus;

        self.inner
            .status
            .store(Arc::new(rushhft_core::PluginStatus::Stopping));
        self.inner
            .stop_flag
            .store(true, std::sync::atomic::Ordering::Relaxed);

        // Drop the QuoteContext — cascade stops the consumer.
        let _ = self.inner.quote_ctx.lock().await.take();

        self.inner
            .status
            .store(Arc::new(rushhft_core::PluginStatus::Stopped));

        let ctx = { self.inner.ctx.lock().await.clone() };
        if let Some(ctx) = ctx {
            ctx.publish_provider(Provider {
                id: self.inner.settings.provider_id,
                name: "LongPort".to_string(),
                status: SessionStatus::Disconnected,
            })
            .await;
        }
        Ok(())
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

    use async_trait::async_trait;
    use rushhft_core::plugin::PluginContext;
    use rushhft_core::Plugin;
    use rushhft_core::{
        hub::{OrderBookHub, ProviderHub, TradeHub},
        model::provider::Provider,
    };
    use rust_decimal::Decimal;
    use time::OffsetDateTime;

    struct MockCtx {
        ob_hub: Arc<OrderBookHub>,
        t_hub: Arc<TradeHub>,
        p_hub: Arc<ProviderHub>,
        published_obs: Arc<dashmap::DashMap<String, rushhft_core::OrderBook>>,
        published_trades: Arc<std::sync::Mutex<Vec<rushhft_core::Trade>>>,
        published_providers: Arc<std::sync::Mutex<Vec<Provider>>>,
    }

    impl MockCtx {
        fn new() -> Self {
            Self {
                ob_hub: Arc::new(OrderBookHub::new()),
                t_hub: Arc::new(TradeHub::new()),
                p_hub: Arc::new(ProviderHub::new()),
                published_obs: Arc::new(dashmap::DashMap::new()),
                published_trades: Arc::new(std::sync::Mutex::new(Vec::new())),
                published_providers: Arc::new(std::sync::Mutex::new(Vec::new())),
            }
        }
    }

    #[async_trait]
    impl PluginContext for MockCtx {
        async fn publish_order_book(&self, ob: rushhft_core::OrderBook) {
            self.published_obs.insert(ob.symbol.clone(), ob);
        }
        async fn publish_trade(&self, t: rushhft_core::Trade) {
            self.published_trades.lock().unwrap().push(t);
        }
        async fn publish_provider(&self, p: Provider) {
            self.published_providers.lock().unwrap().push(p);
        }
        async fn register_metric(
            &self, _: &str, _: &str, _: &str, _: &str, _: Decimal, _: OffsetDateTime,
        ) {}
        fn order_book_hub(&self) -> Arc<OrderBookHub> { self.ob_hub.clone() }
        fn trade_hub(&self) -> Arc<TradeHub> { self.t_hub.clone() }
        fn provider_hub(&self) -> Arc<ProviderHub> { self.p_hub.clone() }
    }

    fn test_connector() -> LongPortConnector {
        LongPortConnector::new(ConnectorSettings {
            symbols: vec!["700.HK".into()],
            depth_levels: 10,
            price_decimal_places: 2,
            size_decimal_places: 0,
            ..ConnectorSettings::default()
        })
    }

    #[tokio::test]
    async fn on_depth_maps_push_depth_to_order_book() {
        use rust_decimal_macros::dec;
        let c = test_connector();
        let ctx = Arc::new(MockCtx::new());
        c.inner
            .ctx
            .lock()
            .await
            .replace(ctx.clone() as Arc<dyn PluginContext>);

        let push = longport::quote::PushDepth {
            asks: vec![
                longport::quote::Depth {
                    position: 1,
                    price: Some(dec!(100.60)),
                    volume: 400,
                    order_num: 4,
                },
                longport::quote::Depth {
                    position: 2,
                    price: Some(dec!(100.65)),
                    volume: 200,
                    order_num: 2,
                },
            ],
            bids: vec![
                longport::quote::Depth {
                    position: 1,
                    price: Some(dec!(100.55)),
                    volume: 500,
                    order_num: 5,
                },
                longport::quote::Depth {
                    position: 2,
                    price: Some(dec!(100.50)),
                    volume: 300,
                    order_num: 3,
                },
            ],
        };
        c.on_depth("700.HK", push).await;

        let book = c.local_book("700.HK").unwrap();
        assert_eq!(book.bids.len(), 2);
        assert_eq!(book.bids[0].price, dec!(100.55)); // desc
        assert_eq!(book.bids[1].price, dec!(100.50));
        assert_eq!(book.asks.len(), 2);
        assert_eq!(book.asks[0].price, dec!(100.60)); // asc
        assert_eq!(book.asks[1].price, dec!(100.65));
        assert_eq!(book.bids[0].cumulative_size, dec!(500));
        assert_eq!(book.bids[1].cumulative_size, dec!(800));
        assert!(book.mid_price().unwrap() == dec!(100.575));

        // Published
        let published = ctx.published_obs.get("700.HK").unwrap();
        assert_eq!(published.bids.len(), 2);
    }

    #[tokio::test]
    async fn on_brokers_merges_broker_ids_into_existing_levels() {
        use rust_decimal_macros::dec;
        let c = test_connector();
        let ctx = Arc::new(MockCtx::new());
        c.inner
            .ctx
            .lock()
            .await
            .replace(ctx.clone() as Arc<dyn PluginContext>);

        // First push a depth so the book exists.
        c.on_depth(
            "700.HK",
            longport::quote::PushDepth {
                asks: vec![longport::quote::Depth {
                    position: 1,
                    price: Some(dec!(100.60)),
                    volume: 400,
                    order_num: 4,
                }],
                bids: vec![longport::quote::Depth {
                    position: 1,
                    price: Some(dec!(100.55)),
                    volume: 500,
                    order_num: 5,
                }],
            },
        )
        .await;

        // Now push brokers — position 1 → asks[0] / bids[0].
        c.on_brokers(
            "700.HK",
            longport::quote::PushBrokers {
                ask_brokers: vec![longport::quote::Brokers {
                    position: 1,
                    broker_ids: vec![1001, 1002],
                }],
                bid_brokers: vec![longport::quote::Brokers {
                    position: 1,
                    broker_ids: vec![2001, 2002, 2003],
                }],
            },
        )
        .await;

        let book = c.local_book("700.HK").unwrap();
        assert_eq!(book.asks[0].broker_ids, vec![1001, 1002]);
        assert_eq!(book.bids[0].broker_ids, vec![2001, 2002, 2003]);
    }

    #[tokio::test]
    async fn on_brokers_is_noop_when_no_depth_exists() {
        let c = test_connector();
        // No depth pushed yet.
        c.on_brokers(
            "700.HK",
            longport::quote::PushBrokers {
                ask_brokers: vec![longport::quote::Brokers {
                    position: 1,
                    broker_ids: vec![1001],
                }],
                bid_brokers: vec![],
            },
        )
        .await;
        assert!(c.local_book("700.HK").is_none());
    }

    #[tokio::test]
    async fn on_depth_preserves_broker_ids_across_refresh() {
        use rust_decimal_macros::dec;
        let c = test_connector();
        let ctx = Arc::new(MockCtx::new());
        c.inner
            .ctx
            .lock()
            .await
            .replace(ctx.clone() as Arc<dyn PluginContext>);

        // Depth + brokers.
        c.on_depth(
            "700.HK",
            longport::quote::PushDepth {
                asks: vec![longport::quote::Depth {
                    position: 1,
                    price: Some(dec!(100.60)),
                    volume: 400,
                    order_num: 4,
                }],
                bids: vec![longport::quote::Depth {
                    position: 1,
                    price: Some(dec!(100.55)),
                    volume: 500,
                    order_num: 5,
                }],
            },
        )
        .await;
        c.on_brokers(
            "700.HK",
            longport::quote::PushBrokers {
                ask_brokers: vec![longport::quote::Brokers {
                    position: 1,
                    broker_ids: vec![1001, 1002],
                }],
                bid_brokers: vec![],
            },
        )
        .await;

        // Second depth refresh at same price should preserve broker_ids.
        c.on_depth(
            "700.HK",
            longport::quote::PushDepth {
                asks: vec![longport::quote::Depth {
                    position: 1,
                    price: Some(dec!(100.60)),
                    volume: 600, // volume changed
                    order_num: 6,
                }],
                bids: vec![longport::quote::Depth {
                    position: 1,
                    price: Some(dec!(100.55)),
                    volume: 500,
                    order_num: 5,
                }],
            },
        )
        .await;

        let book = c.local_book("700.HK").unwrap();
        assert_eq!(book.asks[0].size, dec!(600)); // volume updated
        assert_eq!(book.asks[0].broker_ids, vec![1001, 1002]); // brokers preserved
    }

    #[tokio::test]
    async fn on_trade_maps_push_trades_and_uses_local_mid_price() {
        use rust_decimal_macros::dec;
        let c = test_connector();
        let ctx = Arc::new(MockCtx::new());
        c.inner
            .ctx
            .lock()
            .await
            .replace(ctx.clone() as Arc<dyn PluginContext>);

        // Push a depth so mid_price is known.
        c.on_depth(
            "700.HK",
            longport::quote::PushDepth {
                asks: vec![longport::quote::Depth {
                    position: 1,
                    price: Some(dec!(100.60)),
                    volume: 400,
                    order_num: 4,
                }],
                bids: vec![longport::quote::Depth {
                    position: 1,
                    price: Some(dec!(100.50)),
                    volume: 500,
                    order_num: 5,
                }],
            },
        )
        .await;
        // mid_price = (100.50 + 100.60) / 2 = 100.55

        c.on_trade(
            "700.HK",
            longport::quote::PushTrades {
                trades: vec![
                    longport::quote::Trade {
                        price: dec!(100.55),
                        volume: 200,
                        timestamp: time::OffsetDateTime::from_unix_timestamp(1_700_000_000)
                            .unwrap(),
                        trade_type: "D".to_string(),
                        direction: longport::quote::TradeDirection::Up,
                        trade_session: longport::quote::TradeSession::Intraday,
                    },
                    longport::quote::Trade {
                        price: dec!(100.52),
                        volume: 100,
                        timestamp: time::OffsetDateTime::from_unix_timestamp(1_700_000_001)
                            .unwrap(),
                        trade_type: "".to_string(),
                        direction: longport::quote::TradeDirection::Down,
                        trade_session: longport::quote::TradeSession::Intraday,
                    },
                ],
            },
        )
        .await;

        let trades = ctx.published_trades.lock().unwrap().clone();
        assert_eq!(trades.len(), 2);
        assert_eq!(trades[0].price, dec!(100.55));
        assert_eq!(trades[0].size, dec!(200));
        assert_eq!(trades[0].direction, rushhft_core::TradeDirection::Up);
        assert_eq!(trades[0].trade_type, "D");
        assert_eq!(trades[0].market_mid_price, dec!(100.55));
        assert_eq!(trades[1].direction, rushhft_core::TradeDirection::Down);
        assert_eq!(trades[1].size, dec!(100));
    }

    #[tokio::test]
    async fn on_trade_with_no_local_book_uses_zero_mid_price() {
        use rust_decimal_macros::dec;
        let c = test_connector();
        let ctx = Arc::new(MockCtx::new());
        c.inner
            .ctx
            .lock()
            .await
            .replace(ctx.clone() as Arc<dyn PluginContext>);

        c.on_trade(
            "700.HK",
            longport::quote::PushTrades {
                trades: vec![longport::quote::Trade {
                    price: dec!(100.00),
                    volume: 50,
                    timestamp: time::OffsetDateTime::from_unix_timestamp(1_700_000_000)
                        .unwrap(),
                    trade_type: "".to_string(),
                    direction: longport::quote::TradeDirection::Neutral,
                    trade_session: longport::quote::TradeSession::Intraday,
                }],
            },
        )
        .await;

        let trades = ctx.published_trades.lock().unwrap().clone();
        assert_eq!(trades.len(), 1);
        assert_eq!(trades[0].market_mid_price, dec!(0));
    }

    #[tokio::test]
    async fn on_quote_stores_quote_stats() {
        use rust_decimal_macros::dec;
        let c = test_connector();
        let ctx = Arc::new(MockCtx::new());
        c.inner
            .ctx
            .lock()
            .await
            .replace(ctx.clone() as Arc<dyn PluginContext>);

        c.on_quote(
            "700.HK",
            longport::quote::PushQuote {
                last_done: dec!(350.00),
                open: dec!(345.00),
                high: dec!(352.00),
                low: dec!(344.00),
                timestamp: time::OffsetDateTime::from_unix_timestamp(1_700_000_000)
                    .unwrap(),
                volume: 1_000_000,
                turnover: dec!(350_000_000),
                trade_status: longport::quote::TradeStatus::Normal,
                trade_session: longport::quote::TradeSession::Intraday,
                current_volume: 5_000,
                current_turnover: dec!(1_750_000),
            },
        )
        .await;

        let stats = c.quote_stats("700.HK").unwrap();
        assert_eq!(stats.last_done, dec!(350.00));
        assert_eq!(stats.high, dec!(352.00));
        assert_eq!(stats.volume, 1_000_000);
        assert_eq!(stats.timestamp.unix_timestamp(), 1_700_000_000);
    }

    #[tokio::test]
    async fn plugin_metadata() {
        let c = test_connector();
        assert_eq!(c.name(), "LongPort Connector");
        assert_eq!(c.plugin_type(), rushhft_core::PluginType::MarketConnector);
        assert!(!c.plugin_id().is_empty());
    }

    #[tokio::test]
    async fn plugin_start_with_empty_credentials_returns_error() {
        let c = LongPortConnector::new(ConnectorSettings {
            app_key: String::new(),
            ..ConnectorSettings::default()
        });
        let ctx = Arc::new(MockCtx::new());
        let result =
            rushhft_core::plugin::Plugin::start(&c, ctx.clone() as Arc<dyn PluginContext>).await;
        assert!(result.is_err());
        assert_eq!(c.status(), rushhft_core::PluginStatus::StoppedFailed);
        // Provider DisconnectedFailed published
        let providers = ctx.published_providers.lock().unwrap().clone();
        assert_eq!(providers.len(), 1);
        assert_eq!(
            providers[0].status,
            rushhft_core::SessionStatus::DisconnectedFailed
        );
    }

    #[tokio::test]
    async fn plugin_start_when_already_started_returns_already_running() {
        let c = test_connector();
        c.inner
            .status
            .store(Arc::new(rushhft_core::PluginStatus::Started));
        let ctx = Arc::new(MockCtx::new());
        let result =
            rushhft_core::plugin::Plugin::start(&c, ctx.clone() as Arc<dyn PluginContext>).await;
        assert!(matches!(
            result,
            Err(rushhft_core::PluginError::AlreadyRunning(_))
        ));
    }
}
