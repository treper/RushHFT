//! Replay integration test: scripted trade + order book stream → both studies emit expected metrics.

use async_trait::async_trait;
use rushhft_core::model::book_item::BookItem;
use rushhft_core::model::enums::AggregationLevel;
use rushhft_core::model::enums::TradeDirection;
use rushhft_core::model::order_book::OrderBook;
use rushhft_core::model::provider::Provider;
use rushhft_core::model::trade::Trade;
use rushhft_core::{OrderBookHub, Plugin, PluginContext, ProviderHub, TradeHub};
use rushhft_studies::{LobImbalanceSettings, LobImbalanceStudy, VpinSettings, VpinStudy};
use rust_decimal::Decimal;
use rust_decimal_macros::dec;
use std::sync::Arc;
use time::OffsetDateTime;

struct ReplayCtx {
    ob_hub: Arc<OrderBookHub>,
    t_hub: Arc<TradeHub>,
    p_hub: Arc<ProviderHub>,
    metrics: Arc<std::sync::Mutex<Vec<MetricRecord>>>,
}

#[derive(Clone, Debug)]
#[allow(dead_code)]
struct MetricRecord {
    plugin: String,
    metric: String,
    symbol: String,
    value: Decimal,
}

#[async_trait]
impl PluginContext for ReplayCtx {
    async fn publish_order_book(&self, _ob: OrderBook) {}
    async fn publish_trade(&self, _t: Trade) {}
    async fn publish_provider(&self, _p: Provider) {}
    async fn register_metric(
        &self,
        plugin: &str,
        metric: &str,
        _exchange: &str,
        symbol: &str,
        value: Decimal,
        _ts: OffsetDateTime,
    ) {
        self.metrics.lock().unwrap().push(MetricRecord {
            plugin: plugin.into(),
            metric: metric.into(),
            symbol: symbol.into(),
            value,
        });
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
}

fn trade(price: Decimal, size: Decimal, dir: TradeDirection, secs: i64) -> Trade {
    Trade {
        price,
        size,
        timestamp: OffsetDateTime::from_unix_timestamp(secs).unwrap(),
        direction: dir,
        trade_type: "D".to_string(),
        symbol: "700.HK".to_string(),
        provider_id: 1,
        market_mid_price: dec!(100.575),
    }
}

fn book(bids: Vec<(Decimal, Decimal)>, asks: Vec<(Decimal, Decimal)>) -> OrderBook {
    let mut ob = OrderBook::new("700.HK", 10, 2, 0, 1);
    for (p, s) in bids {
        ob.add_or_update_level(BookItem::new(p, s, true, "700.HK", 1));
    }
    for (p, s) in asks {
        ob.add_or_update_level(BookItem::new(p, s, false, "700.HK", 1));
    }
    ob
}

#[tokio::test]
async fn replay_both_studies_emit_expected_metrics() {
    let ob_hub = Arc::new(OrderBookHub::new());
    let t_hub = Arc::new(TradeHub::new());
    let p_hub = Arc::new(ProviderHub::new());
    let metrics = Arc::new(std::sync::Mutex::new(Vec::new()));
    let ctx = Arc::new(ReplayCtx {
        ob_hub: ob_hub.clone(),
        t_hub: t_hub.clone(),
        p_hub: p_hub.clone(),
        metrics: metrics.clone(),
    }) as Arc<dyn PluginContext>;

    let vpin = Arc::new(VpinStudy::new(VpinSettings {
        bucket_volume_size: dec!(1),
        number_of_buckets: 50,
        symbol: "700.HK".into(),
        provider_id: 1,
        aggregation_level: AggregationLevel::S1,
    }));
    let lob = Arc::new(LobImbalanceStudy::new(LobImbalanceSettings {
        symbol: "700.HK".into(),
        provider_id: 1,
        levels: 5,
        aggregation_level: AggregationLevel::S1,
    }));
    vpin.start(ctx.clone()).await.unwrap();
    lob.start(ctx.clone()).await.unwrap();

    // 1. Book: all bids → imbalance = 1
    ob_hub.publish(book(vec![(dec!(100), dec!(100))], vec![]));
    // 2. Trade: 1-volume buy → bucket completes, vpin = 1
    t_hub.publish(trade(
        dec!(100.50),
        dec!(1),
        TradeDirection::Up,
        1_700_000_000,
    ));

    tokio::time::sleep(std::time::Duration::from_millis(80)).await;

    let collected = metrics.lock().unwrap().clone();

    // VPIN should have at least one record with value == 1
    assert!(
        collected
            .iter()
            .any(|m| m.plugin == "VPIN Study" && m.value == Decimal::ONE),
        "expected VPIN=1, got {:?}",
        collected
    );
    // LOB Imbalance should have at least one record with value == 1
    assert!(
        collected
            .iter()
            .any(|m| m.plugin == "LOB Imbalance Study" && m.value == Decimal::ONE),
        "expected LOB imbalance=1, got {:?}",
        collected
    );

    vpin.stop().await.unwrap();
    lob.stop().await.unwrap();
}

#[tokio::test]
async fn replay_lob_balanced_book_gives_imbalance_zero() {
    let ob_hub = Arc::new(OrderBookHub::new());
    let t_hub = Arc::new(TradeHub::new());
    let p_hub = Arc::new(ProviderHub::new());
    let metrics = Arc::new(std::sync::Mutex::new(Vec::new()));
    let ctx = Arc::new(ReplayCtx {
        ob_hub: ob_hub.clone(),
        t_hub: t_hub.clone(),
        p_hub: p_hub.clone(),
        metrics: metrics.clone(),
    }) as Arc<dyn PluginContext>;

    let lob = Arc::new(LobImbalanceStudy::new(LobImbalanceSettings {
        symbol: "700.HK".into(),
        provider_id: 1,
        levels: 5,
        aggregation_level: AggregationLevel::S1,
    }));
    lob.start(ctx).await.unwrap();

    // Balanced book: 100 bid vs 100 ask → imbalance = 0
    ob_hub.publish(book(
        vec![(dec!(100), dec!(100))],
        vec![(dec!(101), dec!(100))],
    ));

    tokio::time::sleep(std::time::Duration::from_millis(50)).await;
    let collected = metrics.lock().unwrap().clone();
    assert!(
        collected
            .iter()
            .any(|m| m.plugin == "LOB Imbalance Study" && m.value == Decimal::ZERO),
        "expected imbalance=0, got {:?}",
        collected
    );

    lob.stop().await.unwrap();
}
