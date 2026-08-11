//! Concrete PluginContext that wires the connector + studies to hubs + SnapshotStore.
#![allow(dead_code)]

use crate::dto::{
    BookItemDto, ProviderDto, SessionStatusDto, StudyValueDto, TradeDirectionDto, TradeDto,
};
use crate::state::SnapshotStore;
use rushhft_core::model::book_item::BookItem;
use rushhft_core::model::enums::{SessionStatus, TradeDirection as CoreTradeDirection};
use rushhft_core::model::order_book::OrderBook;
use rushhft_core::model::provider::Provider;
use rushhft_core::model::trade::Trade;
use rushhft_core::plugin::PluginContext;
use rushhft_core::{MetricEvent, OrderBookHub, ProviderHub, TradeHub};
use rust_decimal::Decimal;
use std::sync::Arc;
use time::OffsetDateTime;

pub struct PluginContextImpl {
    ob_hub: Arc<OrderBookHub>,
    t_hub: Arc<TradeHub>,
    p_hub: Arc<ProviderHub>,
    snapshot_store: Arc<SnapshotStore>,
    // TriggerEngine handle — we hold a Sender for register_metric
    metric_tx: tokio::sync::mpsc::UnboundedSender<MetricEvent>,
}

impl PluginContextImpl {
    pub fn new(
        ob_hub: Arc<OrderBookHub>,
        t_hub: Arc<TradeHub>,
        p_hub: Arc<ProviderHub>,
        snapshot_store: Arc<SnapshotStore>,
        metric_tx: tokio::sync::mpsc::UnboundedSender<MetricEvent>,
    ) -> Self {
        Self {
            ob_hub,
            t_hub,
            p_hub,
            snapshot_store,
            metric_tx,
        }
    }
}

#[async_trait::async_trait]
impl PluginContext for PluginContextImpl {
    async fn publish_order_book(&self, ob: OrderBook) {
        // Fan out to studies via the hub...
        self.ob_hub.publish(ob.clone());

        // ...and update the SnapshotStore.
        let symbol = ob.symbol.clone();
        self.snapshot_store.update_book(&symbol, |snap| {
            snap.symbol = ob.symbol.clone();
            snap.bids = ob.bids.iter().map(map_book_item).collect();
            snap.asks = ob.asks.iter().map(map_book_item).collect();
            snap.spread = ob.spread().unwrap_or(Decimal::ZERO);
            snap.mid_price = ob.mid_price().unwrap_or(Decimal::ZERO);
            snap.last_updated = (ob.last_updated.unix_timestamp_nanos() / 1_000_000) as i64;
            snap.sequence = ob.sequence;
            snap.provider_status = SessionStatusDto::Connected;
        });
    }

    async fn publish_trade(&self, t: Trade) {
        self.t_hub.publish(t.clone());

        let symbol = t.symbol.clone();
        let dto = TradeDto {
            price: t.price,
            size: t.size,
            timestamp: (t.timestamp.unix_timestamp_nanos() / 1_000_000) as i64,
            direction: map_trade_direction(t.direction),
            trade_type: t.trade_type,
        };
        self.snapshot_store.append_trade(&symbol, dto);
    }

    async fn publish_provider(&self, p: Provider) {
        self.p_hub.publish(p.clone());

        let dto = ProviderDto {
            id: p.id,
            name: p.name,
            status: map_session_status(p.status),
        };
        let mut current = self.snapshot_store.providers();
        if let Some(existing) = current.iter_mut().find(|x| x.id == dto.id) {
            *existing = dto.clone();
        } else {
            current.push(dto);
        }
        self.snapshot_store.set_providers(current);
    }

    async fn register_metric(
        &self,
        plugin: &str,
        metric: &str,
        exchange: &str,
        symbol: &str,
        value: Decimal,
        ts: OffsetDateTime,
    ) {
        let event = MetricEvent {
            plugin: plugin.to_string(),
            metric: metric.to_string(),
            exchange: exchange.to_string(),
            symbol: symbol.to_string(),
            value,
            timestamp: ts,
            is_replay: false,
        };
        let _ = self.metric_tx.send(event);

        // Also surface as a StudyValueDto so the frontend sees the latest value
        // under the per-symbol snapshot.
        let study_dto = StudyValueDto {
            name: metric.to_string(),
            value,
            format: "N2".into(),
            value_color: "White".into(),
            tooltip: String::new(),
            has_error: false,
            is_stale: false,
            timestamp: (ts.unix_timestamp_nanos() / 1_000_000) as i64,
        };
        self.snapshot_store.update_study(symbol, plugin, study_dto);
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

fn map_book_item(b: &BookItem) -> BookItemDto {
    BookItemDto {
        price: b.price,
        size: b.size,
        cumulative_size: b.cumulative_size,
        is_bid: b.is_bid,
        broker_ids: b.broker_ids.clone(),
    }
}

fn map_trade_direction(d: CoreTradeDirection) -> TradeDirectionDto {
    match d {
        CoreTradeDirection::Neutral => TradeDirectionDto::Neutral,
        CoreTradeDirection::Down => TradeDirectionDto::Down,
        CoreTradeDirection::Up => TradeDirectionDto::Up,
    }
}

fn map_session_status(s: SessionStatus) -> SessionStatusDto {
    match s {
        SessionStatus::Connecting => SessionStatusDto::Connecting,
        SessionStatus::Connected => SessionStatusDto::Connected,
        SessionStatus::ConnectedWithWarnings => SessionStatusDto::ConnectedWithWarnings,
        SessionStatus::DisconnectedFailed => SessionStatusDto::DisconnectedFailed,
        SessionStatus::Disconnected => SessionStatusDto::Disconnected,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rust_decimal_macros::dec;
    use time::OffsetDateTime;

    fn make_ctx() -> (Arc<PluginContextImpl>, Arc<SnapshotStore>) {
        let ob_hub = Arc::new(OrderBookHub::new());
        let t_hub = Arc::new(TradeHub::new());
        let p_hub = Arc::new(ProviderHub::new());
        let store = Arc::new(SnapshotStore::new());
        let (tx, _rx) = tokio::sync::mpsc::unbounded_channel::<MetricEvent>();
        let ctx = Arc::new(PluginContextImpl::new(
            ob_hub,
            t_hub,
            p_hub,
            store.clone(),
            tx,
        ));
        (ctx, store)
    }

    #[tokio::test]
    async fn publish_order_book_stores_snapshot() {
        let (ctx, store) = make_ctx();
        let mut ob = OrderBook::new("700.HK", 10, 2, 0, 1);
        ob.add_or_update_level(BookItem::new(dec!(100.50), dec!(500), true, "700.HK", 1));
        ob.add_or_update_level(BookItem::new(dec!(100.60), dec!(300), false, "700.HK", 1));
        ctx.publish_order_book(ob).await;

        let snap = store.snapshot("700.HK").unwrap();
        assert_eq!(snap.bids.len(), 1);
        assert_eq!(snap.asks.len(), 1);
        assert_eq!(snap.mid_price, dec!(100.55));
    }

    #[tokio::test]
    async fn publish_trade_appends_to_store() {
        let (ctx, store) = make_ctx();
        // Need a book entry so snapshot() returns Some.
        let ob = OrderBook::new("700.HK", 10, 2, 0, 1);
        ctx.publish_order_book(ob).await;

        let t = Trade {
            price: dec!(100.55),
            size: dec!(200),
            timestamp: OffsetDateTime::from_unix_timestamp(1_700_000_000).unwrap(),
            direction: CoreTradeDirection::Up,
            trade_type: "D".to_string(),
            symbol: "700.HK".to_string(),
            provider_id: 1,
            market_mid_price: dec!(100.575),
        };
        ctx.publish_trade(t).await;
        let snap = store.snapshot("700.HK").unwrap();
        assert_eq!(snap.recent_trades.len(), 1);
        assert_eq!(snap.recent_trades[0].size, dec!(200));
    }

    #[tokio::test]
    async fn publish_provider_updates_store() {
        let (ctx, _store) = make_ctx();
        ctx.publish_provider(Provider {
            id: 1,
            name: "LongPort".into(),
            status: SessionStatus::Connected,
        })
        .await;
        let store = Arc::new(SnapshotStore::new());
        // Use a fresh store to assert — but the ctx holds its own. Recreate:
        let _ = store;
        // Re-check via the ctx's store clone: the simplest check is via the
        // publish path — but we don't expose store publicly. So re-run with a
        // known state using a fresh ctx.
        let (ctx2, store2) = make_ctx();
        ctx2.publish_provider(Provider {
            id: 1,
            name: "LongPort".into(),
            status: SessionStatus::Connected,
        })
        .await;
        let ps = store2.providers();
        assert_eq!(ps.len(), 1);
        assert_eq!(ps[0].status, SessionStatusDto::Connected);
        // suppress unused warning from the first ctx
        let _ = ctx;
    }

    #[tokio::test]
    async fn register_metric_updates_studies_map() {
        let (ctx, store) = make_ctx();
        // first create a book entry so snapshot() works
        let ob = OrderBook::new("700.HK", 10, 2, 0, 1);
        ctx.publish_order_book(ob).await;

        ctx.register_metric(
            "VPIN Study",
            "VPIN",
            "LongPort",
            "700.HK",
            dec!(0.5),
            OffsetDateTime::from_unix_timestamp(1_700_000_000).unwrap(),
        )
        .await;

        let snap = store.snapshot("700.HK").unwrap();
        assert_eq!(snap.studies.len(), 1);
        assert_eq!(snap.studies[0].value, dec!(0.5));
        assert_eq!(snap.studies[0].name, "VPIN");
    }
}
