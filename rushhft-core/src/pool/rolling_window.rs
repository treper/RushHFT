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

/// f64 rolling window for MR recovery-time series (existing `RollingWindow`
/// is `Decimal`-typed; recovery times are millisecond floats).
pub struct RollingWindowF64 {
    buffer: Vec<f64>,
    index: usize,
    count: usize,
    capacity: usize,
    sum: f64,
}

impl RollingWindowF64 {
    pub fn new(capacity: usize) -> Self {
        Self {
            buffer: vec![0.0; capacity],
            index: 0,
            count: 0,
            capacity,
            sum: 0.0,
        }
    }

    pub fn push(&mut self, value: f64) {
        if self.count == self.capacity {
            self.sum -= self.buffer[self.index];
        } else {
            self.count += 1;
        }
        self.buffer[self.index] = value;
        self.sum += value;
        self.index = (self.index + 1) % self.capacity;
    }

    pub fn average(&self) -> f64 {
        if self.count == 0 {
            0.0
        } else {
            self.sum / self.count as f64
        }
    }

    pub fn sum(&self) -> f64 {
        self.sum
    }

    pub fn count(&self) -> usize {
        self.count
    }

    /// Returns the median of values currently in the window. None if empty.
    pub fn median(&self) -> Option<f64> {
        if self.count == 0 {
            return None;
        }
        // Collect from the ring buffer in order of insertion.
        let start = if self.count == self.capacity {
            self.index
        } else {
            0
        };
        let mut v: Vec<f64> = (0..self.count)
            .map(|i| self.buffer[(start + i) % self.capacity])
            .collect();
        v.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
        let mid = v.len() / 2;
        if v.len().is_multiple_of(2) {
            Some((v[mid - 1] + v[mid]) / 2.0)
        } else {
            Some(v[mid])
        }
    }
}

#[cfg(test)]
mod tests_f64 {
    use super::*;

    #[test]
    fn empty_f64_window_has_zero_average() {
        let rw = RollingWindowF64::new(3);
        assert_eq!(rw.average(), 0.0);
        assert_eq!(rw.count(), 0);
    }

    #[test]
    fn push_beyond_capacity_evicts_oldest_f64() {
        let mut rw = RollingWindowF64::new(3);
        rw.push(10.0);
        rw.push(20.0);
        rw.push(30.0);
        rw.push(40.0); // evicts 10.0
        assert_eq!(rw.count(), 3);
        assert!((rw.average() - 30.0).abs() < 1e-9);
        assert!((rw.sum() - 90.0).abs() < 1e-9);
    }

    #[test]
    fn median_of_window() {
        let mut rw = RollingWindowF64::new(5);
        for v in [10.0, 20.0, 30.0, 40.0, 50.0] {
            rw.push(v);
        }
        assert!((rw.median().unwrap() - 30.0).abs() < 1e-9);
    }

    #[test]
    fn median_even_count_averages_middle_two() {
        let mut rw = RollingWindowF64::new(4);
        for v in [10.0, 20.0, 30.0, 40.0] {
            rw.push(v);
        }
        // Even count: median = (20 + 30) / 2 = 25
        assert!((rw.median().unwrap() - 25.0).abs() < 1e-9);
    }
}
