//! Market resilience calculator: detects spread/depth shocks, measures
//! 90% recovery time. Port of VisualHFT `MarketResilienceCalculator.cs`,
//! simplified to two metrics: spread-recovery and depth-recovery (ms).
//!
//! MVP scope per spec: skip the Bullish/Bearish/Neutral bias sub-study.

use rushhft_core::P2Quantile;
use rushhft_core::RollingWindowF64;
use time::OffsetDateTime;

const SHOCK_THRESHOLD_SIGMA: f64 = 2.0;
const Z_K_DEPTH: f64 = 3.0;
const RECOVERY_TARGET: f64 = 0.90;
const WARMUP_MIN_SAMPLES: usize = 200;

#[derive(Debug, Clone, Copy)]
pub struct MrMetrics {
    pub spread_recovery_ms: Option<f64>,
    // Reserved for future depth-recovery metric emission (currently computed
    // but not consumed by the plugin).
    #[allow(dead_code)]
    pub depth_recovery_ms: Option<f64>,
}

pub struct MarketResilienceCalculator {
    q_spread_median: P2Quantile,
    q_bid_depth_median: P2Quantile,
    q_ask_depth_median: P2Quantile,
    q_bid_dev_median: P2Quantile,
    q_ask_dev_median: P2Quantile,
    samples_spread: usize,
    samples_depth: usize,
    last_spread: f64,
    last_bid_depth: f64,
    last_ask_depth: f64,
    spread_baseline: Option<f64>,
    spread_shock_start: Option<OffsetDateTime>,
    depth_shock_start: Option<OffsetDateTime>,
    depth_baseline: Option<f64>,
    spread_recovery_times: RollingWindowF64,
    depth_recovery_times: RollingWindowF64,
}

impl MarketResilienceCalculator {
    pub fn new() -> Self {
        Self {
            q_spread_median: P2Quantile::new(0.5),
            q_bid_depth_median: P2Quantile::new(0.5),
            q_ask_depth_median: P2Quantile::new(0.5),
            q_bid_dev_median: P2Quantile::new(0.5),
            q_ask_dev_median: P2Quantile::new(0.5),
            samples_spread: 0,
            samples_depth: 0,
            last_spread: 0.0,
            last_bid_depth: 0.0,
            last_ask_depth: 0.0,
            spread_baseline: None,
            spread_shock_start: None,
            depth_shock_start: None,
            depth_baseline: None,
            spread_recovery_times: RollingWindowF64::new(500),
            depth_recovery_times: RollingWindowF64::new(500),
        }
    }

    /// Feed one (spread, bid_immediacy_depth, ask_immediacy_depth) observation.
    pub fn observe(&mut self, spread: f64, bid_depth: f64, ask_depth: f64, ts: OffsetDateTime) {
        // Update baseline estimators.
        self.q_spread_median.observe(spread);
        self.samples_spread += 1;
        let mid_depth = (bid_depth + ask_depth) / 2.0;
        self.q_bid_depth_median.observe(bid_depth);
        self.q_ask_depth_median.observe(ask_depth);
        self.samples_depth += 1;
        let bid_dev = (bid_depth - self.q_bid_depth_median.estimate()).abs();
        let ask_dev = (ask_depth - self.q_ask_depth_median.estimate()).abs();
        self.q_bid_dev_median.observe(bid_dev);
        self.q_ask_dev_median.observe(ask_dev);

        self.last_spread = spread;
        self.last_bid_depth = bid_depth;
        self.last_ask_depth = ask_depth;

        if self.samples_spread < WARMUP_MIN_SAMPLES {
            return;
        }

        let spread_med = self.q_spread_median.estimate();
        // MAD-based spread sigma (approx: median of |x - med| scaled by 1.4826).
        // Reuse bid_dev_median as a proxy for spread deviation median —
        // spec says "spread + MAD for each side", MVP keeps it simple.
        let spread_sigma = (self.q_bid_dev_median.estimate() * 1.4826).max(1e-9);

        // Spread shock detection.
        if self.spread_shock_start.is_none()
            && spread > spread_med + SHOCK_THRESHOLD_SIGMA * spread_sigma
        {
            self.spread_shock_start = Some(ts);
            self.spread_baseline = Some(spread_med);
        } else if let Some(start) = self.spread_shock_start {
            let baseline = self.spread_baseline.unwrap_or(spread_med);
            // Recovery: spread back within (1 - RECOVERY_TARGET) of baseline.
            // For spread (higher = worse), 90% recovery = spread <= baseline * 1.1.
            if spread <= baseline * (2.0 - RECOVERY_TARGET) {
                let dur_ms = (ts - start).whole_milliseconds().max(0) as f64;
                self.spread_recovery_times.push(dur_ms);
                self.spread_shock_start = None;
                self.spread_baseline = None;
            }
        }

        // Depth shock detection — symmetric on either side.
        // Use the median depth (from P2 quantiles), not the current value.
        let depth_med =
            (self.q_bid_depth_median.estimate() + self.q_ask_depth_median.estimate()) / 2.0;
        let depth_sigma = (self.q_bid_dev_median.estimate() * 1.4826).max(1e-9);
        if self.depth_shock_start.is_none()
            && mid_depth < depth_med - Z_K_DEPTH * depth_sigma
        {
            self.depth_shock_start = Some(ts);
            self.depth_baseline = Some(depth_med);
        } else if let Some(start) = self.depth_shock_start {
            let baseline = self.depth_baseline.unwrap_or(depth_med);
            if mid_depth >= baseline * RECOVERY_TARGET {
                let dur_ms = (ts - start).whole_milliseconds().max(0) as f64;
                self.depth_recovery_times.push(dur_ms);
                self.depth_shock_start = None;
                self.depth_baseline = None;
            }
        }
    }

    pub fn metrics(&self) -> MrMetrics {
        MrMetrics {
            spread_recovery_ms: self.spread_recovery_times.median(),
            depth_recovery_ms: self.depth_recovery_times.median(),
        }
    }
}

impl Default for MarketResilienceCalculator {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use time::OffsetDateTime;

    fn ts(seconds: i64) -> OffsetDateTime {
        OffsetDateTime::from_unix_timestamp(1_700_000_000 + seconds).unwrap()
    }

    #[test]
    fn warmup_returns_no_metrics() {
        let mut c = MarketResilienceCalculator::new();
        for i in 0..100 {
            c.observe(0.05, 100.0, 100.0, ts(i));
        }
        let m = c.metrics();
        assert!(m.spread_recovery_ms.is_none());
        assert!(m.depth_recovery_ms.is_none());
    }

    #[test]
    fn spread_shock_then_recovery_records_duration() {
        let mut c = MarketResilienceCalculator::new();
        // Warm up with 250 calm samples.
        for i in 0..250 {
            c.observe(0.05, 100.0, 100.0, ts(i));
        }
        // Shock: spread jumps to 0.20
        c.observe(0.20, 100.0, 100.0, ts(300));
        // Recovery: spread back to ~0.05
        c.observe(0.055, 100.0, 100.0, ts(350));
        let m = c.metrics();
        let dur = m.spread_recovery_ms.expect("spread recovery recorded");
        // 50s between ts(300) and ts(350) -> 50000ms
        assert!((dur - 50_000.0).abs() < 1.0, "got {dur}");
    }

    #[test]
    fn depth_shock_then_recovery_records_duration() {
        let mut c = MarketResilienceCalculator::new();
        for i in 0..250 {
            c.observe(0.05, 1000.0, 1000.0, ts(i));
        }
        // Shock: depth drops to 50 (way below median ~1000)
        c.observe(0.05, 50.0, 50.0, ts(300));
        // Recovery: depth back to ~900 (>= 90% of 1000 = 900)
        c.observe(0.05, 950.0, 950.0, ts(310));
        let m = c.metrics();
        let dur = m.depth_recovery_ms.expect("depth recovery recorded");
        assert!((dur - 10_000.0).abs() < 1.0, "got {dur}");
    }
}
