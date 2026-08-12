//! Replay test: feed a scripted sequence of PushDepth / PushBrokers /
//! PushTrades / PushQuote through the connector and assert final state.
//!
//! No network — all payloads are hand-crafted.

use async_trait::async_trait;
use rushhft_connector_longport::{ConnectorSettings, LongPortConnector};
use rushhft_core::plugin::{Plugin, PluginContext};
use rushhft_core::{
    PluginStatus, PluginType, TradeDirection,
    hub::{OrderBookHub, ProviderHub, TradeHub},
    model::order_book::OrderBook,
    model::provider::Provider,
    model::trade::Trade,
};
use rust_decimal::Decimal;
use rust_decimal_macros::dec;
use std::sync::Arc;
use time::OffsetDateTime;

struct ReplayCtx {
    ob_hub: Arc<OrderBookHub>,
    t_hub: Arc<TradeHub>,
    p_hub: Arc<ProviderHub>,
    obs: Arc<dashmap::DashMap<String, OrderBook>>,
    trades: Arc<std::sync::Mutex<Vec<Trade>>>,
    providers: Arc<std::sync::Mutex<Vec<Provider>>>,
}

#[async_trait]
impl PluginContext for ReplayCtx {
    async fn publish_order_book(&self, ob: OrderBook) {
        self.obs.insert(ob.symbol.clone(), ob);
    }
    async fn publish_trade(&self, t: Trade) {
        self.trades.lock().unwrap().push(t);
    }
    async fn publish_provider(&self, p: Provider) {
        self.providers.lock().unwrap().push(p);
    }
    async fn register_metric(
        &self,
        _: &str,
        _: &str,
        _: &str,
        _: &str,
        _: Decimal,
        _: OffsetDateTime,
    ) {
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
    fn current_symbol(&self) -> String {
        "700.HK".to_string()
    }
}

#[tokio::test]
async fn replay_depth_brokers_trades_quote_sequence() {
    let connector = LongPortConnector::new(ConnectorSettings {
        app_key: "test_key".into(),
        app_secret: "test_secret".into(),
        access_token: "test_token".into(),
        symbols: vec!["700.HK".into()],
        depth_levels: 10,
        price_decimal_places: 2,
        size_decimal_places: 0,
        provider_id: 1,
        ..ConnectorSettings::default()
    });
    let ctx = Arc::new(ReplayCtx {
        ob_hub: Arc::new(OrderBookHub::new()),
        t_hub: Arc::new(TradeHub::new()),
        p_hub: Arc::new(ProviderHub::new()),
        obs: Arc::new(dashmap::DashMap::new()),
        trades: Arc::new(std::sync::Mutex::new(Vec::new())),
        providers: Arc::new(std::sync::Mutex::new(Vec::new())),
    });
    connector
        .set_context(ctx.clone() as Arc<dyn PluginContext>)
        .await;

    // 1. Depth push — initial ladder.
    connector
        .on_depth(
            "700.HK",
            longport::quote::PushDepth {
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
            },
        )
        .await;

    // 2. Brokers push — merge broker IDs at position 1.
    connector
        .on_brokers(
            "700.HK",
            longport::quote::PushBrokers {
                ask_brokers: vec![longport::quote::Brokers {
                    position: 1,
                    broker_ids: vec![1001, 1002],
                }],
                bid_brokers: vec![longport::quote::Brokers {
                    position: 1,
                    broker_ids: vec![2001, 2002],
                }],
            },
        )
        .await;

    // 3. Trade push — two trades.
    connector
        .on_trade(
            "700.HK",
            longport::quote::PushTrades {
                trades: vec![
                    longport::quote::Trade {
                        price: dec!(100.55),
                        volume: 200,
                        timestamp: OffsetDateTime::from_unix_timestamp(1_700_000_000).unwrap(),
                        trade_type: "D".to_string(),
                        direction: longport::quote::TradeDirection::Up,
                        trade_session: longport::quote::TradeSession::Intraday,
                    },
                    longport::quote::Trade {
                        price: dec!(100.52),
                        volume: 100,
                        timestamp: OffsetDateTime::from_unix_timestamp(1_700_000_001).unwrap(),
                        trade_type: "".to_string(),
                        direction: longport::quote::TradeDirection::Down,
                        trade_session: longport::quote::TradeSession::Intraday,
                    },
                ],
            },
        )
        .await;

    // 4. Quote push — OHLC.
    connector
        .on_quote(
            "700.HK",
            longport::quote::PushQuote {
                last_done: dec!(100.58),
                open: dec!(100.00),
                high: dec!(100.70),
                low: dec!(99.90),
                timestamp: OffsetDateTime::from_unix_timestamp(1_700_000_002).unwrap(),
                volume: 5_000_000,
                turnover: dec!(502_900_000),
                trade_status: longport::quote::TradeStatus::Normal,
                trade_session: longport::quote::TradeSession::Intraday,
                current_volume: 200,
                current_turnover: dec!(20_116),
            },
        )
        .await;

    // Assertions on final state.
    let book = connector.local_book("700.HK").unwrap();
    assert_eq!(book.bids.len(), 2);
    assert_eq!(book.asks.len(), 2);
    assert_eq!(book.bids[0].broker_ids, vec![2001, 2002]);
    assert_eq!(book.asks[0].broker_ids, vec![1001, 1002]);
    assert_eq!(book.mid_price().unwrap(), dec!(100.575));

    let stats = connector.quote_stats("700.HK").unwrap();
    assert_eq!(stats.last_done, dec!(100.58));
    assert_eq!(stats.high, dec!(100.70));

    let trades = ctx.trades.lock().unwrap().clone();
    assert_eq!(trades.len(), 2);
    assert_eq!(trades[0].direction, TradeDirection::Up);
    assert_eq!(trades[1].direction, TradeDirection::Down);
    assert_eq!(trades[0].market_mid_price, dec!(100.575));

    let providers = ctx.providers.lock().unwrap().clone();
    assert!(providers.is_empty()); // No start() called — no provider published.
}

#[tokio::test]
async fn connector_metadata_matches_spec() {
    let c = LongPortConnector::new(ConnectorSettings::default());
    assert_eq!(c.name(), "LongPort Connector");
    assert_eq!(c.plugin_type(), PluginType::MarketConnector);
    assert_eq!(c.status(), PluginStatus::Loaded);
    assert_eq!(c.author(), "RushHFT");
    assert_eq!(c.version(), "0.1.0");
    assert_eq!(
        c.description(),
        "LongPort OpenAPI connector (HK/US equities)"
    );
    assert!(!c.plugin_id().is_empty());
}
