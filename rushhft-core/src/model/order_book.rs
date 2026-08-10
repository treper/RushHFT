use crate::model::book_item::BookItem;
use rust_decimal::prelude::ToPrimitive;
use rust_decimal::Decimal;
use time::OffsetDateTime;

#[derive(Debug, Clone)]
pub struct OrderBook {
    pub symbol: String,
    pub bids: Vec<BookItem>,
    pub asks: Vec<BookItem>,
    pub max_depth: usize,
    pub price_decimal_places: u8,
    pub size_decimal_places: u8,
    pub provider_id: i32,
    pub sequence: i64,
    pub last_updated: OffsetDateTime,
    pub imbalance_value: Decimal,
    pub added_levels: u64,
    pub deleted_levels: u64,
    pub updated_levels: u64,
    pub added_volume_scaled: u64,
    pub deleted_volume_scaled: u64,
}

impl OrderBook {
    pub fn new(
        symbol: &str,
        max_depth: usize,
        price_decimal_places: u8,
        size_decimal_places: u8,
        provider_id: i32,
    ) -> Self {
        Self {
            symbol: symbol.to_string(),
            bids: Vec::new(),
            asks: Vec::new(),
            max_depth,
            price_decimal_places,
            size_decimal_places,
            provider_id,
            sequence: 0,
            last_updated: OffsetDateTime::now_utc(),
            imbalance_value: Decimal::ZERO,
            added_levels: 0,
            deleted_levels: 0,
            updated_levels: 0,
            added_volume_scaled: 0,
            deleted_volume_scaled: 0,
        }
    }

    pub fn compute_scale(&self) -> i64 {
        let mut scale: i64 = 1;
        for _ in 0..self.size_decimal_places {
            scale *= 10;
        }
        scale
    }

    pub fn add_or_update_level(&mut self, mut item: BookItem) {
        let scale = self.compute_scale();
        let side = if item.is_bid {
            &mut self.bids
        } else {
            &mut self.asks
        };

        let pos = side.iter().position(|l| l.price == item.price);

        match pos {
            Some(idx) => {
                let old_size = side[idx].size;
                side[idx].size = item.size;
                side[idx].server_timestamp = item.server_timestamp;
                side[idx].local_timestamp = item.local_timestamp;
                if !item.broker_ids.is_empty() {
                    let mut merged = std::mem::take(&mut side[idx].broker_ids);
                    for bid in item.broker_ids.drain(..) {
                        if !merged.contains(&bid) {
                            merged.push(bid);
                        }
                    }
                    side[idx].broker_ids = merged;
                }
                self.updated_levels += 1;
                let _ = old_size;
            }
            None => {
                if item.is_bid {
                    let pos = side.iter().position(|l| l.price < item.price);
                    match pos {
                        Some(idx) => side.insert(idx, item.clone()),
                        None => side.push(item.clone()),
                    }
                } else {
                    let pos = side.iter().position(|l| l.price > item.price);
                    match pos {
                        Some(idx) => side.insert(idx, item.clone()),
                        None => side.push(item.clone()),
                    }
                }
                self.added_levels += 1;
                let scaled = (item.size * Decimal::from(scale)).to_i64().unwrap_or(0);
                self.added_volume_scaled += scaled as u64;
            }
        }

        if side.len() > self.max_depth {
            side.truncate(self.max_depth);
        }

        self.compute_cumulative_sizes();
        self.calculate_metrics();
        self.sequence += 1;
        self.last_updated = OffsetDateTime::now_utc();
    }

    pub fn compute_cumulative_sizes(&mut self) {
        let mut cum = Decimal::ZERO;
        for level in &mut self.bids {
            cum += level.size;
            level.cumulative_size = cum;
        }
        cum = Decimal::ZERO;
        for level in &mut self.asks {
            cum += level.size;
            level.cumulative_size = cum;
        }
    }

    pub fn calculate_metrics(&mut self) {
        self.imbalance_value = self.compute_imbalance();
    }

    fn compute_imbalance(&self) -> Decimal {
        let bid_vol: Decimal = self.bids.iter().map(|l| l.size).sum();
        let ask_vol: Decimal = self.asks.iter().map(|l| l.size).sum();
        let total = bid_vol + ask_vol;
        if total.is_zero() {
            return Decimal::ZERO;
        }
        (bid_vol - ask_vol) / total
    }

    pub fn delete_level(&mut self, price: Decimal, is_bid: bool) {
        let scale = self.compute_scale();
        let side = if is_bid {
            &mut self.bids
        } else {
            &mut self.asks
        };

        if let Some(pos) = side.iter().position(|l| l.price == price) {
            let removed = side.remove(pos);
            self.deleted_levels += 1;
            let scaled = (removed.size * Decimal::from(scale))
                .to_i64()
                .unwrap_or(0);
            self.deleted_volume_scaled += scaled as u64;

            self.compute_cumulative_sizes();
            self.calculate_metrics();
            self.sequence += 1;
            self.last_updated = OffsetDateTime::now_utc();
        }
    }

    pub fn mid_price(&self) -> Option<Decimal> {
        match (self.bids.first(), self.asks.first()) {
            (Some(bid), Some(ask)) => Some((bid.price + ask.price) / Decimal::from(2)),
            _ => None,
        }
    }

    pub fn spread(&self) -> Option<Decimal> {
        match (self.bids.first(), self.asks.first()) {
            (Some(bid), Some(ask)) => Some(ask.price - bid.price),
            _ => None,
        }
    }

    pub fn get_bids_snapshot(&self) -> &[BookItem] {
        &self.bids
    }

    pub fn get_asks_snapshot(&self) -> &[BookItem] {
        &self.asks
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rust_decimal_macros::dec;

    #[test]
    fn new_order_book_is_empty() {
        let ob = OrderBook::new("700.HK", 10, 2, 0, 1);
        assert_eq!(ob.symbol, "700.HK");
        assert!(ob.bids.is_empty());
        assert!(ob.asks.is_empty());
        assert_eq!(ob.max_depth, 10);
        assert_eq!(ob.sequence, 0);
        assert_eq!(ob.imbalance_value, Decimal::ZERO);
        assert_eq!(ob.added_levels, 0);
    }

    #[test]
    fn compute_scale_with_zero_decimal_places() {
        let ob = OrderBook::new("700.HK", 10, 2, 0, 1);
        assert_eq!(ob.compute_scale(), 1);
    }

    #[test]
    fn compute_scale_with_two_decimal_places() {
        let ob = OrderBook::new("700.HK", 10, 2, 2, 1);
        assert_eq!(ob.compute_scale(), 100);
    }

    #[test]
    fn add_bid_level_inserts_in_descending_order() {
        let mut ob = OrderBook::new("700.HK", 10, 2, 0, 1);
        ob.add_or_update_level(BookItem::new(dec!(100.50), dec!(500), true, "700.HK", 1));
        ob.add_or_update_level(BookItem::new(dec!(100.55), dec!(300), true, "700.HK", 1));
        ob.add_or_update_level(BookItem::new(dec!(100.45), dec!(200), true, "700.HK", 1));

        assert_eq!(ob.bids.len(), 3);
        assert_eq!(ob.bids[0].price, dec!(100.55));
        assert_eq!(ob.bids[1].price, dec!(100.50));
        assert_eq!(ob.bids[2].price, dec!(100.45));
        assert_eq!(ob.added_levels, 3);
    }

    #[test]
    fn add_ask_level_inserts_in_ascending_order() {
        let mut ob = OrderBook::new("700.HK", 10, 2, 0, 1);
        ob.add_or_update_level(BookItem::new(dec!(100.60), dec!(400), false, "700.HK", 1));
        ob.add_or_update_level(BookItem::new(dec!(100.55), dec!(200), false, "700.HK", 1));

        assert_eq!(ob.asks.len(), 2);
        assert_eq!(ob.asks[0].price, dec!(100.55));
        assert_eq!(ob.asks[1].price, dec!(100.60));
    }

    #[test]
    fn update_existing_bid_level_changes_size() {
        let mut ob = OrderBook::new("700.HK", 10, 2, 0, 1);
        ob.add_or_update_level(BookItem::new(dec!(100.50), dec!(500), true, "700.HK", 1));
        ob.add_or_update_level(BookItem::new(dec!(100.50), dec!(800), true, "700.HK", 1));

        assert_eq!(ob.bids.len(), 1);
        assert_eq!(ob.bids[0].size, dec!(800));
        assert_eq!(ob.added_levels, 1);
        assert_eq!(ob.updated_levels, 1);
    }

    #[test]
    fn add_level_beyond_max_depth_is_dropped() {
        let mut ob = OrderBook::new("700.HK", 2, 2, 0, 1);
        ob.add_or_update_level(BookItem::new(dec!(100.50), dec!(100), true, "700.HK", 1));
        ob.add_or_update_level(BookItem::new(dec!(100.45), dec!(200), true, "700.HK", 1));
        ob.add_or_update_level(BookItem::new(dec!(100.40), dec!(300), true, "700.HK", 1));

        assert_eq!(ob.bids.len(), 2);
        assert_eq!(ob.bids[0].price, dec!(100.50));
        assert_eq!(ob.bids[1].price, dec!(100.45));
    }

    #[test]
    fn delete_bid_level_removes_it() {
        let mut ob = OrderBook::new("700.HK", 10, 2, 0, 1);
        ob.add_or_update_level(BookItem::new(dec!(100.50), dec!(500), true, "700.HK", 1));
        ob.add_or_update_level(BookItem::new(dec!(100.45), dec!(200), true, "700.HK", 1));

        ob.delete_level(dec!(100.45), true);

        assert_eq!(ob.bids.len(), 1);
        assert_eq!(ob.bids[0].price, dec!(100.50));
        assert_eq!(ob.deleted_levels, 1);
    }

    #[test]
    fn delete_nonexistent_level_is_noop() {
        let mut ob = OrderBook::new("700.HK", 10, 2, 0, 1);
        ob.add_or_update_level(BookItem::new(dec!(100.50), dec!(500), true, "700.HK", 1));

        ob.delete_level(dec!(999.00), true);

        assert_eq!(ob.bids.len(), 1);
        assert_eq!(ob.deleted_levels, 0);
    }

    #[test]
    fn mid_price_and_spread() {
        let mut ob = OrderBook::new("700.HK", 10, 2, 0, 1);
        ob.add_or_update_level(BookItem::new(dec!(100.50), dec!(500), true, "700.HK", 1));
        ob.add_or_update_level(BookItem::new(dec!(100.60), dec!(300), false, "700.HK", 1));

        assert_eq!(ob.mid_price().unwrap(), dec!(100.55));
        assert_eq!(ob.spread().unwrap(), dec!(0.10));
    }

    #[test]
    fn mid_price_none_when_book_empty() {
        let ob = OrderBook::new("700.HK", 10, 2, 0, 1);
        assert!(ob.mid_price().is_none());
        assert!(ob.spread().is_none());
    }

    #[test]
    fn cumulative_sizes_correct() {
        let mut ob = OrderBook::new("700.HK", 10, 2, 0, 1);
        ob.add_or_update_level(BookItem::new(dec!(100.50), dec!(500), true, "700.HK", 1));
        ob.add_or_update_level(BookItem::new(dec!(100.45), dec!(200), true, "700.HK", 1));

        assert_eq!(ob.bids[0].cumulative_size, dec!(500));
        assert_eq!(ob.bids[1].cumulative_size, dec!(700));
    }

    #[test]
    fn get_bids_snapshot_returns_slice() {
        let mut ob = OrderBook::new("700.HK", 10, 2, 0, 1);
        ob.add_or_update_level(BookItem::new(dec!(100.50), dec!(500), true, "700.HK", 1));

        let snap = ob.get_bids_snapshot();
        assert_eq!(snap.len(), 1);
        assert_eq!(snap[0].price, dec!(100.50));
    }
}
