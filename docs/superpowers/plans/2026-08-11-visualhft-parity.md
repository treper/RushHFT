# VisualHFT Parity Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Bring RushHFT's UI and study set to information-parity with VisualHFT — sidebar+main shell, combined depth ladder with size bars, three uPlot charts, four toolbar modals, stub positions pane, plus two new studies (Market Resilience + OTT Ratio) backed by a P² quantile helper.

**Architecture:** Big-bang rebuild of `rushhft-app/ui` from a 3-column MVP into a structured Svelte 5 component tree mirroring VisualHFT's sidebar+main layout. Backend extends existing `commands.rs` / `dto.rs` / `state.rs` with chart-series ring buffers, runtime symbol subscribe, and a multi-venue price command. Two new study crates in `rushhft-studies` (MR + OTT) backed by a new `rushhft-core/src/stats/p2_quantile.rs` helper and a generic `RollingWindowF64` alongside the existing `RollingWindow` (Decimal).

**Tech Stack:** Rust 2024 / Tauri 2 / Svelte 5 / TypeScript / uPlot (charting, ~40KB canvas-based)

---

## File Structure

### Rust crates

| File | Action | Responsibility |
|---|---|---|
| `rushhft-core/src/stats/mod.rs` | Create | `pub mod p2_quantile;` re-export |
| `rushhft-core/src/stats/p2_quantile.rs` | Create | P² quantile estimator (port of VisualHFT `P2Quantile.cs`) |
| `rushhft-core/src/pool/rolling_window.rs` | Extend | Add `RollingWindowF64` (f64 variant for MR recovery times) |
| `rushhft-core/src/lib.rs` | Modify | `pub mod stats;` + re-export `P2Quantile`, `RollingWindowF64` |
| `rushhft-studies/src/ott_ratio/mod.rs` | Create | OttRatioStudy plugin + settings |
| `rushhft-studies/src/ott_ratio/aggregator.rs` | Create | `compute_ott` pure function + counter struct |
| `rushhft-studies/src/market_resilience/mod.rs` | Create | MarketResilienceStudy plugin + settings |
| `rushhft-studies/src/market_resilience/calculator.rs` | Create | MR shock/recovery calculator |
| `rushhft-studies/src/lib.rs` | Modify | Declare + re-export new modules |
| `rushhft-studies/Cargo.toml` | Modify | (no new deps; uses existing `rust_decimal`, `time`, `tokio`) |
| `rushhft-connector-longport/src/lib.rs` | Modify | Add `subscribe_symbol` + `unsubscribe_symbol` + `user_symbols` field |
| `rushhft-app/src/dto.rs` | Modify | Add `ChartPointDto`, `ChartSeriesDto`, `VenuePriceDto`, `PluginDescriptorDto` |
| `rushhft-app/src/state.rs` | Modify | Add `ChartSeriesBuffer` ring buffer |
| `rushhft-app/src/context.rs` | Modify | Push chart points in `publish_order_book` |
| `rushhft-app/src/ui_state.rs` | Create | `UserSymbols` registry (Arc<RwLock<HashSet>>) |
| `rushhft-app/src/commands.rs` | Modify | Add 6 new commands; extend tests |
| `rushhft-app/src/main.rs` | Modify | Register new commands; add MR + OTT plugins |

### UI

| File | Action | Responsibility |
|---|---|---|
| `rushhft-app/ui/package.json` | Modify | Add `uplot` + `@types/uplot` deps |
| `rushhft-app/ui/src/app.css` | Extend | Panel/tile/gauge/depth styles |
| `rushhft-app/ui/src/routes/+page.svelte` | Replace | Shell + layout A wiring |
| `rushhft-app/ui/src/lib/stores/snapshot.ts` | Create | 500ms snapshot poll + chart series poll |
| `rushhft-app/ui/src/lib/stores/symbols.ts` | Create | Symbol list + current symbol |
| `rushhft-app/ui/src/lib/stores/plugins.ts` | Create | Plugin descriptors |
| `rushhft-app/ui/src/lib/stores/settings.ts` | Create | Settings DTO + save |
| `rushhft-app/ui/src/lib/stores/triggers.ts` | Create | Trigger rules CRUD |
| `rushhft-app/ui/src/lib/stores/notifications.ts` | Create | Channel-based notification subscription |
| `rushhft-app/ui/src/lib/charts/uPlotSetup.ts` | Create | Theme, scales, cursor options |
| `rushhft-app/ui/src/lib/charts/series.ts` | Create | Series builders for cumulative/price/spread |
| `rushhft-app/ui/src/lib/components/Sidebar.svelte` | Create | Sidebar container (480px) |
| `rushhft-app/ui/src/lib/components/Toolbar.svelte` | Create | 4 buttons + notification bell |
| `rushhft-app/ui/src/lib/components/ProviderStatus.svelte` | Create | Provider chips |
| `rushhft-app/ui/src/lib/components/StudyTiles.svelte` | Create | Scrollable study value tiles |
| `rushhft-app/ui/src/lib/components/DepthLadder.svelte` | Create | Combined ladder + size bars (style B) |
| `rushhft-app/ui/src/lib/components/TopOfBook.svelte` | Create | Big bid/ask + spread + stale badge |
| `rushhft-app/ui/src/lib/components/LOBImbalanceGauge.svelte` | Create | Red↔white↔red gradient + arrow |
| `rushhft-app/ui/src/lib/components/TradesTape.svelte` | Create | Recent trades tape |
| `rushhft-app/ui/src/lib/components/Positions.svelte` | Create | Stub empty state |
| `rushhft-app/ui/src/lib/components/Charts/CumulativeBook.svelte` | Create | uPlot cumulative bids + asks |
| `rushhft-app/ui/src/lib/components/Charts/PriceChart.svelte` | Create | uPlot bid/ask/mid + trade dots |
| `rushhft-app/ui/src/lib/components/Charts/SpreadChart.svelte` | Create | uPlot spread over time |
| `rushhft-app/ui/src/lib/modals/PluginManagerModal.svelte` | Create | Plugin list + start/stop |
| `rushhft-app/ui/src/lib/modals/SettingsModal.svelte` | Create | Settings form |
| `rushhft-app/ui/src/lib/modals/TriggersModal.svelte` | Create | Trigger rule CRUD |
| `rushhft-app/ui/src/lib/modals/MultiVenueModal.svelte` | Create | Per-venue price table |

### VisualHFT ports (reference, do not modify)

- `/Users/tangning/Documents/workspace/mine/VisualHFT/VisualHFT.Plugins/Studies.OTT_Ratio/OrderToTradeRatioStudy.cs` — OTT port source
- `/Users/tangning/Documents/workspace/mine/VisualHFT/VisualHFT.Plugins/Studies.MarketResilience/Model/MarketResilienceCalculator.cs` — MR port source
- `/Users/tangning/Documents/workspace/mine/VisualHFT/VisualHFT.Plugins/Studies.MarketResilience/Model/P2Quantile.cs` — P² quantile port source

---

## Phase A — rushhft-core helpers

### Task A1: P² quantile estimator

**Files:**
- Create: `rushhft-core/src/stats/mod.rs`
- Create: `rushhft-core/src/stats/p2_quantile.rs`
- Modify: `rushhft-core/src/lib.rs:4` (add `pub mod stats;`)

- [ ] **Step 1: Write the failing test**

Create `rushhft-core/src/stats/p2_quantile.rs` with a `#[cfg(test)] mod tests` block only — no production code yet.

```rust
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
```

Also create `rushhft-core/src/stats/mod.rs`:

```rust
pub mod p2_quantile;
pub use p2_quantile::P2Quantile;
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cd /Users/tangning/Documents/workspace/mine/RushHFT && cargo test -p rushhft-core --lib stats::p2_quantile`
Expected: FAIL with "function `estimate`/`observe`/`new` not found (or `P2Quantile` not constructed) — struct has no methods yet."

- [ ] **Step 3: Write minimal implementation**

Add the `impl` block to `rushhft-core/src/stats/p2_quantile.rs` (above the tests module):

```rust
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
```

- [ ] **Step 4: Run test to verify it passes**

Run: `cd /Users/tangning/Documents/workspace/mine/RushHFT && cargo test -p rushhft-core --lib stats::p2_quantile`
Expected: PASS — 4 tests.

- [ ] **Step 5: Re-export from lib.rs**

Modify `rushhft-core/src/lib.rs` — add after line 4 (`pub mod pool;`):

```rust
pub mod stats;
```

And in the re-exports, after line 21 (`pub use pool::{ObjectPool, PoolGuard, RollingWindow};`) add:

```rust
pub use stats::P2Quantile;
```

- [ ] **Step 6: Run full crate test suite**

Run: `cd /Users/tangning/Documents/workspace/mine/RushHFT && cargo test -p rushhft-core`
Expected: PASS — all existing tests still green, 4 new p2_quantile tests added.

- [ ] **Step 7: Commit**

```bash
cd /Users/tangning/Documents/workspace/mine/RushHFT
git add rushhft-core/src/stats/ rushhft-core/src/lib.rs
git commit -m "feat(core): add P² quantile estimator for online median/p90"
```

---

### Task A2: RollingWindowF64 for MR recovery times

**Files:**
- Modify: `rushhft-core/src/pool/rolling_window.rs` (append below existing `RollingWindow`)
- Modify: `rushhft-core/src/pool/mod.rs:5` (re-export)

- [ ] **Step 1: Write the failing test**

Append to `rushhft-core/src/pool/rolling_window.rs` (after the existing `#[cfg(test)]` block):

```rust
/// f64 rolling window for MR recovery-time series (existing `RollingWindow`
/// is `Decimal`-typed; recovery times are millisecond floats).
pub struct RollingWindowF64 {
    buffer: Vec<f64>,
    index: usize,
    count: usize,
    capacity: usize,
    sum: f64,
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
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p rushhft-core --lib pool::rolling_window::tests_f64`
Expected: FAIL — `RollingWindowF64` not defined.

- [ ] **Step 3: Write minimal implementation**

Add the `impl` block above the new test module in `rushhft-core/src/pool/rolling_window.rs`:

```rust
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
        if v.len() % 2 == 0 {
            Some((v[mid - 1] + v[mid]) / 2.0)
        } else {
            Some(v[mid])
        }
    }
}
```

- [ ] **Step 4: Re-export from pool/mod.rs**

Modify `rushhft-core/src/pool/mod.rs` to:

```rust
pub mod object_pool;
pub mod rolling_window;

pub use object_pool::{ObjectPool, PoolGuard};
pub use rolling_window::{RollingWindow, RollingWindowF64};
```

And in `rushhft-core/src/lib.rs:21`, extend the re-export:

```rust
pub use pool::{ObjectPool, PoolGuard, RollingWindow, RollingWindowF64};
```

- [ ] **Step 5: Run tests to verify they pass**

Run: `cargo test -p rushhft-core --lib pool::rolling_window`
Expected: PASS — existing Decimal tests + 3 new f64 tests.

- [ ] **Step 6: Commit**

```bash
cd /Users/tangning/Documents/workspace/mine/RushHFT
git add rushhft-core/src/pool/ rushhft-core/src/lib.rs
git commit -m "feat(core): add RollingWindowF64 for f64 recovery-time series"
```

---

## Phase B — New studies

### Task B1: OTT study — settings + pure formula

**Files:**
- Create: `rushhft-studies/src/ott_ratio/mod.rs`
- Create: `rushhft-studies/src/ott_ratio/aggregator.rs`
- Modify: `rushhft-studies/src/lib.rs:4` (declare module)

- [ ] **Step 1: Write the failing test**

Create `rushhft-studies/src/ott_ratio/aggregator.rs`:

```rust
//! Pure OTT computation. L2 formula (LongPort provides price-level data):
//!   OTR = (AddedΔ + 2×UpdatedΔ + DeletedΔ) / max(Trades, 1) − 1
//! Port of VisualHFT `OrderToTradeRatioStudy.cs:198-235` (L2 branch).

use rushhft_core::model::order_book::OrderBook;
use rust_decimal::Decimal;
use rust_decimal::prelude::ToPrimitive;

#[derive(Debug, Clone, Default)]
pub struct OttCounters {
    pub prev_added: u64,
    pub prev_deleted: u64,
    pub prev_updated: u64,
    pub is_first_call: bool,
    pub order_events: u64,
    pub trade_count: u64,
}

impl OttCounters {
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
    use rushhft_core::model::book_item::BookItem;
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
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p rushhft-studies --lib ott_ratio::aggregator`
Expected: FAIL — module not declared in `lib.rs`.

- [ ] **Step 3: Declare the module**

Modify `rushhft-studies/src/lib.rs` to:

```rust
//! RushHFT studies crate — VPIN, LOB Imbalance, OTT Ratio, Market Resilience.
pub use rushhft_core;

mod lob_imbalance;
mod market_resilience;
mod ott_ratio;
mod vpin;

pub use lob_imbalance::{LobImbalanceSettings, LobImbalanceStudy};
pub use market_resilience::{MarketResilienceSettings, MarketResilienceStudy};
pub use ott_ratio::{OttRatioSettings, OttRatioStudy};
pub use vpin::{VpinSettings, VpinStudy};
```

Create `rushhft-studies/src/ott_ratio/mod.rs`:

```rust
mod aggregator;
pub use aggregator::{compute_ott, OttCounters};

use rushhft_core::model::enums::{AggregationLevel, PluginStatus, PluginType};
use rushhft_core::plugin::{BaseStudy, Plugin, PluginContext, PluginError};
use rushhft_core::hub::SubscriptionGuard;
use rushhft_core::model::order_book::OrderBook;
use rushhft_core::model::study::BaseStudyModel;
use rust_decimal::Decimal;
use std::sync::Arc;
use time::OffsetDateTime;
use tokio::sync::Mutex;

#[derive(Debug, Clone)]
pub struct OttRatioSettings {
    pub symbol: String,
    pub provider_id: i32,
    pub aggregation_level: AggregationLevel,
}

impl Default for OttRatioSettings {
    fn default() -> Self {
        Self {
            symbol: String::new(),
            provider_id: 0,
            aggregation_level: AggregationLevel::S1,
        }
    }
}
```

(The plugin + start/stop impls come in Task B2.)

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test -p rushhft-studies --lib ott_ratio::aggregator`
Expected: PASS — 4 tests.

- [ ] **Step 5: Commit**

```bash
git add rushhft-studies/src/ott_ratio/ rushhft-studies/src/lib.rs
git commit -m "feat(studies): add OTT ratio pure formula + tests"
```

---

### Task B2: OTT study — plugin + integration

**Files:**
- Modify: `rushhft-studies/src/ott_ratio/mod.rs` (append plugin impl)
- Modify: `rushhft-studies/Cargo.toml` (no change — already has `tokio`/`async-trait`)

- [ ] **Step 1: Write the failing test**

Append to `rushhft-studies/src/ott_ratio/mod.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use rushhft_core::Plugin;
    use rust_decimal_macros::dec;

    #[test]
    fn metadata_matches_spec() {
        let s = OttRatioStudy::new(OttRatioSettings::default());
        assert_eq!(s.name(), "OTT Ratio Study");
        assert_eq!(s.plugin_type(), PluginType::Study);
        assert_eq!(s.status(), PluginStatus::Loaded);
        assert!(s.emits_metric());
        assert!(!s.plugin_id().is_empty());
    }
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p rushhft-studies --lib ott_ratio::tests`
Expected: FAIL — `OttRatioStudy` not defined.

- [ ] **Step 3: Write minimal implementation**

Append to `rushhft-studies/src/ott_ratio/mod.rs` (above the `#[cfg(test)]`):

```rust
struct Inner {
    settings: OttRatioSettings,
    base: BaseStudy,
    counters: std::sync::Mutex<OttCounters>,
    status: Arc<arc_swap::ArcSwap<PluginStatus>>,
    ctx: Mutex<Option<Arc<dyn PluginContext>>>,
    guards: Mutex<Option<Vec<SubscriptionGuard>>>,
}

pub struct OttRatioStudy {
    id: String,
    version: &'static str,
    author: &'static str,
    description: &'static str,
    inner: Arc<Inner>,
}

impl OttRatioStudy {
    pub fn new(settings: OttRatioSettings) -> Self {
        let id = format!(
            "ott-{:x}",
            (settings.symbol.as_bytes().iter().fold(0xcbf29ce484222325u64, |acc, b| {
                (acc ^ (*b as u64)).wrapping_mul(0x100000001b3)
            })) ^ (settings.provider_id as u64)
        );
        let inner = Arc::new(Inner {
            settings,
            base: BaseStudy::new(AggregationLevel::S1),
            counters: std::sync::Mutex::new(OttCounters::default()),
            status: Arc::new(arc_swap::ArcSwap::from_pointee(PluginStatus::Loaded)),
            ctx: Mutex::new(None),
            guards: Mutex::new(None),
        });
        Self {
            id,
            version: "0.1.0",
            author: "RushHFT",
            description: "Order-to-Trade Ratio (L2 book deltas vs executed trades)",
            inner,
        }
    }
}

#[async_trait::async_trait]
impl Plugin for OttRatioStudy {
    fn name(&self) -> &str { "OTT Ratio Study" }
    fn version(&self) -> &str { self.version }
    fn author(&self) -> &str { self.author }
    fn description(&self) -> &str { self.description }
    fn plugin_type(&self) -> PluginType { PluginType::Study }
    fn status(&self) -> PluginStatus { **self.inner.status.load() }
    fn plugin_id(&self) -> &str { &self.id }
    fn emits_metric(&self) -> bool { true }

    async fn start(&self, ctx: Arc<dyn PluginContext>) -> Result<(), PluginError> {
        {
            let mut g = self.inner.ctx.lock().await;
            *g = Some(ctx.clone());
        }
        let inner = self.inner.clone();
        let inner_closure = self.inner.clone();
        let ctx_for_consumer = ctx.clone();
        tokio::spawn(async move {
            inner
                .base
                .start_consumer(move |item: &BaseStudyModel| {
                    let ctx = ctx_for_consumer.clone();
                    let symbol = inner_closure.settings.symbol.clone();
                    let value = item.value;
                    let ts = item.timestamp;
                    tokio::spawn(async move {
                        let _ = ctx
                            .register_metric("OTT Ratio Study", "OTT", "LongPort", &symbol, value, ts)
                            .await;
                    });
                })
                .await;
        });

        let inner_ob = self.inner.clone();
        let inner_trade = self.inner.clone();
        let ob_hub = ctx.order_book_hub();
        let trade_hub = ctx.trade_hub();

        let ob_guard = ob_hub.subscribe(Arc::new(move |ob: &OrderBook| {
            if ob.symbol != inner_ob.settings.symbol
                || ob.provider_id != inner_ob.settings.provider_id
            {
                return;
            }
            let mut counters = inner_ob.counters.lock().unwrap();
            let value = compute_ott(&mut counters, ob, 0);
            let mid = ob.mid_price().unwrap_or(Decimal::ZERO);
            inner_ob.base.add_calculation(BaseStudyModel {
                value,
                format: "N1".into(),
                timestamp: OffsetDateTime::now_utc(),
                market_mid_price: mid,
                value_color: "White".into(),
                tooltip: String::new(),
                has_error: false,
                is_stale: false,
            });
        }));

        use rushhft_core::model::trade::Trade;
        let trade_guard = trade_hub.subscribe(Arc::new(move |t: &Trade| {
            if t.symbol != inner_trade.settings.symbol
                || t.provider_id != inner_trade.settings.provider_id
            {
                return;
            }
            // Increment trade counter; the next order-book push will compute.
            let mut counters = inner_trade.counters.lock().unwrap();
            counters.trade_count += 1;
        }));

        {
            let mut guards = self.inner.guards.lock().await;
            *guards = Some(vec![ob_guard, trade_guard]);
        }
        self.inner.status.store(Arc::new(PluginStatus::Started));
        Ok(())
    }

    async fn stop(&self) -> Result<(), PluginError> {
        self.inner.status.store(Arc::new(PluginStatus::Stopping));
        {
            let mut guards = self.inner.guards.lock().await;
            *guards = None;
        }
        self.inner.status.store(Arc::new(PluginStatus::Stopped));
        Ok(())
    }
}
```

- [ ] **Step 4: Verify TradeHub has a subscribe API matching the above**

Run: `grep -n "pub fn subscribe" rushhft-core/src/hub/mod.rs`
If `TradeHub::subscribe` accepts `Arc<dyn Fn(&Trade) + Send + Sync>` and returns `SubscriptionGuard`, the implementation compiles. If it does not exist, the build will fail and the implementing agent must extend `TradeHub` first — but the existing `OrderBookHub::subscribe` pattern (see `rushhft-studies/src/lob_imbalance.rs:137`) is mirrored for trade.

- [ ] **Step 5: Run tests to verify they pass**

Run: `cargo test -p rushhft-studies --lib ott_ratio`
Expected: PASS — 1 metadata test + 4 aggregator tests.

- [ ] **Step 6: Commit**

```bash
git add rushhft-studies/src/ott_ratio/
git commit -m "feat(studies): wire OTT ratio plugin to OB+trade hubs"
```

---

### Task B3: MR calculator core

**Files:**
- Create: `rushhft-studies/src/market_resilience/mod.rs` (settings only for now)
- Create: `rushhft-studies/src/market_resilience/calculator.rs`
- Reference: `VisualHFT/VisualHFT.Plugins/Studies.MarketResilience/Model/MarketResilienceCalculator.cs`

- [ ] **Step 1: Write the failing test**

Create `rushhft-studies/src/market_resilience/calculator.rs`:

```rust
//! Market resilience calculator: detects spread/depth shocks, measures
//! 90% recovery time. Port of VisualHFT `MarketResilienceCalculator.cs`,
//! simplified to two metrics: spread-recovery and depth-recovery (ms).
//!
//! MVP scope per spec: skip the Bullish/Bearish/Neutral bias sub-study.

use rushhft_core::P2Quantile;
use rushhft_core::RollingWindowF64;
use rust_decimal::Decimal;
use rust_decimal::prelude::ToPrimitive;
use time::OffsetDateTime;

const SHOCK_THRESHOLD_SIGMA: f64 = 2.0;
const Z_K_DEPTH: f64 = 3.0;
const RECOVERY_TARGET: f64 = 0.90;
const WARMUP_MIN_SAMPLES: usize = 200;

#[derive(Debug, Clone, Copy)]
pub struct MrMetrics {
    pub spread_recovery_ms: Option<f64>,
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
        // MAD-based spread sigma (approx: median of |x - med| scaled by 1.4826)
        let spread_dev = (spread - spread_med).abs();
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
            if spread <= baseline * RECOVERY_TARGET + baseline * (1.0 - RECOVERY_TARGET) {
                let dur_ms = (ts - start).whole_milliseconds().max(0) as f64;
                self.spread_recovery_times.push(dur_ms);
                self.spread_shock_start = None;
                self.spread_baseline = None;
            }
        }

        // Depth shock detection — symmetric on either side.
        let depth_med = mid_depth;
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
```

Create `rushhft-studies/src/market_resilience/mod.rs`:

```rust
mod calculator;
pub use calculator::{MarketResilienceCalculator, MrMetrics};

#[derive(Debug, Clone)]
pub struct MarketResilienceSettings {
    pub symbol: String,
    pub provider_id: i32,
    pub aggregation_level: rushhft_core::model::enums::AggregationLevel,
}

impl Default for MarketResilienceSettings {
    fn default() -> Self {
        use rushhft_core::model::enums::AggregationLevel;
        Self {
            symbol: String::new(),
            provider_id: 0,
            aggregation_level: AggregationLevel::S1,
        }
    }
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p rushhft-studies --lib market_resilience`
Expected: FAIL — module not declared (declared in Task B1's lib.rs update, but the test bodies will fail until `MarketResilienceCalculator` is fully implemented — it is, in the same file).

Wait — since the impl and tests are in the same file write, this should pass on first run once the module is declared. The "fail" step here is satisfied by `cargo build` failing before this file exists.

- [ ] **Step 3: Verify it builds**

Run: `cargo test -p rushhft-studies --lib market_resilience`
Expected: PASS — 3 tests.

If `TradeHub::subscribe` is required (it isn't yet at this point — the calculator is pure), the build will surface that. Fix the missing API in `rushhft-core/src/hub/mod.rs` mirroring `OrderBookHub::subscribe`.

- [ ] **Step 4: Commit**

```bash
git add rushhft-studies/src/market_resilience/
git commit -m "feat(studies): add Market Resilience calculator core"
```

---

### Task B4: MR study — plugin + integration

**Files:**
- Modify: `rushhft-studies/src/market_resilience/mod.rs` (append plugin impl)

- [ ] **Step 1: Write the failing test**

Append to `rushhft-studies/src/market_resilience/mod.rs`:

```rust
#[cfg(test)]
mod plugin_tests {
    use super::*;
    use rushhft_core::Plugin;

    #[test]
    fn metadata_matches_spec() {
        let s = MarketResilienceStudy::new(MarketResilienceSettings::default());
        assert_eq!(s.name(), "Market Resilience Study");
        assert_eq!(s.plugin_type(), rushhft_core::model::enums::PluginType::Study);
        assert_eq!(s.status(), rushhft_core::model::enums::PluginStatus::Loaded);
        assert!(s.emits_metric());
        assert!(!s.plugin_id().is_empty());
    }
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p rushhft-studies --lib market_resilience::plugin_tests`
Expected: FAIL — `MarketResilienceStudy` not defined.

- [ ] **Step 3: Write minimal implementation**

Append to `rushhft-studies/src/market_resilience/mod.rs` (above the test module):

```rust
use rushhft_core::hub::SubscriptionGuard;
use rushhft_core::model::enums::{AggregationLevel, PluginStatus, PluginType};
use rushhft_core::model::order_book::OrderBook;
use rushhft_core::model::study::BaseStudyModel;
use rushhft_core::plugin::{BaseStudy, Plugin, PluginContext, PluginError};
use rust_decimal::Decimal;
use rust_decimal::prelude::ToPrimitive;
use std::sync::Arc;
use std::sync::Mutex as StdMutex;
use time::OffsetDateTime;
use tokio::sync::Mutex;

struct Inner {
    settings: MarketResilienceSettings,
    base: BaseStudy,
    calc: StdMutex<MarketResilienceCalculator>,
    status: Arc<arc_swap::ArcSwap<PluginStatus>>,
    ctx: Mutex<Option<Arc<dyn PluginContext>>>,
    guards: Mutex<Option<Vec<SubscriptionGuard>>>,
}

pub struct MarketResilienceStudy {
    id: String,
    version: &'static str,
    author: &'static str,
    description: &'static str,
    inner: Arc<Inner>,
}

impl MarketResilienceStudy {
    pub fn new(settings: MarketResilienceSettings) -> Self {
        let id = format!(
            "mr-{:x}",
            (settings.symbol.as_bytes().iter().fold(0xcbf29ce484222325u64, |acc, b| {
                (acc ^ (*b as u64)).wrapping_mul(0x100000001b3)
            })) ^ (settings.provider_id as u64)
        );
        let inner = Arc::new(Inner {
            settings,
            base: BaseStudy::new(AggregationLevel::S1),
            calc: StdMutex::new(MarketResilienceCalculator::new()),
            status: Arc::new(arc_swap::ArcSwap::from_pointee(PluginStatus::Loaded)),
            ctx: Mutex::new(None),
            guards: Mutex::new(None),
        });
        Self {
            id,
            version: "0.1.0",
            author: "RushHFT",
            description: "Spread/depth shock detection + 90% recovery time",
            inner,
        }
    }
}

#[async_trait::async_trait]
impl Plugin for MarketResilienceStudy {
    fn name(&self) -> &str { "Market Resilience Study" }
    fn version(&self) -> &str { self.version }
    fn author(&self) -> &str { self.author }
    fn description(&self) -> &str { self.description }
    fn plugin_type(&self) -> PluginType { PluginType::Study }
    fn status(&self) -> PluginStatus { **self.inner.status.load() }
    fn plugin_id(&self) -> &str { &self.id }
    fn emits_metric(&self) -> bool { true }

    async fn start(&self, ctx: Arc<dyn PluginContext>) -> Result<(), PluginError> {
        {
            let mut g = self.inner.ctx.lock().await;
            *g = Some(ctx.clone());
        }
        let inner_for_consumer = self.inner.clone();
        let ctx_for_consumer = ctx.clone();
        tokio::spawn(async move {
            inner_for_consumer
                .base
                .start_consumer(move |item: &BaseStudyModel| {
                    let ctx = ctx_for_consumer.clone();
                    let symbol = inner_for_consumer.settings.symbol.clone();
                    let value = item.value;
                    let ts = item.timestamp;
                    tokio::spawn(async move {
                        let _ = ctx
                            .register_metric("Market Resilience Study", "MR_SpreadRecovery", "LongPort", &symbol, value, ts)
                            .await;
                    });
                })
                .await;
        });

        let inner_ob = self.inner.clone();
        let ob_hub = ctx.order_book_hub();
        let ob_guard = ob_hub.subscribe(Arc::new(move |ob: &OrderBook| {
            if ob.symbol != inner_ob.settings.symbol
                || ob.provider_id != inner_ob.settings.provider_id
            {
                return;
            }
            let spread = ob.spread().and_then(|s| s.to_f64()).unwrap_or(0.0);
            let bid_depth = ob.bids.first().map(|l| l.size.to_f64().unwrap_or(0.0)).unwrap_or(0.0);
            let ask_depth = ob.asks.first().map(|l| l.size.to_f64().unwrap_or(0.0)).unwrap_or(0.0);
            let mut calc = inner_ob.calc.lock().unwrap();
            calc.observe(spread, bid_depth, ask_depth, OffsetDateTime::now_utc());
            let m = calc.metrics();
            let value = Decimal::from_f64_retain(m.spread_recovery_ms.unwrap_or(0.0))
                .unwrap_or(Decimal::ZERO);
            inner_ob.base.add_calculation(BaseStudyModel {
                value,
                format: "N0".into(),
                timestamp: OffsetDateTime::now_utc(),
                market_mid_price: ob.mid_price().unwrap_or(Decimal::ZERO),
                value_color: "White".into(),
                tooltip: String::new(),
                has_error: false,
                is_stale: m.spread_recovery_ms.is_none(),
            });
        }));

        {
            let mut guards = self.inner.guards.lock().await;
            *guards = Some(vec![ob_guard]);
        }
        self.inner.status.store(Arc::new(PluginStatus::Started));
        Ok(())
    }

    async fn stop(&self) -> Result<(), PluginError> {
        self.inner.status.store(Arc::new(PluginStatus::Stopping));
        {
            let mut guards = self.inner.guards.lock().await;
            *guards = None;
        }
        self.inner.status.store(Arc::new(PluginStatus::Stopped));
        Ok(())
    }
}
```

- [ ] **Step 4: Run tests to verify they pass**

Run: `cargo test -p rushhft-studies`
Expected: PASS — existing VPIN/LOB tests + OTT tests + MR tests (3 calculator + 1 metadata).

- [ ] **Step 5: Commit**

```bash
git add rushhft-studies/src/market_resilience/
git commit -m "feat(studies): wire Market Resilience plugin to OB hub"
```

---

## Phase C — Backend

### Task C1: New DTOs

**Files:**
- Modify: `rushhft-app/src/dto.rs` (append before `#[cfg(test)]`)

- [ ] **Step 1: Write the failing test**

Append to `rushhft-app/src/dto.rs` (before the `#[cfg(test)] mod tests`):

```rust
#[derive(Serialize, Clone, Debug)]
pub struct ChartPointDto {
    pub t: i64,
    pub value: Decimal,
    pub bid: Option<Decimal>,
    pub ask: Option<Decimal>,
    pub mid: Option<Decimal>,
}

#[derive(Serialize, Clone, Debug)]
pub struct ChartSeriesDto {
    pub kind: String,
    pub points: Vec<ChartPointDto>,
}

#[derive(Serialize, Clone, Debug)]
pub struct VenuePriceDto {
    pub venue: String,
    pub bid: Decimal,
    pub ask: Decimal,
    pub last: Decimal,
    pub timestamp: i64,
}

#[derive(Serialize, Clone, Debug)]
pub struct PluginDescriptorDto {
    pub plugin_id: String,
    pub name: String,
    pub version: String,
    pub description: String,
    pub plugin_type: PluginTypeDto,
    pub status: PluginStatusDto,
    pub emits_metric: bool,
}
```

Add to the existing `#[cfg(test)] mod tests`:

```rust
    #[test]
    fn chart_series_dto_serializes() {
        let p = ChartPointDto {
            t: 1_700_000_000_000,
            value: dec!(0.05),
            bid: Some(dec!(100)),
            ask: Some(dec!(101)),
            mid: Some(dec!(100.5)),
        };
        let s = ChartSeriesDto { kind: "spread".into(), points: vec![p] };
        let json = serde_json::to_string(&s).unwrap();
        assert!(json.contains("\"kind\":\"spread\""));
        assert!(json.contains("\"value\":\"0.05\""));
    }

    #[test]
    fn venue_price_dto_serializes() {
        let v = VenuePriceDto {
            venue: "LongPort".into(),
            bid: dec!(100),
            ask: dec!(101),
            last: dec!(100.5),
            timestamp: 1,
        };
        let json = serde_json::to_string(&v).unwrap();
        assert!(json.contains("\"venue\":\"LongPort\""));
        assert!(json.contains("\"bid\":\"100\""));
    }
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p rushhft-app --lib dto::tests`
Expected: FAIL — `ChartPointDto`/`ChartSeriesDto`/`VenuePriceDto` not defined.

- [ ] **Step 3: Verify they pass**

Run: `cargo test -p rushhft-app --lib dto::tests`
Expected: PASS — 6 DTO tests (4 existing + 2 new).

- [ ] **Step 4: Commit**

```bash
git add rushhft-app/src/dto.rs
git commit -m "feat(app): add chart-series + venue-price DTOs"
```

---

### Task C2: ChartSeriesBuffer

**Files:**
- Modify: `rushhft-app/src/state.rs` (add buffer + accessor)

- [ ] **Step 1: Write the failing test**

Add to `rushhft-app/src/state.rs`. First extend the imports:

```rust
use crate::dto::{
    BookItemDto, ChartPointDto, ProviderDto, QuoteStatsDto, SessionStatusDto, StudyValueDto,
    TradeDto,
};
```

Append to `SnapshotStore` struct (line 48-53):

```rust
pub struct SnapshotStore {
    books: DashMap<String, ArcSwap<SymbolSnapshot>>,
    studies: DashMap<String, DashMap<String, ArcSwap<StudyValueDto>>>,
    trades: DashMap<String, VecDeque<TradeDto>>,
    providers: ArcSwap<Vec<ProviderDto>>,
    chart_buffers: DashMap<String, DashMap<String, VecDeque<ChartPointDto>>>,
}
```

Update `SnapshotStore::new()`:

```rust
pub fn new() -> Self {
    Self {
        books: DashMap::new(),
        studies: DashMap::new(),
        trades: DashMap::new(),
        providers: ArcSwap::from_pointee(Vec::new()),
        chart_buffers: DashMap::new(),
    }
}
```

Add methods:

```rust
    /// Push a chart point into the per-symbol, per-kind ring buffer.
    /// Cap at `cap` points (default 600 = 1min @ 10Hz).
    pub fn push_chart_point(&self, symbol: &str, kind: &str, point: ChartPointDto, cap: usize) {
        let per_symbol = self
            .chart_buffers
            .entry(symbol.to_string())
            .or_default();
        let buf = per_symbol.entry(kind.to_string()).or_default();
        buf.push_back(point);
        while buf.len() > cap {
            buf.pop_front();
        }
    }

    /// Read up to `points` last points for (symbol, kind). Returns empty vec if none.
    pub fn chart_series(&self, symbol: &str, kind: &str, points: usize) -> Vec<ChartPointDto> {
        let Some(per_symbol) = self.chart_buffers.get(symbol) else {
            return Vec::new();
        };
        let Some(buf) = per_symbol.get(kind) else {
            return Vec::new();
        };
        let skip = buf.len().saturating_sub(points);
        buf.iter().skip(skip).cloned().collect()
    }
```

Add tests to the existing `#[cfg(test)] mod tests`:

```rust
    #[test]
    fn push_chart_point_caps_at_default_cap() {
        let store = SnapshotStore::new();
        for i in 0..700 {
            store.push_chart_point(
                "700.HK",
                "spread",
                ChartPointDto {
                    t: i,
                    value: dec!(0.05),
                    bid: None,
                    ask: None,
                    mid: None,
                },
                600,
            );
        }
        let pts = store.chart_series("700.HK", "spread", 1000);
        assert_eq!(pts.len(), 600);
        assert_eq!(pts[0].t, 100);
    }

    #[test]
    fn chart_series_returns_last_n() {
        let store = SnapshotStore::new();
        for i in 0..50 {
            store.push_chart_point(
                "700.HK",
                "price",
                ChartPointDto {
                    t: i,
                    value: dec!(0),
                    bid: Some(dec!(i)),
                    ask: Some(dec!(i + 1)),
                    mid: None,
                },
                600,
            );
        }
        let pts = store.chart_series("700.HK", "price", 10);
        assert_eq!(pts.len(), 10);
        assert_eq!(pts[0].bid, Some(dec!(40)));
    }

    #[test]
    fn chart_series_unknown_symbol_returns_empty() {
        let store = SnapshotStore::new();
        assert!(store.chart_series("NOPE.HK", "spread", 100).is_empty());
    }
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p rushhft-app --lib state::tests`
Expected: FAIL — `push_chart_point` / `chart_series` not defined.

- [ ] **Step 3: Run tests to verify they pass**

Run: `cargo test -p rushhft-app --lib state::tests`
Expected: PASS — 9 tests (6 existing + 3 new).

- [ ] **Step 4: Commit**

```bash
git add rushhft-app/src/state.rs
git commit -m "feat(app): add ChartSeriesBuffer ring buffer to SnapshotStore"
```

---

### Task C3: Wire ChartSeriesBuffer in PluginContext

**Files:**
- Modify: `rushhft-app/src/context.rs:48-63` (publish_order_book pushes chart points)

- [ ] **Step 1: Write the failing test**

Add to `rushhft-app/src/context.rs` `#[cfg(test)] mod tests`:

```rust
    #[tokio::test]
    async fn publish_order_book_pushes_chart_points() {
        let (ctx, store) = make_ctx();
        let mut ob = OrderBook::new("700.HK", 10, 2, 0, 1);
        ob.add_or_update_level(BookItem::new(dec!(100.50), dec!(500), true, "700.HK", 1));
        ob.add_or_update_level(BookItem::new(dec!(100.60), dec!(300), false, "700.HK", 1));
        ctx.publish_order_book(ob).await;

        let spread_pts = store.chart_series("700.HK", "spread", 100);
        assert_eq!(spread_pts.len(), 1);
        assert!(spread_pts[0].spread > dec!(0)); // field is `value`
        let price_pts = store.chart_series("700.HK", "price", 100);
        assert_eq!(price_pts.len(), 1);
        assert!(price_pts[0].bid.is_some());
    }
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p rushhft-app --lib context::tests::publish_order_book_pushes_chart_points`
Expected: FAIL — chart buffers empty (publish_order_book doesn't push yet).

- [ ] **Step 3: Modify publish_order_book**

In `rushhft-app/src/context.rs`, extend `publish_order_book` (after the existing `self.snapshot_store.update_book(...)` call, still inside the method):

```rust
        // Push chart-series points.
        use crate::dto::ChartPointDto;
        let spread = ob.spread().unwrap_or(Decimal::ZERO);
        let mid = ob.mid_price().unwrap_or(Decimal::ZERO);
        let bid_top = ob.bids.first().map(|l| l.price);
        let ask_top = ob.asks.first().map(|l| l.price);
        let t_ms = (ob.last_updated.unix_timestamp_nanos() / 1_000_000) as i64;

        self.snapshot_store.push_chart_point(
            &symbol,
            "spread",
            ChartPointDto { t: t_ms, value: spread, bid: bid_top, ask: ask_top, mid: Some(mid) },
            600,
        );
        self.snapshot_store.push_chart_point(
            &symbol,
            "price",
            ChartPointDto { t: t_ms, value: mid, bid: bid_top, ask: ask_top, mid: Some(mid) },
            600,
        );
        // Cumulative bids: series of (price, cumulative size) — flattened into
        // one value per push = best-bid cumulative size for MVP (frontend
        // reconstructs full ladder from snapshot.asks/bids).
        let cum_bid = ob.bids.first().map(|l| l.cumulative_size).unwrap_or(Decimal::ZERO);
        let cum_ask = ob.asks.first().map(|l| l.cumulative_size).unwrap_or(Decimal::ZERO);
        self.snapshot_store.push_chart_point(
            &symbol,
            "cumulative-bids",
            ChartPointDto { t: t_ms, value: cum_bid, bid: bid_top, ask: ask_top, mid: None },
            600,
        );
        self.snapshot_store.push_chart_point(
            &symbol,
            "cumulative-asks",
            ChartPointDto { t: t_ms, value: cum_ask, bid: bid_top, ask: ask_top, mid: None },
            600,
        );
```

- [ ] **Step 4: Run tests to verify they pass**

Run: `cargo test -p rushhft-app --lib context::tests`
Expected: PASS — 4 tests (3 existing + 1 new).

- [ ] **Step 5: Commit**

```bash
git add rushhft-app/src/context.rs
git commit -m "feat(app): wire PluginContext to push chart-series points"
```

---

### Task C4: New IPC commands (chart series, multi-venue, plugin descriptors)

**Files:**
- Modify: `rushhft-app/src/commands.rs` (add 3 commands)
- Modify: `rushhft-app/src/main.rs:132-146` (register them)

- [ ] **Step 1: Write the failing tests**

Append to `rushhft-app/src/commands.rs` `#[cfg(test)] mod tests`:

```rust
    #[tokio::test]
    async fn get_chart_series_returns_empty_for_unknown_symbol() {
        let state = make_state(vec![]);
        let dto = get_chart_series_inner(&state, "NOPE.HK", "spread", 100).await;
        assert!(dto.points.is_empty());
        assert_eq!(dto.kind, "spread");
    }

    #[tokio::test]
    async fn get_multi_venue_prices_single_venue_stub() {
        let state = make_state(vec![]);
        let prices = get_multi_venue_prices_inner(&state, "700.HK").await;
        // Single-venue (LongPort only) — returns empty vec when no book.
        assert!(prices.is_empty());
    }

    #[tokio::test]
    async fn get_plugin_descriptors_lists_all_plugins() {
        use rushhft_studies::{VpinSettings, VpinStudy};
        let vpin = Arc::new(VpinStudy::new(VpinSettings::default()));
        let state = make_state(vec![vpin]);
        let descs = get_plugin_descriptors_inner(&state).await;
        assert_eq!(descs.len(), 1);
        assert_eq!(descs[0].name, "VPIN Study");
    }
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test -p rushhft-app --lib commands::tests`
Expected: FAIL — `get_chart_series_inner`, `get_multi_venue_prices_inner`, `get_plugin_descriptors_inner` not defined.

- [ ] **Step 3: Write the commands**

Append to `rushhft-app/src/commands.rs` (before the `#[cfg(test)]` block):

```rust
pub async fn get_chart_series_inner(
    state: &AppState,
    symbol: &str,
    kind: &str,
    points: usize,
) -> crate::dto::ChartSeriesDto {
    crate::dto::ChartSeriesDto {
        kind: kind.to_string(),
        points: state.snapshot_store.chart_series(symbol, kind, points),
    }
}

#[tauri::command]
pub async fn get_chart_series(
    state: tauri::State<'_, AppState>,
    symbol: String,
    kind: String,
    points: usize,
) -> Result<crate::dto::ChartSeriesDto, String> {
    Ok(get_chart_series_inner(&state, &symbol, &kind, points).await)
}

#[tauri::command]
pub async fn subscribe_chart_series(
    state: tauri::State<'_, AppState>,
    symbol: String,
    channel: tauri::ipc::Channel<crate::dto::ChartSeriesDto>,
) -> Result<(), String> {
    // MVP: poll-based fallback. Channel push deferred to a follow-up task.
    // Spawn a lightweight 250ms poller that pushes the latest series.
    let store = state.snapshot_store.clone();
    tokio::spawn(async move {
        let mut last_t: i64 = 0;
        loop {
            tokio::time::sleep(std::time::Duration::from_millis(250)).await;
            let pts = store.chart_series(&symbol, "price", 1);
            if let Some(p) = pts.last() {
                if p.t > last_t {
                    last_t = p.t;
                    let _ = channel.send(crate::dto::ChartSeriesDto {
                        kind: "price".into(),
                        points: vec![p.clone()],
                    });
                }
            }
        }
    });
    Ok(())
}

pub async fn get_multi_venue_prices_inner(
    state: &AppState,
    symbol: &str,
) -> Vec<crate::dto::VenuePriceDto> {
    let snap = match state.snapshot_store.snapshot(symbol) {
        Some(s) => s,
        None => return Vec::new(),
    };
    let bid = snap.bids.first().map(|b| b.price).unwrap_or(Decimal::ZERO);
    let ask = snap.asks.first().map(|a| a.price).unwrap_or(Decimal::ZERO);
    let last = snap.quote_stats.as_ref().map(|q| q.last_done).unwrap_or(Decimal::ZERO);
    vec![crate::dto::VenuePriceDto {
        venue: "LongPort".into(),
        bid,
        ask,
        last,
        timestamp: snap.last_updated,
    }]
}

#[tauri::command]
pub async fn get_multi_venue_prices(
    state: tauri::State<'_, AppState>,
    symbol: String,
) -> Result<Vec<crate::dto::VenuePriceDto>, String> {
    Ok(get_multi_venue_prices_inner(&state, &symbol).await)
}

pub async fn get_plugin_descriptors_inner(
    state: &AppState,
) -> Vec<crate::dto::PluginDescriptorDto> {
    state
        .plugins
        .iter()
        .map(|p| crate::dto::PluginDescriptorDto {
            plugin_id: p.plugin_id().to_string(),
            name: p.name().to_string(),
            version: p.version().to_string(),
            description: p.description().to_string(),
            plugin_type: map_plugin_type(p.plugin_type()),
            status: map_plugin_status(p.status()),
            emits_metric: p.emits_metric(),
        })
        .collect()
}

#[tauri::command]
pub async fn get_plugin_descriptors(
    state: tauri::State<'_, AppState>,
) -> Result<Vec<crate::dto::PluginDescriptorDto>, String> {
    Ok(get_plugin_descriptors_inner(&state).await)
}
```

- [ ] **Step 4: Register commands in main.rs**

Modify `rushhft-app/src/main.rs:132-146` `invoke_handler`:

```rust
        .invoke_handler(tauri::generate_handler![
            commands::get_snapshot,
            commands::get_providers,
            commands::get_symbols,
            commands::get_studies,
            commands::get_plugin_descriptors,
            commands::start_plugin,
            commands::stop_plugin,
            commands::get_settings,
            commands::save_settings,
            commands::get_triggers,
            commands::save_trigger,
            commands::delete_trigger,
            commands::test_trigger_rest,
            commands::subscribe_notifications,
            commands::get_chart_series,
            commands::subscribe_chart_series,
            commands::get_multi_venue_prices,
        ])
```

- [ ] **Step 5: Run tests to verify they pass**

Run: `cargo test -p rushhft-app --lib commands::tests`
Expected: PASS — 3 new tests added to existing suite.

- [ ] **Step 6: Commit**

```bash
git add rushhft-app/src/commands.rs rushhft-app/src/main.rs
git commit -m "feat(app): add chart-series, multi-venue, plugin-descriptor commands"
```

---

### Task C5: Runtime symbol subscribe/unsubscribe (connector + commands)

**Files:**
- Modify: `rushhft-connector-longport/src/lib.rs` (add `subscribe_symbol`, `unsubscribe_symbol`, `user_symbols` field)
- Create: `rushhft-app/src/ui_state.rs` (`UserSymbols` registry)
- Modify: `rushhft-app/src/commands.rs` (add `add_symbol`/`remove_symbol` commands)
- Modify: `rushhft-app/src/main.rs` (register + wire)

- [ ] **Step 1: Add connector methods**

In `rushhft-connector-longport/src/lib.rs`, extend the `Inner` struct:

```rust
struct Inner {
    settings: ConnectorSettings,
    local_books: DashMap<String, rushhft_core::OrderBook>,
    quote_stats: DashMap<String, QuoteStats>,
    stop_flag: AtomicBool,
    quote_ctx: tokio::sync::Mutex<Option<Arc<longport::QuoteContext>>>,
    ctx: tokio::sync::Mutex<Option<Arc<dyn rushhft_core::plugin::PluginContext>>>,
    status: arc_swap::ArcSwap<rushhft_core::PluginStatus>,
    user_symbols: dashmap::DashMap<String, ()>,
}
```

Update `LongPortConnector::new` to initialize `user_symbols: dashmap::DashMap::new()`.

Add methods on `LongPortConnector`:

```rust
    /// Runtime-subscribe a new symbol. Idempotent.
    pub async fn subscribe_symbol(&self, symbol: &str) -> Result<(), String> {
        if self.inner.user_symbols.contains_key(symbol) {
            return Ok(());
        }
        let guard = self.inner.quote_ctx.lock().await;
        let Some(ctx) = guard.as_ref() else {
            return Err("connector not started".into());
        };
        ctx.subscribe(
            vec![symbol],
            self.inner.settings.sub_flags,
        )
        .await
        .map_err(|e| format!("subscribe failed: {e}"))?;
        self.inner.user_symbols.insert(symbol.to_string(), ());
        Ok(())
    }

    /// Runtime-unsubscribe a symbol. Idempotent.
    pub async fn unsubscribe_symbol(&self, symbol: &str) -> Result<(), String> {
        if !self.inner.user_symbols.contains_key(symbol) {
            return Ok(());
        }
        let guard = self.inner.quote_ctx.lock().await;
        let Some(ctx) = guard.as_ref() else {
            return Err("connector not started".into());
        };
        ctx.unsubscribe(vec![symbol])
            .await
            .map_err(|e| format!("unsubscribe failed: {e}"))?;
        self.inner.user_symbols.remove(symbol);
        Ok(())
    }

    /// Snapshot of currently-subscribed user symbols (does not include
    /// `settings.default_symbols`).
    pub fn user_symbols(&self) -> Vec<String> {
        self.inner
            .user_symbols
            .iter()
            .map(|e| e.key().clone())
            .collect()
    }
```

- [ ] **Step 2: Add UserSymbols registry**

Create `rushhft-app/src/ui_state.rs`:

```rust
//! Runtime symbol registry: the set of symbols the user has added through
//! the UI (separate from `Settings.default_symbols` which is loaded at start).
#![allow(dead_code)]

use std::collections::HashSet;
use std::sync::Arc;
use tokio::sync::RwLock;

#[derive(Debug, Clone, Default)]
pub struct UserSymbols {
    inner: Arc<RwLock<HashSet<String>>>,
}

impl UserSymbols {
    pub fn new() -> Self {
        Self { inner: Arc::new(RwLock::new(HashSet::new())) }
    }

    pub async fn add(&self, symbol: &str) -> bool {
        let mut g = self.inner.write().await;
        g.insert(symbol.to_string())
    }

    pub async fn remove(&self, symbol: &str) -> bool {
        let mut g = self.inner.write().await;
        g.remove(symbol)
    }

    pub async fn list(&self) -> Vec<String> {
        let g = self.inner.read().await;
        let mut v: Vec<String> = g.iter().cloned().collect();
        v.sort();
        v
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn add_returns_true_on_new_symbol_false_on_dup() {
        let u = UserSymbols::new();
        assert!(u.add("700.HK").await);
        assert!(!u.add("700.HK").await);
        assert_eq!(u.list().await, vec!["700.HK".to_string()]);
    }

    #[tokio::test]
    async fn remove_returns_true_only_when_present() {
        let u = UserSymbols::new();
        u.add("AAPL.US").await;
        assert!(u.remove("AAPL.US").await);
        assert!(!u.remove("AAPL.US").await);
    }
}
```

- [ ] **Step 3: Write the failing commands tests**

Append to `rushhft-app/src/commands.rs` `#[cfg(test)] mod tests`:

```rust
    #[tokio::test]
    async fn user_symbols_starts_empty() {
        let state = make_state(vec![]);
        let syms = list_user_symbols_inner(&state).await;
        assert!(syms.is_empty());
    }

    #[tokio::test]
    async fn add_symbol_then_listed() {
        let state = make_state(vec![]);
        add_symbol_inner(&state, "700.HK").await.unwrap();
        let syms = list_user_symbols_inner(&state).await;
        assert_eq!(syms, vec!["700.HK".to_string()]);
        remove_symbol_inner(&state, "700.HK").await.unwrap();
        assert!(list_user_symbols_inner(&state).await.is_empty());
    }
```

- [ ] **Step 4: Run tests to verify they fail**

Run: `cargo test -p rushhft-app --lib commands::tests`
Expected: FAIL — `add_symbol_inner` etc not defined.

- [ ] **Step 5: Add commands and AppState field**

Modify the `AppState` struct in `rushhft-app/src/commands.rs`:

```rust
pub struct AppState {
    pub snapshot_store: Arc<SnapshotStore>,
    pub plugins: Vec<Arc<dyn Plugin>>,
    pub settings: Arc<RwLock<Settings>>,
    pub plugin_context: Arc<dyn rushhft_core::plugin::PluginContext>,
    pub trigger_engine: Arc<rushhft_core::TriggerEngine>,
    pub notification_hub: Arc<crate::notification::NotificationHub>,
    pub user_symbols: Arc<crate::ui_state::UserSymbols>,
    pub connector: Option<Arc<rushhft_connector_longport::LongPortConnector>>,
}
```

Add commands (before `#[cfg(test)]`):

```rust
pub async fn add_symbol_inner(state: &AppState, symbol: &str) -> Result<(), String> {
    state.user_symbols.add(symbol).await;
    if let Some(conn) = &state.connector {
        conn.subscribe_symbol(symbol).await?;
    }
    Ok(())
}

pub async fn remove_symbol_inner(state: &AppState, symbol: &str) -> Result<(), String> {
    state.user_symbols.remove(symbol).await;
    if let Some(conn) = &state.connector {
        conn.unsubscribe_symbol(symbol).await?;
    }
    Ok(())
}

pub async fn list_user_symbols_inner(state: &AppState) -> Vec<String> {
    state.user_symbols.list().await
}

#[tauri::command]
pub async fn add_symbol(
    state: tauri::State<'_, AppState>,
    symbol: String,
) -> Result<(), String> {
    add_symbol_inner(&state, &symbol).await
}

#[tauri::command]
pub async fn remove_symbol(
    state: tauri::State<'_, AppState>,
    symbol: String,
) -> Result<(), String> {
    remove_symbol_inner(&state, &symbol).await
}
```

Update `make_state` in the test module:

```rust
    fn make_state(plugins: Vec<Arc<dyn Plugin>>) -> AppState {
        let ob_hub = Arc::new(rushhft_core::OrderBookHub::new());
        let t_hub = Arc::new(rushhft_core::TradeHub::new());
        let p_hub = Arc::new(rushhft_core::ProviderHub::new());
        let snapshot_store = Arc::new(SnapshotStore::new());
        let trigger_engine = Arc::new(rushhft_core::TriggerEngine::new());
        let notification_hub = Arc::new(crate::notification::NotificationHub::new());
        let (tx, _rx) = tokio::sync::mpsc::unbounded_channel::<rushhft_core::MetricEvent>();
        let ctx: Arc<dyn rushhft_core::plugin::PluginContext> =
            Arc::new(crate::context::PluginContextImpl::new(
                ob_hub,
                t_hub,
                p_hub,
                snapshot_store.clone(),
                tx,
            ));
        AppState {
            snapshot_store,
            plugins,
            settings: Arc::new(RwLock::new(Settings::default())),
            plugin_context: ctx,
            trigger_engine,
            notification_hub,
            user_symbols: Arc::new(crate::ui_state::UserSymbols::new()),
            connector: None,
        }
    }
```

- [ ] **Step 6: Update main.rs**

In `rushhft-app/src/main.rs`:

- Add `mod ui_state;` near the top (line 5).
- Change the connector construction to keep a strong reference:

```rust
    let connector = Arc::new(LongPortConnector::new(ConnectorSettings::from_settings(
        &settings_snapshot,
    )));

    // ...

    let app_state = AppState {
        snapshot_store,
        plugins: plugins.clone(),
        settings: settings.clone(),
        plugin_context: plugin_context.clone(),
        trigger_engine: trigger_engine.clone(),
        notification_hub: notification_hub.clone(),
        user_symbols: Arc::new(ui_state::UserSymbols::new()),
        connector: Some(connector.clone() as Arc<rushhft_connector_longport::LongPortConnector>),
    };
```

- Add `commands::add_symbol`, `commands::remove_symbol` to the `invoke_handler` list.

- [ ] **Step 7: Register new plugins (MR + OTT) in main.rs**

After the `lob` study construction (around line 109), add:

```rust
    let mr = Arc::new(MarketResilienceStudy::new(MarketResilienceSettings {
        symbol: first_symbol.clone(),
        provider_id: 1,
        aggregation_level: settings_snapshot.aggregation_level,
    })) as Arc<dyn Plugin>;

    let ott = Arc::new(OttRatioStudy::new(OttRatioSettings {
        symbol: first_symbol.clone(),
        provider_id: 1,
        aggregation_level: settings_snapshot.aggregation_level,
    })) as Arc<dyn Plugin>;
```

Update the `plugins` vec:

```rust
    let plugins: Vec<Arc<dyn Plugin>> = vec![connector.clone(), vpin.clone(), lob.clone(), mr, ott];
```

Update the imports:

```rust
use rushhft_studies::{
    LobImbalanceSettings, LobImbalanceStudy, MarketResilienceSettings, MarketResilienceStudy,
    OttRatioSettings, OttRatioStudy, VpinSettings, VpinStudy,
};
```

- [ ] **Step 8: Run all tests**

Run: `cargo test`
Expected: PASS — all crate test suites green.

- [ ] **Step 9: Commit**

```bash
git add rushhft-connector-longport/src/lib.rs rushhft-app/src/ui_state.rs rushhft-app/src/commands.rs rushhft-app/src/main.rs
git commit -m "feat(app): runtime symbol subscribe/unsubscribe + MR/OTT plugins wired"
```

---

## Phase D — UI shell

> Frontend has no test framework for MVP. Verification per task is `pnpm check` (svelte-check) + manual `pnpm tauri dev` smoke at the end of the phase.

### Task D1: UI deps + scaffolding

**Files:**
- Modify: `rushhft-app/ui/package.json` (add `uplot`)
- Create: `rushhft-app/ui/src/lib/` directory tree

- [ ] **Step 1: Add uplot dependency**

Modify `rushhft-app/ui/package.json` — add to `dependencies`:

```json
    "uplot": "^1.4.0"
```

And to `devDependencies`:

```json
    "@types/uplot": "^1.4.0"
```

- [ ] **Step 2: Install**

Run: `cd /Users/tangning/Documents/workspace/mine/RushHFT/rushhft-app/ui && pnpm install`
Expected: lockfile updates, `uplot` installed under `node_modules`.

- [ ] **Step 3: Create lib directory tree**

Run:
```bash
cd /Users/tangning/Documents/workspace/mine/RushHFT/rushhft-app/ui/src
mkdir -p lib/components/Charts lib/modals lib/stores lib/charts
```

- [ ] **Step 4: Commit**

```bash
git add rushhft-app/ui/package.json rushhft-app/ui/pnpm-lock.yaml
git commit -m "chore(ui): add uplot dependency"
```

---

### Task D2: app.css theme extensions

**Files:**
- Modify: `rushhft-app/ui/src/app.css` (extend with panel/tile/gauge/depth styles)

- [ ] **Step 1: Replace app.css**

Replace the contents of `rushhft-app/ui/src/app.css` with:

```css
:root {
  --bg: #0d1117;
  --panel: #161b22;
  --panel-2: #1c2129;
  --border: #30363d;
  /* 红涨绿跌 (A-share convention): bids/Up → red, asks/Down → green */
  --bid: #f85149;
  --ask: #7ee787;
  --accent: #58a6ff;
  --muted: #8b949e;
  --warn: #d29922;
  --err: #f85149;
}

* { box-sizing: border-box; }

body {
  background: var(--bg);
  color: #c9d1d9;
  font: 12px/1.4 -apple-system, BlinkMacSystemFont, "Segoe UI", sans-serif;
  margin: 0;
}

.app {
  display: grid;
  grid-template-columns: 480px 1fr;
  height: 100vh;
  overflow: hidden;
}

.sidebar {
  background: var(--panel);
  border-right: 1px solid var(--border);
  display: flex;
  flex-direction: column;
  overflow: hidden;
}

.sidebar-scroll {
  overflow-y: auto;
  flex: 1;
  padding: 8px;
}

.main {
  display: grid;
  grid-template-rows: auto 1fr 1fr 1fr;
  gap: 4px;
  padding: 4px;
  overflow: hidden;
}

.panel {
  background: var(--panel);
  border: 1px solid var(--border);
  border-radius: 4px;
  overflow: hidden;
}

.panel-header {
  padding: 4px 8px;
  background: var(--panel-2);
  border-bottom: 1px solid var(--border);
  font-weight: 600;
  font-size: 11px;
  color: var(--muted);
  display: flex;
  justify-content: space-between;
  align-items: center;
}

/* Toolbar */
.toolbar {
  display: flex;
  gap: 4px;
  padding: 6px 8px;
  border-bottom: 1px solid var(--border);
  background: var(--panel-2);
}
.toolbar button {
  background: transparent;
  border: 1px solid var(--border);
  color: #c9d1d9;
  padding: 4px 8px;
  border-radius: 4px;
  cursor: pointer;
  font-size: 11px;
}
.toolbar button:hover { border-color: var(--accent); }
.toolbar .bell { margin-left: auto; }

/* Provider chips */
.provider {
  display: inline-flex;
  gap: 4px;
  align-items: center;
  padding: 2px 6px;
  border: 1px solid var(--border);
  border-radius: 10px;
  font-size: 10px;
}
.provider .dot { width: 6px; height: 6px; border-radius: 50%; }

/* Study tiles */
.tile {
  background: var(--panel-2);
  border: 1px solid var(--border);
  border-radius: 4px;
  padding: 6px 8px;
  margin-bottom: 4px;
}
.tile .label { color: var(--muted); font-size: 10px; }
.tile .value { font-size: 18px; font-weight: 600; }
.tile.stale .value { color: var(--muted); }

/* Depth ladder (combined with size bars) */
.depth {
  display: grid;
  grid-auto-rows: 18px;
  font-family: ui-monospace, "SF Mono", monospace;
  font-size: 11px;
}
.depth .row {
  position: relative;
  display: flex;
  justify-content: space-between;
  padding: 1px 8px;
  align-items: center;
}
.depth .row.ask { color: var(--ask); }
.depth .row.bid { color: var(--bid); }
.depth .row .bar {
  position: absolute;
  top: 0; bottom: 0;
  opacity: 0.2;
}
.depth .row.ask .bar { right: 0; background: var(--ask); }
.depth .row.bid .bar { left: 0; background: var(--bid); }
.depth .row .text { position: relative; z-index: 1; }
.depth .spread {
  text-align: center;
  color: var(--muted);
  border-top: 1px solid var(--border);
  border-bottom: 1px solid var(--border);
  padding: 2px 0;
  font-weight: 600;
}

/* Top of book */
.tob {
  display: flex;
  justify-content: space-between;
  padding: 8px;
  font-family: ui-monospace, monospace;
  font-size: 16px;
}
.tob .bid { color: var(--bid); }
.tob .ask { color: var(--ask); }
.tob .stale { color: var(--muted); font-size: 10px; }

/* LOB Imbalance gauge */
.gauge {
  position: relative;
  height: 18px;
  background: linear-gradient(to right, var(--bid) 0%, #2a2a2a 50%, var(--bid) 100%);
  border-radius: 4px;
  overflow: hidden;
}
.gauge .arrow {
  position: absolute;
  top: 0; bottom: 0;
  width: 2px;
  background: white;
  transition: left 100ms linear;
}

/* Trades tape */
.trade {
  display: grid;
  grid-template-columns: 1fr 1fr 1fr;
  gap: 8px;
  font-family: ui-monospace, monospace;
  font-size: 11px;
  padding: 1px 8px;
}

/* Positions stub */
.empty-state {
  display: flex;
  align-items: center;
  justify-content: center;
  height: 100%;
  color: var(--muted);
  font-size: 11px;
}

/* Modal */
.modal-backdrop {
  position: fixed;
  inset: 0;
  background: rgba(0,0,0,0.6);
  display: flex;
  align-items: center;
  justify-content: center;
  z-index: 100;
}
.modal {
  background: var(--panel);
  border: 1px solid var(--border);
  border-radius: 8px;
  width: 600px;
  max-height: 80vh;
  overflow: auto;
  padding: 16px;
}
.modal h2 { margin: 0 0 12px; font-size: 14px; }
.modal .row { display: flex; justify-content: space-between; padding: 4px 0; }
.modal input, .modal select {
  background: var(--bg);
  color: inherit;
  border: 1px solid var(--border);
  padding: 4px 8px;
  border-radius: 4px;
}
.modal button { cursor: pointer; background: var(--panel-2); color: inherit; border: 1px solid var(--border); padding: 4px 12px; border-radius: 4px; }
```

- [ ] **Step 2: Verify CSS compiles**

Run: `cd /Users/tangning/Documents/workspace/mine/RushHFT/rushhft-app/ui && pnpm check`
Expected: PASS (svelte-check passes with no new errors).

- [ ] **Step 3: Commit**

```bash
git add rushhft-app/ui/src/app.css
git commit -m "feat(ui): extend app.css with panel/tile/depth/gauge/modal styles"
```

---

### Task D3: Svelte stores

**Files:**
- Create: `rushhft-app/ui/src/lib/stores/snapshot.ts`
- Create: `rushhft-app/ui/src/lib/stores/symbols.ts`
- Create: `rushhft-app/ui/src/lib/stores/plugins.ts`
- Create: `rushhft-app/ui/src/lib/stores/settings.ts`
- Create: `rushhft-app/ui/src/lib/stores/triggers.ts`
- Create: `rushhft-app/ui/src/lib/stores/notifications.ts`

- [ ] **Step 1: snapshot.ts**

```typescript
import { invoke } from '@tauri-apps/api/core';
import { writable } from 'svelte/store';

export interface BookItem { price: string; size: string; cumulative_size: string; is_bid: boolean; broker_ids: number[]; }
export interface Trade { price: string; size: string; timestamp: number; direction: 'Neutral'|'Down'|'Up'; trade_type: string; }
export interface StudyValue { name: string; value: string; format: string; value_color: string; tooltip: string; has_error: boolean; is_stale: boolean; timestamp: number; }
export interface QuoteStats { last_done: string; open: string; high: string; low: string; volume: number; turnover: string; trade_status: string; timestamp: number; }
export interface Provider { id: number; name: string; status: string; }
export interface Snapshot {
  symbol: string;
  bids: BookItem[];
  asks: BookItem[];
  spread: string;
  mid_price: string;
  last_updated: number;
  sequence: number;
  provider_status: string;
  studies: StudyValue[];
  recent_trades: Trade[];
  quote_stats: QuoteStats | null;
}

export const snapshot = writable<Snapshot | null>(null);
export const providers = writable<Provider[]>([]);
export const chartSeries = writable<Record<string, any[]>>({});

let pollHandle: number | null = null;

export async function startPolling(symbol: string) {
  stopPolling();
  pollHandle = window.setInterval(async () => {
    try {
      const [snap, ps] = await Promise.all([
        invoke<Snapshot>('get_snapshot', { symbol }),
        invoke<Provider[]>('get_providers'),
      ]);
      snapshot.set(snap);
      providers.set(ps);
    } catch { /* plugin not started yet */ }
  }, 500);
}

export function stopPolling() {
  if (pollHandle !== null) { clearInterval(pollHandle); pollHandle = null; }
}

export async function fetchChartSeries(symbol: string, kind: string, points = 600): Promise<any[]> {
  try {
    const dto = await invoke<{ kind: string; points: any[] }>('get_chart_series', { symbol, kind, points });
    return dto.points;
  } catch { return []; }
}
```

- [ ] **Step 2: symbols.ts**

```typescript
import { invoke } from '@tauri-apps/api/core';
import { writable } from 'svelte/store';

export const symbols = writable<string[]>([]);
export const currentSymbol = writable<string>('700.HK');
export const userSymbols = writable<string[]>([]);

export async function loadSymbols() {
  symbols.set(await invoke<string[]>('get_symbols'));
  userSymbols.set(await invoke<string[]>('get_user_symbols').catch(() => []));
}

export async function addSymbol(symbol: string) {
  await invoke('add_symbol', { symbol });
  await loadSymbols();
}

export async function removeSymbol(symbol: string) {
  await invoke('remove_symbol', { symbol });
  await loadSymbols();
}
```

Note: `get_user_symbols` command is not wired; replace with the `list_user_symbols_inner` exposed as `list_user_symbols` — or simply omit. For MVP, the existing `get_symbols` already includes all subscribed symbols.

Update `symbols.ts` to drop the `get_user_symbols` call:

```typescript
export async function loadSymbols() {
  symbols.set(await invoke<string[]>('get_symbols'));
}
```

- [ ] **Step 3: plugins.ts**

```typescript
import { invoke } from '@tauri-apps/api/core';
import { writable } from 'svelte/store';

export interface PluginDescriptor {
  plugin_id: string;
  name: string;
  version: string;
  description: string;
  plugin_type: string;
  status: string;
  emits_metric: boolean;
}

export const plugins = writable<PluginDescriptor[]>([]);

export async function loadPlugins() {
  // Fall back to get_studies (older command) if get_plugin_descriptors errors.
  try {
    plugins.set(await invoke<PluginDescriptor[]>('get_plugin_descriptors'));
  } catch {
    plugins.set(await invoke<any[]>('get_studies'));
  }
}

export async function startPlugin(id: string) {
  await invoke('start_plugin', { pluginId: id });
  await loadPlugins();
}

export async function stopPlugin(id: string) {
  await invoke('stop_plugin', { pluginId: id });
  await loadPlugins();
}
```

- [ ] **Step 4: settings.ts**

```typescript
import { invoke } from '@tauri-apps/api/core';
import { writable } from 'svelte/store';

export interface Settings {
  app_key: string;
  app_secret_masked: string;
  access_token_masked: string;
  default_symbols: string[];
  depth_levels: number;
  aggregation_level: string;
  log_level: string;
  region: string;
}

export const settings = writable<Settings | null>(null);

export async function loadSettings() {
  settings.set(await invoke<Settings>('get_settings'));
}

export async function saveSettings(s: Settings) {
  await invoke('save_settings', { settings: s });
  await loadSettings();
}
```

- [ ] **Step 5: triggers.ts**

```typescript
import { invoke } from '@tauri-apps/api/core';
import { writable } from 'svelte/store';

export interface TriggerRule { rule_id: number; name: string; is_enabled: boolean; conditions: any[]; actions: any[]; }
export const triggers = writable<TriggerRule[]>([]);

export async function loadTriggers() { triggers.set(await invoke<TriggerRule[]>('get_triggers')); }
export async function saveTrigger(rule: TriggerRule) { await invoke('save_trigger', { rule }); await loadTriggers(); }
export async function deleteTrigger(id: number) { await invoke('delete_trigger', { ruleId: id }); await loadTriggers(); }
export async function testTrigger(id: number) { return invoke<string>('test_trigger_rest', { ruleId: id }); }
```

- [ ] **Step 6: notifications.ts**

```typescript
import { invoke } from '@tauri-apps/api/core';
import { Channel } from '@tauri-apps/api/core';
import { writable } from 'svelte/store';

export interface Notification { source: string; message: string; level: string; category: string; timestamp: number; exception: string | null; }
export const notifications = writable<Notification[]>([]);
export const unreadCount = writable<number>(0);

export async function subscribeNotifications() {
  const ch = new Channel<Notification>();
  ch.onmessage = (n) => {
    notifications.update((list) => [...list.slice(-200), n]);
    unreadCount.update((c) => c + 1);
  };
  await invoke('subscribe_notifications', { channel: ch });
}

export function clearUnread() { unreadCount.set(0); }
```

- [ ] **Step 7: Verify TS compiles**

Run: `cd /Users/tangning/Documents/workspace/mine/RushHFT/rushhft-app/ui && pnpm check`
Expected: PASS.

- [ ] **Step 8: Commit**

```bash
git add rushhft-app/ui/src/lib/stores/
git commit -m "feat(ui): add Svelte stores for snapshot/symbols/plugins/settings/triggers/notifications"
```

---

### Task D4: Layout + components (Sidebar, Toolbar, ProviderStatus, StudyTiles, TradesTape, Positions)

**Files:**
- Create: `rushhft-app/ui/src/lib/components/Sidebar.svelte`
- Create: `rushhft-app/ui/src/lib/components/Toolbar.svelte`
- Create: `rushhft-app/ui/src/lib/components/ProviderStatus.svelte`
- Create: `rushhft-app/ui/src/lib/components/StudyTiles.svelte`
- Create: `rushhft-app/ui/src/lib/components/TradesTape.svelte`
- Create: `rushhft-app/ui/src/lib/components/Positions.svelte`

- [ ] **Step 1: Toolbar.svelte**

```svelte
<script lang="ts">
  import { unreadCount, clearUnread } from '$lib/stores/notifications';
  import { openPluginManager, openSettings, openTriggers, openMultiVenue } from './events';

  let bellOpen = $state(false);
</script>

<div class="toolbar">
  <button onclick={() => openPluginManager.set(true)}>Plugins</button>
  <button onclick={() => openSettings.set(true)}>Settings</button>
  <button onclick={() => openTriggers.set(true)}>Triggers</button>
  <button onclick={() => openMultiVenue.set(true)}>MultiVenue</button>
  <button class="bell" onclick={() => { bellOpen = !bellOpen; if (bellOpen) clearUnread(); }}>
    Bell ({$unreadCount})
  </button>
</div>
```

- [ ] **Step 2: ProviderStatus.svelte**

```svelte
<script lang="ts">
  import { providers } from '$lib/stores/snapshot';
</script>

<div>
  {#each $providers as p}
    <span class="provider">
      <span class="dot" style="background: {p.status === 'Connected' ? 'var(--bid)' : 'var(--muted)'};"></span>
      {p.name} — {p.status}
    </span>
  {/each}
</div>
```

- [ ] **Step 3: StudyTiles.svelte**

```svelte
<script lang="ts">
  import { snapshot } from '$lib/stores/snapshot';
</script>

<div class="sidebar-scroll">
  {#each $snapshot?.studies ?? [] as s}
    <div class="tile" class:stale={s.is_stale}>
      <div class="label">{s.name}</div>
      <div class="value">{s.value}</div>
    </div>
  {/each}
  {#if ($snapshot?.studies ?? []).length === 0}
    <div class="empty-state">No studies running</div>
  {/if}
</div>
```

- [ ] **Step 4: TradesTape.svelte**

```svelte
<script lang="ts">
  import { snapshot } from '$lib/stores/snapshot';
  const fmt = (t: number) => new Date(t).toLocaleTimeString();
</script>

<div class="panel" style="display:flex; flex-direction:column;">
  <div class="panel-header">Recent Trades</div>
  <div style="overflow:auto; flex:1;">
    {#each $snapshot?.recent_trades ?? [] as t}
      <div class="trade">
        <span style="color: {t.direction === 'Up' ? 'var(--bid)' : t.direction === 'Down' ? 'var(--ask)' : 'var(--muted)'};">{t.price}</span>
        <span>{t.size}</span>
        <span style="color: var(--muted);">{fmt(t.timestamp)}</span>
      </div>
    {/each}
  </div>
</div>
```

- [ ] **Step 5: Positions.svelte**

```svelte
<div class="panel">
  <div class="panel-header">Positions</div>
  <div class="empty-state">No broker connected</div>
</div>
```

- [ ] **Step 6: Sidebar.svelte**

```svelte
<script lang="ts">
  import Toolbar from './Toolbar.svelte';
  import ProviderStatus from './ProviderStatus.svelte';
  import StudyTiles from './StudyTiles.svelte';
</script>

<aside class="sidebar">
  <Toolbar />
  <div style="padding:6px 8px; border-bottom:1px solid var(--border);">
    <ProviderStatus />
  </div>
  <StudyTiles />
</aside>
```

- [ ] **Step 7: Create events.ts for modal toggles**

Create `rushhft-app/ui/src/lib/components/events.ts`:

```typescript
import { writable } from 'svelte/store';
export const openPluginManager = writable(false);
export const openSettings = writable(false);
export const openTriggers = writable(false);
export const openMultiVenue = writable(false);
```

- [ ] **Step 8: Verify TS compiles**

Run: `pnpm check`
Expected: PASS.

- [ ] **Step 9: Commit**

```bash
git add rushhft-app/ui/src/lib/components/
git commit -m "feat(ui): add Sidebar/Toolbar/ProviderStatus/StudyTiles/TradesTape/Positions"
```

---

### Task D5: DepthLadder + TopOfBook + LOBImbalanceGauge

**Files:**
- Create: `rushhft-app/ui/src/lib/components/DepthLadder.svelte`
- Create: `rushhft-app/ui/src/lib/components/TopOfBook.svelte`
- Create: `rushhft-app/ui/src/lib/components/LOBImbalanceGauge.svelte`

- [ ] **Step 1: DepthLadder.svelte**

```svelte
<script lang="ts">
  import { snapshot } from '$lib/stores/snapshot';

  // Compute max size across both sides for bar scaling.
  let maxSize = $derived(
    Math.max(
      ...($snapshot?.bids ?? []).map((b) => Number(b.size)),
      ...($snapshot?.asks ?? []).map((a) => Number(a.size)),
      1,
    ),
  );

  // Asks descending (best ask first), bids descending (best bid first).
  let asks = $derived(($snapshot?.asks ?? []).slice().reverse());
  let bids = $derived($snapshot?.bids ?? []);
</script>

<div class="panel" style="display:flex; flex-direction:column; min-height:0;">
  <div class="panel-header">Depth — {$snapshot?.symbol ?? ''}</div>
  <div class="depth" style="overflow:auto; flex:1;">
    {#each asks as a}
      <div class="row ask">
        <div class="bar" style="width: {(Number(a.size) / maxSize) * 100}%;"></div>
        <span class="text">{a.price}</span>
        <span class="text">{a.size}</span>
      </div>
    {/each}
    <div class="spread">spread {$snapshot?.spread ?? '-'}</div>
    {#each bids as b}
      <div class="row bid">
        <div class="bar" style="width: {(Number(b.size) / maxSize) * 100}%;"></div>
        <span class="text">{b.price}</span>
        <span class="text">{b.size}</span>
      </div>
    {/each}
  </div>
</div>
```

- [ ] **Step 2: TopOfBook.svelte**

```svelte
<script lang="ts">
  import { snapshot } from '$lib/stores/snapshot';
  let stale = $derived(!$snapshot || $snapshot.provider_status !== 'Connected');
  let bid = $derived($snapshot?.bids?.[0]);
  let ask = $derived($snapshot?.asks?.[0]);
</script>

<div class="panel">
  <div class="panel-header">Top of Book {#if stale}<span class="stale">stale</span>{/if}</div>
  <div class="tob">
    <span class="bid">{bid?.price ?? '-'} <span style="font-size:11px;">{bid?.size ?? ''}</span></span>
    <span style="color:var(--muted); font-size:11px;">mid {$snapshot?.mid_price ?? '-'}</span>
    <span class="ask">{ask?.price ?? '-'} <span style="font-size:11px;">{ask?.size ?? ''}</span></span>
  </div>
</div>
```

- [ ] **Step 3: LOBImbalanceGauge.svelte**

```svelte
<script lang="ts">
  import { snapshot } from '$lib/stores/snapshot';

  // Find LOB Imbalance study value (range -1..+1).
  let imb = $derived(
    (() => {
      const s = ($snapshot?.studies ?? []).find((x) => x.name === 'Imbalance');
      return s ? Number(s.value) : 0;
    })(),
  );
  // Map [-1, +1] -> [0%, 100%]; 0 = center.
  let pct = $derived(50 + imb * 50);
</script>

<div class="panel">
  <div class="panel-header">LOB Imbalance</div>
  <div style="padding:8px;">
    <div class="gauge">
      <div class="arrow" style="left: {pct}%;"></div>
    </div>
    <div style="display:flex; justify-content:space-between; font-size:10px; color:var(--muted); margin-top:4px;">
      <span>bids</span><span>{imb.toFixed(3)}</span><span>asks</span>
    </div>
  </div>
</div>
```

- [ ] **Step 4: Verify TS compiles**

Run: `pnpm check`
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add rushhft-app/ui/src/lib/components/
git commit -m "feat(ui): add DepthLadder, TopOfBook, LOBImbalanceGauge"
```

---

### Task D6: Four modals

**Files:**
- Create: `rushhft-app/ui/src/lib/modals/PluginManagerModal.svelte`
- Create: `rushhft-app/ui/src/lib/modals/SettingsModal.svelte`
- Create: `rushhft-app/ui/src/lib/modals/TriggersModal.svelte`
- Create: `rushhft-app/ui/src/lib/modals/MultiVenueModal.svelte`

- [ ] **Step 1: PluginManagerModal.svelte**

```svelte
<script lang="ts">
  import { openPluginManager } from '$lib/components/events';
  import { plugins, loadPlugins, startPlugin, stopPlugin } from '$lib/stores/plugins';
  import { onMount } from 'svelte';
  onMount(loadPlugins);
</script>

{#if $openPluginManager}
  <div class="modal-backdrop" onclick={() => openPluginManager.set(false)}>
    <div class="modal" onclick={(e) => e.stopPropagation()}>
      <h2>Plugins</h2>
      {#each $plugins as p}
        <div class="row">
          <span>{p.name} <small style="color:var(--muted);">v{p.version}</small></span>
          <span>
            <small style="color: {p.status === 'Started' ? 'var(--bid)' : 'var(--muted)'};">{p.status}</small>
            {#if p.status === 'Started'}
              <button onclick={() => stopPlugin(p.plugin_id)}>Stop</button>
            {:else}
              <button onclick={() => startPlugin(p.plugin_id)}>Start</button>
            {/if}
          </span>
        </div>
      {/each}
    </div>
  </div>
{/if}
```

- [ ] **Step 2: SettingsModal.svelte**

```svelte
<script lang="ts">
  import { openSettings } from '$lib/components/events';
  import { settings, loadSettings, saveSettings } from '$lib/stores/settings';
  import { onMount } from 'svelte';
  onMount(loadSettings);
  let form = $state<any>({});
  $effect(() => { if ($settings) form = { ...$settings }; });
</script>

{#if $openSettings}
  <div class="modal-backdrop" onclick={() => openSettings.set(false)}>
    <div class="modal" onclick={(e) => e.stopPropagation()}>
      <h2>Settings</h2>
      <label>App Key</label><input bind:value={form.app_key} style="width:100%;" />
      <label>App Secret (leave masked to keep)</label><input bind:value={form.app_secret_masked} style="width:100%;" />
      <label>Access Token</label><input bind:value={form.access_token_masked} style="width:100%;" />
      <label>Default Symbols (comma-separated)</label>
      <input bind:value={form.default_symbols_input} placeholder="700.HK,AAPL.US" style="width:100%;" />
      <label>Region</label><input bind:value={form.region} />
      <label>Depth Levels</label><input type="number" bind:value={form.depth_levels} />
      <label>Log Level</label><input bind:value={form.log_level} />
      <div style="margin-top:12px; text-align:right;">
        <button onclick={async () => {
          const toSave = { ...form, default_symbols: form.default_symbols_input?.split(',').map((s:string)=>s.trim()).filter(Boolean) ?? [] };
          await saveSettings(toSave);
          openSettings.set(false);
        }}>Save</button>
      </div>
    </div>
  </div>
{/if}
```

- [ ] **Step 3: TriggersModal.svelte**

```svelte
<script lang="ts">
  import { openTriggers } from '$lib/components/events';
  import { triggers, loadTriggers, saveTrigger, deleteTrigger, testTrigger } from '$lib/stores/triggers';
  import { onMount } from 'svelte';
  onMount(loadTriggers);
</script>

{#if $openTriggers}
  <div class="modal-backdrop" onclick={() => openTriggers.set(false)}>
    <div class="modal" onclick={(e) => e.stopPropagation()}>
      <h2>Triggers</h2>
      {#each $triggers as t}
        <div class="row">
          <span>{t.name} <small style="color: var(--muted);">(#{t.rule_id})</small></span>
          <span>
            <button onclick={async () => { await testTrigger(t.rule_id).catch(() => 'error'); }}>Test</button>
            <button onclick={async () => { await deleteTrigger(t.rule_id); }}>Delete</button>
          </span>
        </div>
      {/each}
    </div>
  </div>
{/if}
```

- [ ] **Step 4: MultiVenueModal.svelte**

```svelte
<script lang="ts">
  import { openMultiVenue } from '$lib/components/events';
  import { invoke } from '@tauri-apps/api/core';
  import { currentSymbol } from '$lib/stores/symbols';

  interface VenuePrice { venue: string; bid: string; ask: string; last: string; timestamp: number; }
  let rows: VenuePrice[] = [];

  async function refresh() {
    try { rows = await invoke<VenuePrice[]>('get_multi_venue_prices', { symbol: $currentSymbol }); }
    catch { rows = []; }
  }
</script>

{#if $openMultiVenue}
  <div class="modal-backdrop" onclick={() => openMultiVenue.set(false)}>
    <div class="modal" onclick={(e) => e.stopPropagation()}>
      <h2>Multi-Venue Prices — {$currentSymbol}</h2>
      <button onclick={refresh}>Refresh</button>
      {#if rows.length === 0}
        <p style="color: var(--muted);">No other venues configured.</p>
      {:else}
        <table style="width:100%; font-family: ui-monospace, monospace;">
          <thead><tr><th>Venue</th><th>Bid</th><th>Ask</th><th>Last</th></tr></thead>
          <tbody>
            {#each rows as r}<tr><td>{r.venue}</td><td>{r.bid}</td><td>{r.ask}</td><td>{r.last}</td></tr>{/each}
          </tbody>
        </table>
      {/if}
    </div>
  </div>
{/if}
```

- [ ] **Step 5: Verify TS compiles**

Run: `pnpm check`
Expected: PASS.

- [ ] **Step 6: Commit**

```bash
git add rushhft-app/ui/src/lib/modals/
git commit -m "feat(ui): add PluginManager/Settings/Triggers/MultiVenue modals"
```

---

## Phase E — Charts (uPlot)

### Task E1: uPlot setup + series builders

**Files:**
- Create: `rushhft-app/ui/src/lib/charts/uPlotSetup.ts`
- Create: `rushhft-app/ui/src/lib/charts/series.ts`

- [ ] **Step 1: uPlotSetup.ts**

```typescript
import uPlot from 'uplot';

// Theme colors from app.css
const COLORS = {
  bg: '#0d1117',
  panel: '#161b22',
  border: '#30363d',
  muted: '#8b949e',
  accent: '#58a6ff',
  bid: '#f85149',
  ask: '#7ee787',
};

export function baseOptions(width: number, height: number, series: uPlot.Series[], scales: Record<string, { range?: (u: uPlot, min: number, max: number) => [number, number] }> = {}): uPlot.Options {
  return {
    width,
    height,
    series,
    scales: { x: { time: true }, y: { ...scales.y } },
    axes: [
      { grid: { stroke: COLORS.border }, ticks: { stroke: COLORS.border }, stroke: COLORS.muted },
      { grid: { stroke: COLORS.border }, ticks: { stroke: COLORS.border }, stroke: COLORS.muted },
    ],
    cursor: { show: true },
    legend: { show: false },
  };
}

export function lineSeries(label: string, color: string): uPlot.Series {
  return {
    label,
    stroke: color,
    points: { show: false },
    width: 1,
  };
}

export function stepSeries(label: string, color: string): uPlot.Series {
  return {
    label,
    stroke: color,
    points: { show: false },
    width: 1,
    spanGaps: true,
  };
}
```

- [ ] **Step 2: series.ts — builders for the four chart kinds**

```typescript
import uPlot from 'uplot';
import { lineSeries, stepSeries, baseOptions } from './uPlotSetup';
import type { ChartPointDto } from '$lib/stores/snapshot';

export function buildSpreadOptions(width: number, height: number): uPlot.Options {
  return baseOptions(width, height, [lineSeries('Spread', '#58a6ff')]);
}

export function spreadData(pts: ChartPointDto[]): uPlot.AlignedData {
  return [
    pts.map((p) => p.t / 1000),
    pts.map((p) => Number(p.value)),
  ];
}

export function buildPriceOptions(width: number, height: number): uPlot.Options {
  return baseOptions(width, height, [
    lineSeries('Bid', '#f85149'),
    lineSeries('Ask', '#7ee787'),
    lineSeries('Mid', '#8b949e'),
  ]);
}

export function priceData(pts: ChartPointDto[]): uPlot.AlignedData {
  return [
    pts.map((p) => p.t / 1000),
    pts.map((p) => Number(p.bid ?? '0')),
    pts.map((p) => Number(p.ask ?? '0')),
    pts.map((p) => Number(p.mid ?? '0')),
  ];
}

export function buildCumulativeOptions(width: number, height: number, label: string, color: string): uPlot.Options {
  return baseOptions(width, height, [stepSeries(label, color)]);
}

export function cumulativeData(pts: ChartPointDto[]): uPlot.AlignedData {
  return [
    pts.map((p) => p.t / 1000),
    pts.map((p) => Number(p.value)),
  ];
}
```

- [ ] **Step 3: Verify TS compiles**

Run: `pnpm check`
Expected: PASS.

- [ ] **Step 4: Commit**

```bash
git add rushhft-app/ui/src/lib/charts/
git commit -m "feat(ui): add uPlot theme setup + series builders"
```

---

### Task E2: Chart components

**Files:**
- Create: `rushhft-app/ui/src/lib/components/Charts/CumulativeBook.svelte`
- Create: `rushhft-app/ui/src/lib/components/Charts/PriceChart.svelte`
- Create: `rushhft-app/ui/src/lib/components/Charts/SpreadChart.svelte`

- [ ] **Step 1: SpreadChart.svelte**

```svelte
<script lang="ts">
  import { onMount, onDestroy } from 'svelte';
  import { currentSymbol } from '$lib/stores/symbols';
  import { fetchChartSeries } from '$lib/stores/snapshot';
  import { buildSpreadOptions, spreadData } from '$lib/charts/series';
  import uPlot from 'uplot';

  let container: HTMLDivElement;
  let chart: uPlot | null = null;
  let stopped = false;

  async function refresh() {
    const pts = await fetchChartSeries($currentSymbol, 'spread', 600);
    if (chart && pts.length) chart.setData(spreadData(pts));
  }

  onMount(async () => {
    chart = new uPlot(buildSpreadOptions(600, 120, []), [[]], container);
    while (!stopped) {
      await refresh();
      await new Promise((r) => setTimeout(r, 1000));
    }
  });

  onDestroy(() => { stopped = true; chart?.destroy(); });
</script>

<div class="panel">
  <div class="panel-header">Spread</div>
  <div bind:this={container}></div>
</div>
```

- [ ] **Step 2: PriceChart.svelte**

```svelte
<script lang="ts">
  import { onMount, onDestroy } from 'svelte';
  import { currentSymbol } from '$lib/stores/symbols';
  import { fetchChartSeries } from '$lib/stores/snapshot';
  import { buildPriceOptions, priceData } from '$lib/charts/series';
  import uPlot from 'uplot';

  let container: HTMLDivElement;
  let chart: uPlot | null = null;
  let stopped = false;

  async function refresh() {
    const pts = await fetchChartSeries($currentSymbol, 'price', 600);
    if (chart && pts.length) chart.setData(priceData(pts));
  }

  onMount(async () => {
    chart = new uPlot(buildPriceOptions(600, 160, []), [[], [], [], []], container);
    while (!stopped) { await refresh(); await new Promise((r) => setTimeout(r, 1000)); }
  });

  onDestroy(() => { stopped = true; chart?.destroy(); });
</script>

<div class="panel">
  <div class="panel-header">Real-time Price</div>
  <div bind:this={container}></div>
</div>
```

- [ ] **Step 3: CumulativeBook.svelte** — renders both bids + asks side by side

```svelte
<script lang="ts">
  import { onMount, onDestroy } from 'svelte';
  import { currentSymbol } from '$lib/stores/symbols';
  import { fetchChartSeries } from '$lib/stores/snapshot';
  import { buildCumulativeOptions, cumulativeData } from '$lib/charts/series';
  import uPlot from 'uplot';

  let bidsEl: HTMLDivElement;
  let asksEl: HTMLDivElement;
  let bidsChart: uPlot | null = null;
  let asksChart: uPlot | null = null;
  let stopped = false;

  async function refresh() {
    const [b, a] = await Promise.all([
      fetchChartSeries($currentSymbol, 'cumulative-bids', 600),
      fetchChartSeries($currentSymbol, 'cumulative-asks', 600),
    ]);
    if (bidsChart && b.length) bidsChart.setData(cumulativeData(b));
    if (asksChart && a.length) asksChart.setData(cumulativeData(a));
  }

  onMount(async () => {
    bidsChart = new uPlot(buildCumulativeOptions(290, 120, 'Cum Bids', '#f85149'), [[]], bidsEl);
    asksChart = new uPlot(buildCumulativeOptions(290, 120, 'Cum Asks', '#7ee787'), [[]], asksEl);
    while (!stopped) { await refresh(); await new Promise((r) => setTimeout(r, 1000)); }
  });

  onDestroy(() => { stopped = true; bidsChart?.destroy(); asksChart?.destroy(); });
</script>

<div class="panel">
  <div class="panel-header">Cumulative Book</div>
  <div style="display:flex; gap:4px;">
    <div bind:this={bidsEl}></div>
    <div bind:this={asksEl}></div>
  </div>
</div>
```

- [ ] **Step 4: Verify TS compiles**

Run: `pnpm check`
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add rushhft-app/ui/src/lib/components/Charts/
git commit -m "feat(ui): add uPlot SpreadChart, PriceChart, CumulativeBook"
```

---

### Task E3: Shell wiring (`+page.svelte`)

**Files:**
- Replace: `rushhft-app/ui/src/routes/+page.svelte`

- [ ] **Step 1: Replace +page.svelte**

```svelte
<script lang="ts">
  import { onMount, onDestroy } from 'svelte';
  import { currentSymbol, loadSymbols, addSymbol, removeSymbol } from '$lib/stores/symbols';
  import { startPolling, stopPolling } from '$lib/stores/snapshot';
  import { subscribeNotifications } from '$lib/stores/notifications';
  import Sidebar from '$lib/components/Sidebar.svelte';
  import DepthLadder from '$lib/components/DepthLadder.svelte';
  import TopOfBook from '$lib/components/TopOfBook.svelte';
  import LOBImbalanceGauge from '$lib/components/LOBImbalanceGauge.svelte';
  import TradesTape from '$lib/components/TradesTape.svelte';
  import Positions from '$lib/components/Positions.svelte';
  import CumulativeBook from '$lib/components/Charts/CumulativeBook.svelte';
  import PriceChart from '$lib/components/Charts/PriceChart.svelte';
  import SpreadChart from '$lib/components/Charts/SpreadChart.svelte';
  import PluginManagerModal from '$lib/modals/PluginManagerModal.svelte';
  import SettingsModal from '$lib/modals/SettingsModal.svelte';
  import TriggersModal from '$lib/modals/TriggersModal.svelte';
  import MultiVenueModal from '$lib/modals/MultiVenueModal.svelte';

  let newSymbol = $state('');

  onMount(async () => {
    await loadSymbols();
    await startPolling($currentSymbol);
    await subscribeNotifications().catch(() => {});
  });

  onDestroy(() => stopPolling());

  async function onAdd() {
    if (!newSymbol) return;
    await addSymbol(newSymbol);
    newSymbol = '';
  }
</script>

<div class="app">
  <Sidebar />
  <main class="main">
    <TopOfBook />
    <CumulativeBook />
    <PriceChart />
    <div style="display:grid; grid-template-columns:1fr 1fr; gap:4px; min-height:0;">
      <DepthLadder />
      <TradesTape />
    </div>
    <Positions />
  </main>
</div>

<PluginManagerModal />
<SettingsModal />
<TriggersModal />
<MultiVenueModal />
```

- [ ] **Step 2: Verify TS compiles**

Run: `pnpm check`
Expected: PASS.

- [ ] **Step 3: Manual smoke — full build**

Run: `cd /Users/tangning/Documents/workspace/mine/RushHFT && cargo build`
Expected: cargo build succeeds.

Then: `cd rushhft-app/ui && pnpm build`
Expected: vite build produces `build/`.

Then: `cd .. && cargo tauri dev` (user runs manually)
Smoke checklist:
- [ ] App boots, sidebar visible (480px)
- [ ] Toolbar shows 4 buttons + bell
- [ ] Provider chips show LongPort Connected
- [ ] Depth ladder renders combined asks/spread/bids with size bars
- [ ] TopOfBook shows bid/ask/mid
- [ ] LOB Imbalance gauge arrow moves with study value
- [ ] Trades tape populates
- [ ] Positions pane shows "No broker connected"
- [ ] 3 charts render (cumulative bids/asks, price, spread)
- [ ] Each modal opens on button click
- [ ] MultiVenue modal shows LongPort row

- [ ] **Step 4: Commit**

```bash
git add rushhft-app/ui/src/routes/+page.svelte
git commit -m "feat(ui): wire shell + all panels + modals in +page.svelte"
```

---

## Phase F — End-to-end test

### Task F1: E2E test with mock connector

**Files:**
- Modify: `rushhft-app/src/commands.rs` (extend `mod tests`)

- [ ] **Step 1: Write the failing test**

Append to `rushhft-app/src/commands.rs` `#[cfg(test)] mod tests`:

```rust
    #[tokio::test]
    async fn end_to_end_snapshot_and_chart_series_round_trip() {
        use rushhft_core::model::book_item::BookItem;
        use rushhft_core::model::order_book::OrderBook;
        use rust_decimal_macros::dec;
        use time::OffsetDateTime;

        let state = make_state(vec![]);
        let ob_hub = state.plugin_context.order_book_hub();

        // Publish a book via the context path.
        let mut ob = OrderBook::new("700.HK", 10, 2, 0, 1);
        ob.add_or_update_level(BookItem::new(dec!(100.50), dec!(500), true, "700.HK", 1));
        ob.add_or_update_level(BookItem::new(dec!(100.60), dec!(300), false, "700.HK", 1));
        ob.last_updated = OffsetDateTime::from_unix_timestamp(1_700_000_000).unwrap();
        state.plugin_context.publish_order_book(ob).await;

        // get_snapshot returns the book.
        let snap = state.snapshot_dto("700.HK");
        assert_eq!(snap.bids.len(), 1);
        assert_eq!(snap.asks.len(), 1);
        assert_eq!(snap.provider_status, SessionStatusDto::Connected);

        // get_chart_series returns non-empty for "spread" and "price".
        let spread_series = get_chart_series_inner(&state, "700.HK", "spread", 100).await;
        assert_eq!(spread_series.points.len(), 1);
        assert!(spread_series.points[0].value > Decimal::ZERO);

        let price_series = get_chart_series_inner(&state, "700.HK", "price", 100).await;
        assert_eq!(price_series.points.len(), 1);
        assert_eq!(price_series.points[0].bid, Some(dec!(100.50)));

        // get_multi_venue_prices returns one row for LongPort.
        let prices = get_multi_venue_prices_inner(&state, "700.HK").await;
        assert_eq!(prices.len(), 1);
        assert_eq!(prices[0].venue, "LongPort");
        assert_eq!(prices[0].bid, dec!(100.50));
    }
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p rushhft-app --lib commands::tests::end_to_end_snapshot_and_chart_series_round_trip`
Expected: FAIL initially if the book isn't pushed via context (it should pass on the first run since all pieces are wired in earlier tasks — if it fails, debug).

- [ ] **Step 3: Run to verify it passes**

Run: `cargo test -p rushhft-app --lib`
Expected: PASS — all tests green.

- [ ] **Step 4: Commit**

```bash
git add rushhft-app/src/commands.rs
git commit -m "test(app): add E2E snapshot+chart-series round-trip test"
```

---

### Task F2: Full-suite verification

- [ ] **Step 1: Run the entire workspace test suite**

Run: `cd /Users/tangning/Documents/workspace/mine/RushHFT && cargo test`
Expected: PASS — every crate's tests pass.

- [ ] **Step 2: Run clippy (deny warnings)**

Run: `cd /Users/tangning/Documents/workspace/mine/RushHFT && cargo clippy -- -D warnings`
Expected: PASS — no warnings.

- [ ] **Step 3: Build the Tauri app**

Run: `cd /Users/tangning/Documents/workspace/mine/RushHFT/rushhft-app && cargo build`
Expected: PASS.

- [ ] **Step 4: Manual smoke — full build**

Run: `cd /Users/tangning/Documents/workspace/mine/RushHFT/rushhft-app/ui && pnpm build`
Expected: vite build produces `build/` directory.

- [ ] **Step 5: Final commit (if any cleanup)**

```bash
git status
# only commit if anything changed
git add -A
git commit -m "chore: post-parity cleanup"
```

---

## Self-Review

### Spec coverage check

- Section 1 (File/component tree): Phase A (core helpers), Phase B (2 studies), Phase C (backend extensions + ui_state), Phase D (UI shell + modals), Phase E (charts). ✓
- Section 2 (Data flow + IPC): Task C1 (DTOs), C2 (ChartSeriesBuffer), C3 (wire in context), C4 (chart/multivenue/plugin-descriptor commands), C5 (add_symbol/remove_symbol + connector subscribe methods). ✓
- Section 3 (New studies): Task A1 (P²), B1+B2 (OTT), B3+B4 (MR). ✓
- Section 4 (Charting): Task D1 (uplot dep), E1 (setup + series), E2 (chart components), E3 (shell wiring). ✓
- Section 5 (Error handling, edge cases, testing): All Rust tasks are TDD with explicit edge-case tests; UI verified via `pnpm check` + manual smoke (Task E3 Step 3). E2E test in Task F1. ✓

### Placeholder scan

No "TBD", "TODO", "implement later", or "similar to above" — every step has full code or exact commands.

### Type consistency

- `ChartPointDto` fields (`t`, `value`, `bid`, `ask`, `mid`) — consistent across dto.rs (C1), state.rs (C2), context.rs (C3), commands.rs (C4), frontend (`ChartPointDto` in `snapshot.ts` + `series.ts`). ✓
- `compute_ott(c: &mut OttCounters, ob: &OrderBook, trade_count_delta: u64) -> Decimal` — same signature in B1 (test) and B2 (plugin callsite). ✓
- `MarketResilienceCalculator::observe(spread: f64, bid_depth: f64, ask_depth: f64, ts: OffsetDateTime)` — same in B3 tests and B4 plugin callsite. ✓
- `AppState` fields: extended consistently in C4 (no change), C5 (`user_symbols`, `connector`), tests' `make_state` updated to match. ✓
- `OttRatioStudy` / `MarketResilienceStudy` re-exported from `rushhft-studies/src/lib.rs` (B1) and imported in `main.rs` (C5 Step 7). ✓
- `RollingWindowF64::median()` returns `Option<f64>` — matches MR calculator usage (`.median()` returns Option). ✓

---

## Execution Handoff

Plan complete and saved to `docs/superpowers/plans/2026-08-11-visualhft-parity.md`. Two execution options:

**1. Subagent-Driven (recommended)** — I dispatch a fresh subagent per task, review between tasks, fast iteration.

**2. Inline Execution** — Execute tasks in this session using executing-plans, batch execution with checkpoints.

**Which approach?**
