use rust_decimal::Decimal;
use time::OffsetDateTime;

#[derive(Debug, Clone, PartialEq)]
pub struct BaseStudyModel {
    pub value: Decimal,
    pub format: String,
    pub timestamp: OffsetDateTime,
    pub market_mid_price: Decimal,
    pub value_color: String,
    pub tooltip: String,
    pub has_error: bool,
    pub is_stale: bool,
}

impl BaseStudyModel {
    pub fn new(value: Decimal, format: &str) -> Self {
        Self {
            value,
            format: format.to_string(),
            timestamp: OffsetDateTime::now_utc(),
            market_mid_price: Decimal::ZERO,
            value_color: String::new(),
            tooltip: String::new(),
            has_error: false,
            is_stale: false,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rust_decimal_macros::dec;

    #[test]
    fn study_model_new_defaults() {
        let m = BaseStudyModel::new(dec!(0.5), "0.0000");
        assert_eq!(m.value, dec!(0.5));
        assert_eq!(m.format, "0.0000");
        assert!(!m.has_error);
        assert!(!m.is_stale);
    }
}
