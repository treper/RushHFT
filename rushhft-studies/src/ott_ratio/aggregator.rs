//! Pure OTT computation. L2 formula (LongPort provides price-level data):
//!   OTR = (AddedΔ + 2×UpdatedΔ + DeletedΔ) / max(Trades, 1) − 1
//! Port of VisualHFT `OrderToTradeRatioStudy.cs:198-235` (L2 branch).

use rushhft_core::model::order_book::OrderBook;
use rust_decimal::Decimal;

#[derive(Debug, Clone)]
pub struct OttCounters {
    pub prev_added: u64,
    pub prev_deleted: u64,
    pub prev_updated: u64,
    pub is_first_call: bool,
    pub order_events: u64,
    pub trade_count: u64,
}

impl Default for OttCounters {
    fn default() -> Self {
        Self {
            prev_added: 0,
            prev_deleted: 0,
            prev_updated: 0,
            is_first_call: true,
            order_events: 0,
            trade_count: 0,
        }
    }
}

impl OttCounters {
    // Reserved for future plugin-restart flow; not called in current MVP.
    #[allow(dead_code)]
    pub fn reset(&mut self) {
        self.order_events = 0;
        self.trade_count = 0;
    }
}

/// Compute the OTT ratio from the current OrderBook's cumulative counters
/// and the current trade count. Returns 0 on the first call (initialization).
pub fn compute_ott(c: &mut OttCounters, ob: &OrderBook, trade_count_delta: u64) -> Decimal {
    let added = ob.added_levels;
    let deleted = ob.deleted_levels;
    let updated = ob.updated_levels;

    if c.is_first_call {
        c.prev_added = added;
        c.prev_deleted = deleted;
        c.prev_updated = updated;
        c.is_first_call = false;
        return Decimal::ZERO;
    }

    let added_d = added.saturating_sub(c.prev_added);
    let deleted_d = deleted.saturating_sub(c.prev_deleted);
    let updated_d = updated.saturating_sub(c.prev_updated);

    c.prev_added = added;
    c.prev_deleted = deleted;
    c.prev_updated = updated;

    c.order_events += added_d + deleted_d + 2 * updated_d;
    c.trade_count += trade_count_delta;

    let denom = c.trade_count.max(1) as f64;
    let order_events = c.order_events as f64;
    let ratio = order_events / denom - 1.0;
    Decimal::from_f64_retain(ratio).unwrap_or(Decimal::ZERO)
}

#[cfg(test)]
mod tests {
    use super::*;
    use rust_decimal_macros::dec;

    fn make_book() -> OrderBook {
        OrderBook::new("700.HK", 10, 2, 0, 1)
    }

    #[test]
    fn first_call_returns_zero_and_seeds_counters() {
        let mut c = OttCounters::default();
        let mut ob = make_book();
        ob.added_levels = 10;
        let v = compute_ott(&mut c, &ob, 0);
        assert_eq!(v, Decimal::ZERO);
        assert_eq!(c.prev_added, 10);
        assert!(c.is_first_call == false);
    }

    #[test]
    fn zero_trades_uses_floor_of_one() {
        let mut c = OttCounters::default();
        let mut ob = make_book();
        // first call seeds prev_added=0
        let _ = compute_ott(&mut c, &ob, 0);
        // second call: 5 added, 0 trades -> order_events=5, denom=1, ratio=4
        ob.added_levels = 5;
        let v = compute_ott(&mut c, &ob, 0);
        assert_eq!(v, dec!(4)); // 5/1 - 1
    }

    #[test]
    fn formula_matches_l2_definition() {
        let mut c = OttCounters::default();
        let mut ob = make_book();
        let _ = compute_ott(&mut c, &ob, 0); // seed
        // 3 added, 2 updated, 1 deleted; 2 trades
        ob.added_levels = 3;
        ob.updated_levels = 2;
        ob.deleted_levels = 1;
        let v = compute_ott(&mut c, &ob, 2);
        // order_events = 3 + 1 + 2*2 = 8; denom = 2; ratio = 8/2 - 1 = 3
        assert_eq!(v, dec!(3));
    }

    #[test]
    fn reset_zeroes_accumulators_but_not_prevs() {
        let mut c = OttCounters::default();
        c.order_events = 10;
        c.trade_count = 5;
        c.reset();
        assert_eq!(c.order_events, 0);
        assert_eq!(c.trade_count, 0);
    }
}
