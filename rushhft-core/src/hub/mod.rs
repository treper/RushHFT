use crate::model::order_book::OrderBook;
use crate::model::provider::Provider;
use crate::model::trade::Trade;
use arc_swap::ArcSwap;
use dashmap::DashMap;
use std::panic::{catch_unwind, AssertUnwindSafe};
use std::sync::Arc;

type Subscriber<T> = Arc<dyn Fn(&T) + Send + Sync>;

pub struct SubscriptionGuard {
    unsubscribe: Option<Box<dyn FnOnce() + Send + Sync>>,
}

impl SubscriptionGuard {
    fn new(unsubscribe: Box<dyn FnOnce() + Send + Sync>) -> Self {
        Self {
            unsubscribe: Some(unsubscribe),
        }
    }
}

impl Drop for SubscriptionGuard {
    fn drop(&mut self) {
        if let Some(f) = self.unsubscribe.take() {
            f();
        }
    }
}

// --- OrderBookHub ---

pub struct OrderBookHub {
    subscribers: Arc<ArcSwap<Vec<Subscriber<OrderBook>>>>,
    latest: DashMap<String, ArcSwap<OrderBook>>,
}

impl OrderBookHub {
    pub fn new() -> Self {
        Self {
            subscribers: Arc::new(ArcSwap::from_pointee(Vec::new())),
            latest: DashMap::new(),
        }
    }

    pub fn subscribe(&self, f: Subscriber<OrderBook>) -> SubscriptionGuard {
        self.subscribers.rcu(|current| {
            let mut new_list = (**current).clone();
            new_list.push(f.clone());
            Arc::new(new_list)
        });
        let subs = self.subscribers.clone();
        SubscriptionGuard::new(Box::new(move || {
            subs.rcu(|current| {
                let mut new_list = (**current).clone();
                new_list.retain(|s| !Arc::ptr_eq(s, &f));
                Arc::new(new_list)
            });
        }))
    }

    pub fn publish(&self, ob: OrderBook) {
        let arc = Arc::new(ob.clone());
        let symbol = arc.symbol.clone();

        self.latest
            .entry(symbol.clone())
            .or_insert_with(|| ArcSwap::from_pointee(ob.clone()));
        if let Some(entry) = self.latest.get(&symbol) {
            entry.store(arc.clone());
        }

        let subs = self.subscribers.load();
        for sub in subs.iter() {
            let sub = sub.clone();
            let arc = arc.clone();
            let _ = catch_unwind(AssertUnwindSafe(move || sub(&arc)));
        }
    }

    pub fn snapshot(&self, symbol: &str) -> Option<Arc<OrderBook>> {
        self.latest.get(symbol).map(|e| e.load_full())
    }

    pub fn symbols(&self) -> Vec<String> {
        self.latest.iter().map(|e| e.key().clone()).collect()
    }
}

impl Default for OrderBookHub {
    fn default() -> Self {
        Self::new()
    }
}

// --- TradeHub ---

pub struct TradeHub {
    subscribers: Arc<ArcSwap<Vec<Subscriber<Trade>>>>,
    latest: DashMap<String, Vec<Trade>>,
}

impl TradeHub {
    pub fn new() -> Self {
        Self {
            subscribers: Arc::new(ArcSwap::from_pointee(Vec::new())),
            latest: DashMap::new(),
        }
    }

    pub fn subscribe(&self, f: Subscriber<Trade>) -> SubscriptionGuard {
        self.subscribers.rcu(|current| {
            let mut new_list = (**current).clone();
            new_list.push(f.clone());
            Arc::new(new_list)
        });
        let subs = self.subscribers.clone();
        SubscriptionGuard::new(Box::new(move || {
            subs.rcu(|current| {
                let mut new_list = (**current).clone();
                new_list.retain(|s| !Arc::ptr_eq(s, &f));
                Arc::new(new_list)
            });
        }))
    }

    pub fn publish(&self, t: Trade) {
        let symbol = t.symbol.clone();
        self.latest
            .entry(symbol.clone())
            .or_insert_with(Vec::new)
            .push(t.clone());

        if let Some(mut entry) = self.latest.get_mut(&symbol) {
            if entry.len() > 200 {
                let drain_from = entry.len() - 200;
                entry.drain(0..drain_from);
            }
        }

        let subs = self.subscribers.load();
        for sub in subs.iter() {
            let sub = sub.clone();
            let t = t.clone();
            let _ = catch_unwind(AssertUnwindSafe(move || sub(&t)));
        }
    }

    pub fn recent_trades(&self, symbol: &str) -> Vec<Trade> {
        self.latest
            .get(symbol)
            .map(|e| e.clone())
            .unwrap_or_default()
    }
}

impl Default for TradeHub {
    fn default() -> Self {
        Self::new()
    }
}

// --- ProviderHub ---

pub struct ProviderHub {
    subscribers: Arc<ArcSwap<Vec<Subscriber<Provider>>>>,
    latest: ArcSwap<Vec<Provider>>,
}

impl ProviderHub {
    pub fn new() -> Self {
        Self {
            subscribers: Arc::new(ArcSwap::from_pointee(Vec::new())),
            latest: ArcSwap::from_pointee(Vec::new()),
        }
    }

    pub fn subscribe(&self, f: Subscriber<Provider>) -> SubscriptionGuard {
        self.subscribers.rcu(|current| {
            let mut new_list = (**current).clone();
            new_list.push(f.clone());
            Arc::new(new_list)
        });
        let subs = self.subscribers.clone();
        SubscriptionGuard::new(Box::new(move || {
            subs.rcu(|current| {
                let mut new_list = (**current).clone();
                new_list.retain(|s| !Arc::ptr_eq(s, &f));
                Arc::new(new_list)
            });
        }))
    }

    pub fn publish(&self, p: Provider) {
        let current = self.latest.load();
        let mut new_list: Vec<Provider> =
            current.iter().filter(|x| x.id != p.id).cloned().collect();
        new_list.push(p.clone());
        self.latest.store(Arc::new(new_list));

        let subs = self.subscribers.load();
        for sub in subs.iter() {
            let sub = sub.clone();
            let p = p.clone();
            let _ = catch_unwind(AssertUnwindSafe(move || sub(&p)));
        }
    }

    pub fn providers(&self) -> Vec<Provider> {
        (**self.latest.load()).clone()
    }
}

impl Default for ProviderHub {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::book_item::BookItem;
    use crate::model::enums::TradeDirection;
    use rust_decimal_macros::dec;
    use std::sync::atomic::{AtomicU32, Ordering};
    use time::OffsetDateTime;

    #[test]
    fn order_book_hub_subscribe_and_publish() {
        let hub = OrderBookHub::new();
        let call_count = Arc::new(AtomicU32::new(0));
        let cc = call_count.clone();
        let _guard = hub.subscribe(Arc::new(move |_ob| {
            cc.fetch_add(1, Ordering::Relaxed);
        }));

        let mut ob = OrderBook::new("700.HK", 10, 2, 0, 1);
        ob.add_or_update_level(BookItem::new(dec!(100.50), dec!(500), true, "700.HK", 1));
        hub.publish(ob);

        assert_eq!(call_count.load(Ordering::Relaxed), 1);
    }

    #[test]
    fn subscription_guard_unsubscribes_on_drop() {
        let hub = OrderBookHub::new();
        let call_count = Arc::new(AtomicU32::new(0));
        let cc = call_count.clone();
        {
            let _guard = hub.subscribe(Arc::new(move |_| {
                cc.fetch_add(1, Ordering::Relaxed);
            }));
        }

        let ob = OrderBook::new("TEST.HK", 10, 2, 0, 1);
        hub.publish(ob);

        assert_eq!(call_count.load(Ordering::Relaxed), 0);
    }

    #[test]
    fn order_book_hub_snapshot_returns_latest() {
        let hub = OrderBookHub::new();
        let mut ob = OrderBook::new("700.HK", 10, 2, 0, 1);
        ob.add_or_update_level(BookItem::new(dec!(100.50), dec!(500), true, "700.HK", 1));
        hub.publish(ob);

        let snap = hub.snapshot("700.HK").unwrap();
        assert_eq!(snap.symbol, "700.HK");
        assert_eq!(snap.bids.len(), 1);
    }

    #[test]
    fn panicking_subscriber_does_not_break_fanout() {
        let hub = OrderBookHub::new();
        let call_count = Arc::new(AtomicU32::new(0));
        let cc = call_count.clone();

        let _g1 = hub.subscribe(Arc::new(move |_| {
            panic!("boom");
        }));
        let _g2 = hub.subscribe(Arc::new(move |_| {
            cc.fetch_add(1, Ordering::Relaxed);
        }));

        let ob = OrderBook::new("TEST.HK", 10, 2, 0, 1);
        hub.publish(ob);

        assert_eq!(call_count.load(Ordering::Relaxed), 1);
    }

    #[test]
    fn trade_hub_publish_and_recent_trades() {
        let hub = TradeHub::new();
        let t1 = Trade {
            price: dec!(100.00),
            size: dec!(50),
            timestamp: OffsetDateTime::now_utc(),
            direction: TradeDirection::Up,
            trade_type: "D".to_string(),
            symbol: "700.HK".to_string(),
            provider_id: 1,
            market_mid_price: dec!(99.95),
        };
        hub.publish(t1.clone());

        let trades = hub.recent_trades("700.HK");
        assert_eq!(trades.len(), 1);
        assert_eq!(trades[0].price, dec!(100.00));
    }

    #[test]
    fn provider_hub_publish_and_list() {
        let hub = ProviderHub::new();
        hub.publish(Provider {
            id: 1,
            name: "LongPort".to_string(),
            status: crate::model::enums::SessionStatus::Connected,
        });

        let providers = hub.providers();
        assert_eq!(providers.len(), 1);
        assert_eq!(providers[0].name, "LongPort");
    }
}
