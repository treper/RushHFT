//! P² quantile estimator (Jain & Chlamtac, 1985). O(1) space online quantile.
//! Port of VisualHFT's `Studies.MarketResilience.Model.P2Quantile`.

#[derive(Debug, Clone)]
pub struct P2Quantile {
    p: f64,
    count: usize,
    q: [f64; 5],
    n: [f64; 5],
    np: [f64; 5],
    dn: [f64; 5],
}

impl P2Quantile {
    pub fn new(p: f64) -> Self {
        assert!(
            (0.0..=1.0).contains(&p),
            "p must be in (0, 1)"
        );
        Self {
            p,
            count: 0,
            q: [0.0; 5],
            n: [0.0; 5],
            np: [0.0; 5],
            dn: [0.0; 5],
        }
    }

    pub fn count(&self) -> usize {
        self.count
    }

    pub fn estimate(&self) -> f64 {
        if self.count < 5 {
            return if self.count == 0 {
                0.0
            } else {
                self.q[self.count.min(5) - 1]
            };
        }
        self.q[2]
    }

    pub fn observe(&mut self, x: f64) {
        if !x.is_finite() {
            return;
        }
        if self.count < 5 {
            self.q[self.count] = x;
            self.count += 1;
            if self.count == 5 {
                self.q.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
                for i in 0..5 {
                    self.n[i] = (i + 1) as f64;
                }
                self.np[0] = 1.0;
                self.np[1] = 1.0 + 2.0 * self.p;
                self.np[2] = 1.0 + 4.0 * self.p;
                self.np[3] = 3.0 + 2.0 * self.p;
                self.np[4] = 5.0;
                self.dn[0] = 0.0;
                self.dn[1] = self.p / 2.0;
                self.dn[2] = self.p;
                self.dn[3] = (1.0 + self.p) / 2.0;
                self.dn[4] = 1.0;
            }
            return;
        }

        // Find cell k and update extreme markers.
        let k;
        if x < self.q[0] {
            self.q[0] = x;
            k = 0;
        } else if x < self.q[1] {
            k = 0;
        } else if x < self.q[2] {
            k = 1;
        } else if x < self.q[3] {
            k = 2;
        } else if x < self.q[4] {
            k = 3;
        } else {
            self.q[4] = x;
            k = 3;
        }

        for i in (k + 1)..5 {
            self.n[i] += 1.0;
        }
        for i in 0..5 {
            self.np[i] += self.dn[i];
        }
        for i in 1..=3 {
            let d = self.np[i] - self.n[i];
            if (d >= 1.0 && self.n[i + 1] - self.n[i] > 1.0)
                || (d <= -1.0 && self.n[i - 1] - self.n[i] < -1.0)
            {
                let sign = if d >= 0.0 { 1.0 } else { -1.0 };
                let q_par = self.q[i]
                    + (sign / (self.n[i + 1] - self.n[i - 1]))
                        * ((self.n[i] - self.n[i - 1] + sign) * (self.q[i + 1] - self.q[i])
                            / (self.n[i + 1] - self.n[i])
                            + (self.n[i + 1] - self.n[i] - sign) * (self.q[i] - self.q[i - 1])
                                / (self.n[i] - self.n[i - 1]));
                let new_q = if self.q[i - 1] < q_par && q_par < self.q[i + 1] {
                    q_par
                } else {
                    let s = sign as i64;
                    let ni = self.n[i];
                    let nis = self.n[(i as i64 + s) as usize];
                    let qis = self.q[(i as i64 + s) as usize];
                    self.q[i] + sign * (qis - self.q[i]) / (nis - ni)
                };
                self.q[i] = new_q;
                self.n[i] += sign;
            }
        }
        self.count += 1;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn estimate_zero_before_any_observations() {
        let q = P2Quantile::new(0.5);
        assert_eq!(q.count(), 0);
        assert_eq!(q.estimate(), 0.0);
    }

    #[test]
    fn median_of_uniform_0_to_100_converges_near_50() {
        // 10k samples from a uniform [0,100) — median should converge to ~50.
        let mut q = P2Quantile::new(0.5);
        let mut rng = 0u64;
        for _ in 0..10_000 {
            // simple LCG for reproducible pseudo-uniform samples
            rng = rng.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
            let x = 100.0 * ((rng >> 33) as f64 / (1u64 << 31) as f64);
            q.observe(x);
        }
        let est = q.estimate();
        assert!((est - 50.0).abs() < 2.0, "median estimate was {est}, want ~50");
    }

    #[test]
    fn ignores_nan_and_infinity() {
        let mut q = P2Quantile::new(0.5);
        q.observe(f64::NAN);
        q.observe(f64::INFINITY);
        q.observe(f64::NEG_INFINITY);
        assert_eq!(q.count(), 0);
        assert_eq!(q.estimate(), 0.0);
    }

    #[test]
    fn p90_of_ascending_1_to_100_is_near_90() {
        let mut q = P2Quantile::new(0.9);
        for i in 1..=10_000 {
            q.observe(i as f64);
        }
        let est = q.estimate();
        assert!((est - 9000.0).abs() < 200.0, "p90 was {est}, want ~9000");
    }
}
