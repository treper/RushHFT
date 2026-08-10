use rust_decimal::Decimal;
use time::OffsetDateTime;

#[derive(Debug, Clone, PartialEq)]
pub struct BookItem {
    pub price: Decimal,
    pub size: Decimal,
    pub cumulative_size: Decimal,
    pub is_bid: bool,
    pub broker_ids: Vec<i32>,
    pub entry_id: Option<String>,
    pub local_timestamp: OffsetDateTime,
    pub server_timestamp: OffsetDateTime,
    pub symbol: String,
    pub provider_id: i32,
}

impl BookItem {
    pub fn new(
        price: Decimal,
        size: Decimal,
        is_bid: bool,
        symbol: &str,
        provider_id: i32,
    ) -> Self {
        let now = OffsetDateTime::now_utc();
        Self {
            price,
            size,
            cumulative_size: size,
            is_bid,
            broker_ids: Vec::new(),
            entry_id: None,
            local_timestamp: now,
            server_timestamp: now,
            symbol: symbol.to_string(),
            provider_id,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rust_decimal_macros::dec;

    #[test]
    fn new_book_item_has_equal_size_and_cumulative() {
        let item = BookItem::new(dec!(100.50), dec!(500), true, "700.HK", 1);
        assert_eq!(item.size, dec!(500));
        assert_eq!(item.cumulative_size, dec!(500));
        assert!(item.is_bid);
        assert_eq!(item.symbol, "700.HK");
        assert!(item.broker_ids.is_empty());
        assert!(item.entry_id.is_none());
    }

    #[test]
    fn new_ask_item_is_not_bid() {
        let item = BookItem::new(dec!(100.52), dec!(300), false, "700.HK", 1);
        assert!(!item.is_bid);
    }
}
