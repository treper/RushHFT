use rust_decimal::Decimal;

pub struct RollingWindow {
    buffer: Vec<Decimal>,
    index: usize,
    count: usize,
    capacity: usize,
    sum: Decimal,
}

impl RollingWindow {
    pub fn new(capacity: usize) -> Self {
        Self {
            buffer: vec![Decimal::ZERO; capacity],
            index: 0,
            count: 0,
            capacity,
            sum: Decimal::ZERO,
        }
    }

    pub fn push(&mut self, value: Decimal) {
        if self.count == self.capacity {
            self.sum -= self.buffer[self.index];
        } else {
            self.count += 1;
        }
        self.buffer[self.index] = value;
        self.sum += value;
        self.index = (self.index + 1) % self.capacity;
    }

    pub fn average(&self) -> Decimal {
        if self.count == 0 {
            return Decimal::ZERO;
        }
        self.sum / Decimal::from(self.count)
    }

    pub fn sum(&self) -> Decimal {
        self.sum
    }

    pub fn count(&self) -> usize {
        self.count
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rust_decimal_macros::dec;

    #[test]
    fn empty_window_has_zero_average() {
        let rw = RollingWindow::new(3);
        assert_eq!(rw.average(), Decimal::ZERO);
        assert_eq!(rw.count(), 0);
    }

    #[test]
    fn push_one_value() {
        let mut rw = RollingWindow::new(3);
        rw.push(dec!(10));
        assert_eq!(rw.sum(), dec!(10));
        assert_eq!(rw.average(), dec!(10));
        assert_eq!(rw.count(), 1);
    }

    #[test]
    fn push_multiple_values_before_full() {
        let mut rw = RollingWindow::new(3);
        rw.push(dec!(10));
        rw.push(dec!(20));
        rw.push(dec!(30));
        assert_eq!(rw.sum(), dec!(60));
        assert_eq!(rw.average(), dec!(20));
        assert_eq!(rw.count(), 3);
    }

    #[test]
    fn push_beyond_capacity_evicts_oldest() {
        let mut rw = RollingWindow::new(3);
        rw.push(dec!(10));
        rw.push(dec!(20));
        rw.push(dec!(30));
        rw.push(dec!(40)); // evicts 10
        assert_eq!(rw.sum(), dec!(90)); // 20 + 30 + 40
        assert_eq!(rw.average(), dec!(30));
        assert_eq!(rw.count(), 3);
    }

    #[test]
    fn o1_average_on_full_window() {
        let mut rw = RollingWindow::new(5);
        for v in [dec!(1), dec!(2), dec!(3), dec!(4), dec!(5)] {
            rw.push(v);
        }
        assert_eq!(rw.average(), dec!(3));
        rw.push(dec!(6)); // evicts 1
        assert_eq!(rw.sum(), dec!(20)); // 2+3+4+5+6
        assert_eq!(rw.average(), dec!(4));
    }
}
