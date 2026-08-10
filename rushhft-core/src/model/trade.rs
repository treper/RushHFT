use crate::model::enums::TradeDirection;
use rust_decimal::Decimal;
use time::OffsetDateTime;

#[derive(Debug, Clone, PartialEq)]
pub struct Trade {
    pub price: Decimal,
    pub size: Decimal,
    pub timestamp: OffsetDateTime,
    pub direction: TradeDirection,
    pub trade_type: String,
    pub symbol: String,
    pub provider_id: i32,
    pub market_mid_price: Decimal,
}

#[cfg(test)]
mod tests {
    use super::*;
    use rust_decimal_macros::dec;

    #[test]
    fn trade_construction() {
        let t = Trade {
            price: dec!(350.00),
            size: dec!(100),
            timestamp: OffsetDateTime::now_utc(),
            direction: TradeDirection::Up,
            trade_type: "D".to_string(),
            symbol: "700.HK".to_string(),
            provider_id: 1,
            market_mid_price: dec!(349.90),
        };
        assert_eq!(t.price, dec!(350.00));
        assert_eq!(t.direction, TradeDirection::Up);
        assert_eq!(t.trade_type, "D");
    }
}
