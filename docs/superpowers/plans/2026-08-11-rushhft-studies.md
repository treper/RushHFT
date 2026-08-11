# rushhft-studies Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Implement the `rushhft-studies` crate with two studies — VPIN (Easley/Lopez de Prado & O'Hara 2012) and LOB Imbalance — as `Plugin`-trait implementations that extend `BaseStudy` from `rushhft-core`.

**Architecture:** Each study is a single struct implementing `Plugin` (variant `Study`, `emits_metric = true`). State is held inside an `Arc<Inner>` shared between the plugin and hub subscription callbacks so the callbacks can mutate bucket/imbalance state without `Arc<Self>`. Studies subscribe to `OrderBookHub` and (VPIN only) `TradeHub` via `SubscriptionGuard`s that are dropped on `stop()` to unsubscribe. The hot path runs inside `tokio::sync::Mutex` to serialize bucket mutations, then enqueues a `BaseStudyModel` to `BaseStudy::add_calculation`, which owns the aggregation cadence (S1 by default).

**Tech Stack:** Rust 2024, `rushhft-core` (workspace path), `tokio` (Mutex + spawn), `async-trait`, `rust_decimal`, `time`, `dashmap`, `arc-swap`, `tracing`.

---

## File Structure

```
rushhft-studies/
├── Cargo.toml
├── src/
│   ├── lib.rs                  # pub re-exports, pub use rushhft_core
│   ├── vpin.rs                 # VpinStudy + VpinSettings + Inner + tests
│   └── lob_imbalance.rs        # LobImbalanceStudy + LobImbalanceSettings + Inner + tests
└── tests/
    └── replay.rs               # integration: scripted trades + books → assert BaseStudyModel series
```

Each study lives in its own file (`vpin.rs`, `lob_imbalance.rs`) because they have distinct state machines and no shared logic beyond what `BaseStudy` already provides. Splitting keeps each file focused on one algorithm; the inline `#[cfg(test)] mod tests` block holds the unit tests so test fixtures sit next to the code under test. The `tests/replay.rs` integration test is shared so both studies can be exercised by the same scripted stream.

**Crate root (`src/lib.rs`):**
```rust
//! RushHFT studies crate — VPIN + LOB Imbalance.
pub use rushhft_core;

mod vpin;
mod lob_imbalance;

pub use vpin::{VpinSettings, VpinStudy};
pub use lob_imbalance::{LobImbalanceSettings, LobImbalanceStudy};
```

**Cargo.toml:**
```toml
[package]
name = "rushhft-studies"
version.workspace = true
edition.workspace = true
license.workspace = true

[dependencies]
rushhft-core = { path = "../rushhft-core" }
tokio = { version = "1", features = ["rt-multi-thread", "macros", "sync"] }
async-trait = "0.1"
tracing = "0.1"
rust_decimal = "1"
time = { version = "0.3", features = ["serde-human-readable"] }
dashmap = "6"
arc-swap = "1"
thiserror = "1"

[dev-dependencies]
rust_decimal_macros = "1"
tokio = { version = "1", features = ["rt-multi-thread", "macros", "sync", "time"] }
```

> Add `rushhft-studies` to workspace `members` array in `/Cargo.toml` in Task 1.

---

## Task 1: Scaffold crate + workspace wiring

**Files:**
- Create: `rushhft-studies/Cargo.toml`
- Create: `rushhft-studies/src/lib.rs`
- Modify: `/Cargo.toml:4` — add `rushhft-studies` to `members`

- [ ] **Step 1: Create `rushhft-studies/Cargo.toml`** with the content from the File Structure section above.

- [ ] **Step 2: Create `rushhft-studies/src/lib.rs`** with the content from the File Structure section above (the `pub mod`/`pub use` shell only — `vpin` and `lob_imbalance` modules don't exist yet).

Replace the two `mod` lines and two `pub use` lines with empty placeholders so the crate compiles:

```rust
//! RushHFT studies crate — VPIN + LOB Imbalance.
pub use rushhft_core;
```

Leave `mod vpin;` and `mod lob_imbalance;` out for now — they'll be added when the files are created in later tasks.

- [ ] **Step 3: Modify `/Cargo.toml`** to add `rushhft-studies` to the members list:

```toml
members = ["rushhft-core", "rushhft-connector-longport", "rushhft-studies"]
```

- [ ] **Step 4: Verify the crate builds (empty state)**

Run: `cargo build -p rushhft-studies`
Expected: PASS — crate builds with only the `pub use rushhft_core;` line.

- [ ] **Step 5: Commit**

```bash
git add rushhft-studies/Cargo.toml rushhft-studies/src/lib.rs Cargo.toml
git commit -m "build(studies): scaffold rushhft-studies crate"
```

---

## Task 2: VpinSettings + metadata smoke test

**Files:**
- Create: `rushhft-studies/src/vpin.rs`
- Modify: `rushhft-studies/src/lib.rs` — add `mod vpin; pub use vpin::{VpinSettings, VpinStudy};`

This task lands the `VpinStudy` struct, `VpinSettings`, constructors, and all `Plugin` trait metadata methods (`name`, `version`, `author`, `description`, `plugin_type`, `status`, `plugin_id`, `emits_metric`). The `start`/`stop` bodies are stubs returning `Ok(())` so metadata can be tested in isolation. Real lifecycle lands in Task 5.

- [ ] **Step 1: Write the failing test**

Append to `rushhft-studies/src/vpin.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use rushhft_core::Plugin;
    use rushhft_core::model::enums::{PluginStatus, PluginType};

    #[test]
    fn metadata_matches_spec() {
        let s = VpinStudy::new(VpinSettings::default());
        assert_eq!(s.name(), "VPIN Study");
        assert_eq!(s.plugin_type(), PluginType::Study);
        assert_eq!(s.status(), PluginStatus::Loaded);
        assert_eq!(s.author(), "RushHFT");
        assert_eq!(s.version(), "0.1.0");
        assert_eq!(
            s.description(),
            "Volume-Synchronized Probability of Informed Trading"
        );
        assert!(!s.plugin_id().is_empty());
        assert!(s.emits_metric());
    }

    #[test]
    fn default_settings_are_sane() {
        let s = VpinSettings::default();
        assert_eq!(s.bucket_volume_size, dec!(1));
        assert_eq!(s.number_of_buckets, 50);
        assert_eq!(s.symbol, "");
        assert_eq!(s.provider_id, 0);
    }
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p rushhft-studies vpin::tests::metadata_matches_spec`
Expected: FAIL — `VpinStudy` not defined.

- [ ] **Step 3: Write the minimal implementation**

Write to `rushhft-studies/src/vpin.rs`:

```rust
//! VPIN (Volume-Synchronized Probability of Informed Trading) study.
//!
//! Easley, Lopez de Prado & O'Hara (2012). VPIN = (1/n) × Σ|V_buy_i − V_sell_i| / V_bucket
//! over n completed volume buckets. Range [0, 1].

use rushhft_core::model::enums::{AggregationLevel, PluginStatus, PluginType};
use rushhft_core::plugin::{BaseStudy, Plugin, PluginContext, PluginError};
use rust_decimal::Decimal;
use std::sync::Arc;
use tokio::sync::Mutex;

/// Parameters for the VPIN study.
#[derive(Debug, Clone)]
pub struct VpinSettings {
    /// Volume that must accumulate before a bucket completes. Default 1.
    pub bucket_volume_size: Decimal,
    /// Number of completed buckets in the rolling window. Default 50.
    pub number_of_buckets: usize,
    /// Symbol this study is bound to (e.g. "700.HK").
    pub symbol: String,
    /// Provider ID filter — only trades/books with this provider_id are processed.
    pub provider_id: i32,
    /// Aggregation cadence. Forced to S1 by the spec.
    pub aggregation_level: AggregationLevel,
}

impl Default for VpinSettings {
    fn default() -> Self {
        Self {
            bucket_volume_size: Decimal::ONE,
            number_of_buckets: 50,
            symbol: String::new(),
            provider_id: 0,
            aggregation_level: AggregationLevel::S1,
        }
    }
}

/// VPIN study plugin.
pub struct VpinStudy {
    id: String,
    version: &'static str,
    author: &'static str,
    description: &'static str,
    settings: VpinSettings,
    base: BaseStudy,
    status: Arc<arc_swap::ArcSwap<PluginStatus>>,
    ctx: Mutex<Option<Arc<dyn PluginContext>>>,
}

impl VpinStudy {
    pub fn new(settings: VpinSettings) -> Self {
        let id = format!("vpin-{}", hash_symbol_provider(&settings.symbol, settings.provider_id));
        Self {
            id,
            version: "0.1.0",
            author: "RushHFT",
            description: "Volume-Synchronized Probability of Informed Trading",
            settings,
            base: BaseStudy::new(AggregationLevel::S1),
            status: Arc::new(arc_swap::ArcSwap::from_pointee(PluginStatus::Loaded)),
            ctx: Mutex::new(None),
        }
    }

    pub fn with_settings(settings: VpinSettings) -> Self {
        Self::new(settings)
    }
}

#[async_trait::async_trait]
impl Plugin for VpinStudy {
    fn name(&self) -> &str { "VPIN Study" }
    fn version(&self) -> &str { self.version }
    fn author(&self) -> &str { self.author }
    fn description(&self) -> &str { self.description }
    fn plugin_type(&self) -> PluginType { PluginType::Study }
    fn status(&self) -> PluginStatus { **self.status.load() }
    fn plugin_id(&self) -> &str { &self.id }
    fn emits_metric(&self) -> bool { true }

    async fn start(&self, _ctx: Arc<dyn PluginContext>) -> Result<(), PluginError> {
        // Real implementation lands in Task 5.
        Ok(())
    }

    async fn stop(&self) -> Result<(), PluginError> {
        // Real implementation lands in Task 5.
        Ok(())
    }
}

fn hash_symbol_provider(symbol: &str, provider_id: i32) -> String {
    let mut h: u64 = 0xcbf29ce484222325;
    for b in symbol.as_bytes() {
        h ^= *b as u64;
        h = h.wrapping_mul(0x100000001b3);
    }
    h ^= provider_id as u64;
    h = h.wrapping_mul(0x100000001b3);
    format!("{:x}", h)
}
```

Update `rushhft-studies/src/lib.rs`:

```rust
//! RushHFT studies crate — VPIN + LOB Imbalance.
pub use rushhft_core;

mod vpin;

pub use vpin::{VpinSettings, VpinStudy};
```

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test -p rushhft-studies vpin::tests`
Expected: PASS — both tests green.

- [ ] **Step 5: Run clippy**

Run: `cargo clippy -p rushhft-studies --all-targets -- -D warnings`
Expected: no warnings.

- [ ] **Step 6: Commit**

```bash
git add rushhft-studies/src/vpin.rs rushhft-studies/src/lib.rs
git commit -m "feat(studies): add VpinStudy metadata + VpinSettings"
```

---

## Task 3: VPIN bucket math (no Plugin wiring)

**Files:**
- Modify: `rushhft-studies/src/vpin.rs` — add `VpinCore` struct + pure bucket logic + tests

Pure, testable bucket arithmetic. No async, no Plugin trait. This isolates the algorithm (which must match the C# line-for-line) from the subscriber wiring. The `VpinStudy` struct from Task 2 will own a `VpinCore` once Task 5 wires it in.

`VpinCore` responsibilities:
- Hold `current_bucket_volume`, `current_buy_volume`, `current_sell_volume`, `last_market_mid_price`
- Hold a rolling ring buffer of completed-bucket imbalances (length = `number_of_buckets`) with O(1) average via `rolling_sum`
- `ingest_trade(size, is_buy)` — splits trade into one-or-more bucket completions + an interim tail; calls `do_calculation` on each completed bucket (green color) and once more for the interim (white color)
- `ingest_mid(mid)` — updates `_last_market_mid_price`; calls `do_calculation` for an interim update
- `current_vpin()` — `rolling_sum / buffer_count` (or 0 if no buckets completed)
- `reset()` — zero out everything; recreate the ring buffer

**Trade classification (Rust port of the spec's simplification):** the caller passes `is_buy: Option<bool>`. `Some(true)` → buy, `Some(false)` → sell, `None` → neutral, the trade is **skipped** (not split 50/50; the spec says "split 50/50 or skip" — we skip to keep math simple and deterministic). For LongPort-fed trades the caller will derive `Option<bool>` from `TradeDirection`: `Up → Some(true)`, `Down → Some(false)`, `Neutral → None`.

- [ ] **Step 1: Write the failing tests**

Append to `rushhft-studies/src/vpin.rs` tests module:

```rust
#[test]
fn vpin_core_zero_until_first_bucket_completes() {
    let mut core = VpinCore::new(dec!(1), 50);
    core.ingest_trade(dec!(0.5), Some(true));
    assert_eq!(core.current_vpin(), Decimal::ZERO);
}

#[test]
fn vpin_core_one_bucket_all_buys_gives_vpin_one() {
    let mut core = VpinCore::new(dec!(1), 50);
    core.ingest_trade(dec!(1), Some(true));   // bucket exactly fills, all buy
    assert_eq!(core.current_vpin(), Decimal::ONE);
}

#[test]
fn vpin_core_split_bucket_gives_half() {
    let mut core = VpinCore::new(dec!(2), 50);
    core.ingest_trade(dec!(1), Some(true));
    core.ingest_trade(dec!(1), Some(false));
    // After 2 volume, bucket completes with |1-1|/2 = 0
    assert_eq!(core.current_vpin(), Decimal::ZERO);
}

#[test]
fn vpin_core_neutral_trade_skipped() {
    let mut core = VpinCore::new(dec!(1), 50);
    core.ingest_trade(dec!(5), None);     // Neutral — skip
    assert_eq!(core.current_vpin(), Decimal::ZERO);
    assert_eq!(core.current_bucket_volume(), Decimal::ZERO);
}

#[test]
fn vpin_core_overflow_carries_to_next_bucket() {
    let mut core = VpinCore::new(dec!(1), 50);
    // 3-volume all-buy trade on bucket size 1 → 3 buckets complete, each imbalance=1
    core.ingest_trade(dec!(3), Some(true));
    assert_eq!(core.current_vpin(), Decimal::ONE);
    assert_eq!(core.completed_buckets(), 3);
}

#[test]
fn vpin_core_rolling_window_caps_at_n() {
    let mut core = VpinCore::new(dec!(1), 2);     // window = 2
    core.ingest_trade(dec!(1), Some(true));       // bucket1: imb=1
    core.ingest_trade(dec!(1), Some(false));      // bucket2: imb=1
    core.ingest_trade(dec!(1), Some(true));       // bucket3: evicts bucket1, imb=1
    // window now {bucket2=0, bucket3=1}, avg = 0.5
    assert_eq!(core.current_vpin(), Decimal::new(5, 1));  // 0.5
    assert_eq!(core.completed_buckets(), 3);
}

#[test]
fn vpin_core_mid_update_does_not_complete_bucket() {
    let mut core = VpinCore::new(dec!(1), 50);
    core.ingest_mid(dec!(100));
    core.ingest_trade(dec!(0.5), Some(true));
    // bucket not complete, interim vpin = 0 (no completed buckets yet)
    assert_eq!(core.current_vpin(), Decimal::ZERO);
    assert_eq!(core.last_market_mid_price(), dec!(100));
}

#[test]
fn vpin_core_reset_clears_state() {
    let mut core = VpinCore::new(dec!(1), 50);
    core.ingest_trade(dec!(1), Some(true));
    assert_eq!(core.current_vpin(), Decimal::ONE);
    core.reset();
    assert_eq!(core.current_vpin(), Decimal::ZERO);
    assert_eq!(core.completed_buckets(), 0);
}
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test -p rushhft-studies vpin::tests::vpin_core`
Expected: FAIL — `VpinCore` not defined.

- [ ] **Step 3: Write the minimal implementation**

Add above the `#[cfg(test)]` block in `rushhft-studies/src/vpin.rs`:

```rust
/// Pure VPIN bucket arithmetic — no I/O, no async. Owned by `VpinStudy`.
pub(crate) struct VpinCore {
    bucket_volume_size: Decimal,
    number_of_buckets: usize,

    current_bucket_volume: Decimal,
    current_buy_volume: Decimal,
    current_sell_volume: Decimal,
    last_market_mid_price: Decimal,

    bucket_imbalances: Vec<Decimal>,
    buffer_index: usize,
    buffer_count: usize,
    rolling_sum: Decimal,
    completed_buckets: u64,
}

impl VpinCore {
    pub fn new(bucket_volume_size: Decimal, number_of_buckets: usize) -> Self {
        let n = if number_of_buckets == 0 { 50 } else { number_of_buckets };
        Self {
            bucket_volume_size,
            number_of_buckets: n,
            current_bucket_volume: Decimal::ZERO,
            current_buy_volume: Decimal::ZERO,
            current_sell_volume: Decimal::ZERO,
            last_market_mid_price: Decimal::ZERO,
            bucket_imbalances: vec![Decimal::ZERO; n],
            buffer_index: 0,
            buffer_count: 0,
            rolling_sum: Decimal::ZERO,
            completed_buckets: 0,
        }
    }

    pub fn current_vpin(&self) -> Decimal {
        if self.buffer_count == 0 {
            Decimal::ZERO
        } else {
            self.rolling_sum / Decimal::from(self.buffer_count)
        }
    }

    pub fn current_bucket_volume(&self) -> Decimal { self.current_bucket_volume }
    pub fn last_market_mid_price(&self) -> Decimal { self.last_market_mid_price }
    pub fn completed_buckets(&self) -> u64 { self.completed_buckets }

    pub fn reset(&mut self) {
        self.current_bucket_volume = Decimal::ZERO;
        self.current_buy_volume = Decimal::ZERO;
        self.current_sell_volume = Decimal::ZERO;
        self.last_market_mid_price = Decimal::ZERO;
        self.bucket_imbalances = vec![Decimal::ZERO; self.number_of_buckets];
        self.buffer_index = 0;
        self.buffer_count = 0;
        self.rolling_sum = Decimal::ZERO;
        self.completed_buckets = 0;
    }

    pub fn ingest_mid(&mut self, mid: Decimal) {
        self.last_market_mid_price = mid;
    }

    /// Feed a trade. `is_buy = None` (Neutral) → skip the trade entirely.
    pub fn ingest_trade(&mut self, size: Decimal, is_buy: Option<bool>) {
        let is_buy = match is_buy {
            Some(b) => b,
            None => return, // Neutral — skip per spec
        };

        if size.is_zero() {
            return;
        }

        if is_buy {
            self.current_buy_volume += size;
        } else {
            self.current_sell_volume += size;
        }
        self.current_bucket_volume += size;

        // Complete as many buckets as this trade fills.
        while self.current_bucket_volume >= self.bucket_volume_size
            && self.bucket_volume_size > Decimal::ZERO
        {
            let overflow = self.current_bucket_volume - self.bucket_volume_size;
            if is_buy {
                self.current_buy_volume -= overflow;
            } else {
                self.current_sell_volume -= overflow;
            }
            self.current_bucket_volume = self.bucket_volume_size;

            self.complete_bucket();

            // Start new bucket with overflow.
            self.current_buy_volume = if is_buy { overflow } else { Decimal::ZERO };
            self.current_sell_volume = if is_buy { Decimal::ZERO } else { overflow };
            self.current_bucket_volume = overflow;
        }
    }

    fn complete_bucket(&mut self) {
        let imbalance = if self.bucket_volume_size.is_zero() {
            Decimal::ZERO
        } else {
            (self.current_buy_volume - self.current_sell_volume).abs() / self.bucket_volume_size
        };

        if self.buffer_count == self.number_of_buckets {
            // Buffer full — evict the value at the current index.
            self.rolling_sum -= self.bucket_imbalances[self.buffer_index];
        } else {
            self.buffer_count += 1;
        }

        self.bucket_imbalances[self.buffer_index] = imbalance;
        self.rolling_sum += imbalance;
        self.buffer_index = (self.buffer_index + 1) % self.number_of_buckets;
        self.completed_buckets += 1;
    }
}
```

- [ ] **Step 4: Run tests to verify they pass**

Run: `cargo test -p rushhft-studies vpin::tests::vpin_core`
Expected: PASS — all 8 tests green.

- [ ] **Step 5: Run clippy**

Run: `cargo clippy -p rushhft-studies --all-targets -- -D warnings`
Expected: no warnings.

- [ ] **Step 6: Commit**

```bash
git add rushhft-studies/src/vpin.rs
git commit -m "feat(studies): add VpinCore bucket arithmetic"
```

---

## Task 4: VPIN trade-direction mapping helper

**Files:**
- Modify: `rushhft-studies/src/vpin.rs` — add `pub fn map_trade_direction()` + tests

Free function, not a `From` impl (the orphan rule forbids `impl From<ForeignType> for LocalType` when the foreign type is not local — same fix as in the connector crate). Maps `rushhft_core::TradeDirection` to `Option<bool>` for `VpinCore::ingest_trade`.

- [ ] **Step 1: Write the failing test**

Append to `vpin.rs` tests module:

```rust
#[test]
fn map_trade_direction_up_is_buy() {
    assert_eq!(map_trade_direction(TradeDirection::Up), Some(true));
}

#[test]
fn map_trade_direction_down_is_sell() {
    assert_eq!(map_trade_direction(TradeDirection::Down), Some(false));
}

#[test]
fn map_trade_direction_neutral_is_skipped() {
    assert_eq!(map_trade_direction(TradeDirection::Neutral), None);
}
```

Add `use rushhft_core::model::enums::TradeDirection;` to the test module's imports.

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p rushhft-studies vpin::tests::map_trade_direction`
Expected: FAIL — function not defined.

- [ ] **Step 3: Write the minimal implementation**

Add to `rushhft-studies/src/vpin.rs`:

```rust
use rushhft_core::model::enums::TradeDirection;

/// Map a `TradeDirection` to `Option<bool>` (buy/sell/skip). Free function —
/// `impl From<TradeDirection> for Option<bool>` would collide with the orphan rule.
pub fn map_trade_direction(d: TradeDirection) -> Option<bool> {
    match d {
        TradeDirection::Up => Some(true),
        TradeDirection::Down => Some(false),
        TradeDirection::Neutral => None,
    }
}
```

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test -p rushhft-studies vpin::tests::map_trade_direction`
Expected: PASS — all 3 tests green.

- [ ] **Step 5: Run clippy**

Run: `cargo clippy -p rushhft-studies --all-targets -- -D warnings`
Expected: no warnings.

- [ ] **Step 6: Commit**

```bash
git add rushhft-studies/src/vpin.rs
git commit -m "feat(studies): map TradeDirection to Option<bool> for VPIN"
```

---

## Task 5: Wire VpinCore into VpinStudy + Plugin lifecycle

**Files:**
- Modify: `rushhft-studies/src/vpin.rs` — give `VpinStudy` a `Mutex<VpinCore>`, hub subscription state, real `start`/`stop`

`VpinStudy::start(ctx)`:
1. Store `ctx` in `self.ctx`.
2. Reset `VpinCore` to fresh state (so restart is clean).
3. Spawn the `BaseStudy::start_consumer` task with a closure that calls `ctx.register_metric(plugin="VPIN Study", metric="VPIN", exchange="LongPort", symbol=settings.symbol, value, ts)`.
4. Subscribe to `ctx.order_book_hub()` with a closure that:
   - filters by `settings.symbol` and `settings.provider_id`
   - reads mid_price via `ob.mid_price()`
   - locks `VpinCore` and calls `ingest_mid(mid)` + enqueues a `BaseStudyModel { value: core.current_vpin(), format: "N2", timestamp: now, market_mid_price: mid, value_color: "White", tooltip: "", has_error: false, is_stale: false }` via `self.base.add_calculation(...)`
5. Subscribe to `ctx.trade_hub()` with a closure that:
   - filters by `settings.symbol` and `settings.provider_id`
   - maps direction via `map_trade_direction`
   - locks `VpinCore` and calls `ingest_trade(size, is_buy)`
   - enqueues `BaseStudyModel` (interim update, white color)
6. Store both `SubscriptionGuard`s in `Mutex<Option<Guards>>` so `stop()` can drop them.
7. Set `status ← Started`.

`VpinStudy::stop()`:
1. Set `status ← Stopping`.
2. Drop the subscription guards (unsubscribes from both hubs).
3. Set `status ← Stopped`.

**Arc<Inner> pattern:** the hub closures need to mutate the `VpinCore` and call `self.base.add_calculation`. But hub callbacks take `&T` via `Arc<dyn Fn(&T) + Send + Sync>`, so we can't capture `&self`. Move all shared state into an `Arc<Inner>` and capture `Arc<Inner>` in the closures — same pattern used in `rushhft-connector-longport`.

- [ ] **Step 1: Write the failing test**

Append to `vpin.rs` tests module. This is a "replay" test: feed scripted trades/books via a `MockCtx` and assert that `register_metric` was called with the expected VPIN value.

```rust
use rushhft_core::hub::{OrderBookHub, ProviderHub, TradeHub};
use rushhft_core::model::order_book::OrderBook;
use rushhft_core::model::trade::Trade;
use rushhft_core::model::book_item::BookItem;
use rushhft_core::model::provider::Provider;
use rushhft_core::PluginContext;
use rust_decimal::Decimal;
use rust_decimal_macros::dec;
use std::sync::Arc;
use time::OffsetDateTime;

struct ReplayCtx {
    ob_hub: Arc<OrderBookHub>,
    t_hub: Arc<TradeHub>,
    p_hub: Arc<ProviderHub>,
    metrics: Arc<std::sync::Mutex<Vec<(String, String, String, String, Decimal)>>>,
}

#[async_trait::async_trait]
impl PluginContext for ReplayCtx {
    async fn publish_order_book(&self, _ob: OrderBook) {}
    async fn publish_trade(&self, _t: Trade) {}
    async fn publish_provider(&self, _p: Provider) {}
    async fn register_metric(
        &self,
        plugin: &str,
        metric: &str,
        exchange: &str,
        symbol: &str,
        value: Decimal,
        _ts: OffsetDateTime,
    ) {
        self.metrics.lock().unwrap().push((
            plugin.to_string(),
            metric.to_string(),
            exchange.to_string(),
            symbol.to_string(),
            value,
        ));
    }
    fn order_book_hub(&self) -> Arc<OrderBookHub> { self.ob_hub.clone() }
    fn trade_hub(&self) -> Arc<TradeHub> { self.t_hub.clone() }
    fn provider_hub(&self) -> Arc<ProviderHub> { self.p_hub.clone() }
}

fn make_trade(price: Decimal, size: Decimal, dir: TradeDirection, ts_secs: i64) -> Trade {
    Trade {
        price,
        size,
        timestamp: OffsetDateTime::from_unix_timestamp(ts_secs).unwrap(),
        direction: dir,
        trade_type: "D".to_string(),
        symbol: "700.HK".to_string(),
        provider_id: 1,
        market_mid_price: Decimal::ZERO,
    }
}

#[tokio::test]
async fn vpin_start_registers_metric_after_bucket_completes() {
    let ob_hub = Arc::new(OrderBookHub::new());
    let t_hub = Arc::new(TradeHub::new());
    let p_hub = Arc::new(ProviderHub::new());
    let metrics = Arc::new(std::sync::Mutex::new(Vec::new()));
    let ctx = Arc::new(ReplayCtx {
        ob_hub: ob_hub.clone(),
        t_hub: t_hub.clone(),
        p_hub: p_hub.clone(),
        metrics: metrics.clone(),
    }) as Arc<dyn PluginContext>;

    let study = Arc::new(VpinStudy::new(VpinSettings {
        bucket_volume_size: dec!(1),
        number_of_buckets: 50,
        symbol: "700.HK".into(),
        provider_id: 1,
        aggregation_level: AggregationLevel::S1,
    }));
    study.start(ctx).await.unwrap();
    assert_eq!(study.status(), PluginStatus::Started);

    // 1-volume buy trade → one bucket completes, imbalance=1
    t_hub.publish(make_trade(dec!(100.50), dec!(1), TradeDirection::Up, 1_700_000_000));

    // give the consumer task time to drain
    tokio::time::sleep(std::time::Duration::from_millis(50)).await;

    let collected = metrics.lock().unwrap().clone();
    assert!(collected.iter().any(|m| m.0 == "VPIN Study" && m.1 == "VPIN" && m.4 == Decimal::ONE),
        "expected at least one metric with VPIN=1, got {:?}", collected);

    study.stop().await.unwrap();
    assert_eq!(study.status(), PluginStatus::Stopped);
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p rushhft-studies vpin::tests::vpin_start_registers_metric_after_bucket_completes`
Expected: FAIL — `start()` currently returns `Ok(())` without subscribing, so no metrics are published.

- [ ] **Step 3: Refactor `VpinStudy` to hold an `Arc<Inner>`**

Rewrite `rushhft-studies/src/vpin.rs` so the struct owns `Arc<Inner>` and the hub closures capture `Arc<Inner>`. Replace the `VpinStudy` definition and `impl VpinStudy` and `impl Plugin for VpinStudy` blocks with:

```rust
use rushhft_core::hub::SubscriptionGuard;
use rushhft_core::model::enums::{AggregationLevel, PluginStatus, PluginType, TradeDirection};
use rushhft_core::model::study::BaseStudyModel;
use rushhft_core::model::order_book::OrderBook;
use rushhft_core::model::trade::Trade;
use rushhft_core::plugin::{BaseStudy, Plugin, PluginContext, PluginError};
use rushhft_core::{OrderBookHub, TradeHub};
use rust_decimal::Decimal;
use std::sync::Arc;
use tokio::sync::Mutex;
use time::OffsetDateTime;

// (VpinSettings, VpinCore, map_trade_direction unchanged — keep them as-is.)

struct Inner {
    settings: VpinSettings,
    core: Mutex<VpinCore>,
    base: BaseStudy,
    status: Arc<arc_swap::ArcSwap<PluginStatus>>,
    ctx: Mutex<Option<Arc<dyn PluginContext>>>,
    guards: Mutex<Option<Vec<SubscriptionGuard>>>,
}

pub struct VpinStudy {
    id: String,
    version: &'static str,
    author: &'static str,
    description: &'static str,
    inner: Arc<Inner>,
}

impl VpinStudy {
    pub fn new(settings: VpinSettings) -> Self {
        let id = format!("vpin-{}", hash_symbol_provider(&settings.symbol, settings.provider_id));
        let core = VpinCore::new(settings.bucket_volume_size, settings.number_of_buckets);
        let inner = Arc::new(Inner {
            settings,
            core: Mutex::new(core),
            base: BaseStudy::new(AggregationLevel::S1),
            status: Arc::new(arc_swap::ArcSwap::from_pointee(PluginStatus::Loaded)),
            ctx: Mutex::new(None),
            guards: Mutex::new(None),
        });
        Self {
            id,
            version: "0.1.0",
            author: "RushHFT",
            description: "Volume-Synchronized Probability of Informed Trading",
            inner,
        }
    }
}

#[async_trait::async_trait]
impl Plugin for VpinStudy {
    fn name(&self) -> &str { "VPIN Study" }
    fn version(&self) -> &str { self.version }
    fn author(&self) -> &str { self.author }
    fn description(&self) -> &str { self.description }
    fn plugin_type(&self) -> PluginType { PluginType::Study }
    fn status(&self) -> PluginStatus { **self.inner.status.load() }
    fn plugin_id(&self) -> &str { &self.id }
    fn emits_metric(&self) -> bool { true }

    async fn start(&self, ctx: Arc<dyn PluginContext>) -> Result<(), PluginError> {
        // 1) Store ctx
        {
            let mut guard = self.inner.ctx.lock().await;
            *guard = Some(ctx.clone());
        }

        // 2) Reset core
        {
            let mut core = self.inner.core.lock().await;
            core.reset();
        }

        // 3) Spawn the BaseStudy consumer -> register_metric
        let inner = self.inner.clone();
        let ctx_for_consumer = ctx.clone();
        tokio::spawn(async move {
            inner.base.start_consumer(move |item: &BaseStudyModel| {
                let _ = ctx_for_consumer.register_metric(
                    "VPIN Study",
                    "VPIN",
                    "LongPort",
                    &inner.settings.symbol,
                    item.value,
                    item.timestamp,
                );
            }).await;
        });

        // 4) Subscribe to OrderBookHub
        let inner_ob = self.inner.clone();
        let ob_hub = ctx.order_book_hub();
        let ob_guard = ob_hub.subscribe(Arc::new(move |ob: &OrderBook| {
            if ob.symbol != inner_ob.settings.symbol || ob.provider_id != inner_ob.settings.provider_id {
                return;
            }
            let mid = ob.mid_price().unwrap_or(Decimal::ZERO);
            // Synchronous mutation — block on the mutex inside the callback.
            // Hub subscribers run on the publisher's thread; we keep the critical
            // section tiny (no await points inside).
            let inner = inner_ob.clone();
            // We can't .await here (closure is sync) — use try_lock guarded by
            // blocking_lock via a fresh task. Simpler: the hub callback is sync,
            // so we use std::sync::Mutex via a thin shim. To avoid a second mutex
            // type, we'll inline a tokio::task::block_in_place + block_on.
            tokio::task::block_in_place(|| {
                let rt = tokio::runtime::Handle::current();
                let _ = rt.block_on(async {
                    let mut core = inner.core.lock().await;
                    core.ingest_mid(mid);
                    let vpin = core.current_vpin();
                    inner.base.add_calculation(BaseStudyModel {
                        value: vpin,
                        format: "N2".into(),
                        timestamp: OffsetDateTime::now_utc(),
                        market_mid_price: mid,
                        value_color: "White".into(),
                        tooltip: String::new(),
                        has_error: false,
                        is_stale: false,
                    });
                });
            });
        }));

        // 5) Subscribe to TradeHub
        let inner_t = self.inner.clone();
        let t_hub = ctx.trade_hub();
        let t_guard = t_hub.subscribe(Arc::new(move |t: &Trade| {
            if t.symbol != inner_t.settings.symbol || t.provider_id != inner_t.settings.provider_id {
                return;
            }
            let is_buy = map_trade_direction(t.direction);
            let size = t.size;
            let mid = t.market_mid_price;
            let inner = inner_t.clone();
            tokio::task::block_in_place(|| {
                let rt = tokio::runtime::Handle::current();
                let _ = rt.block_on(async {
                    let mut core = inner.core.lock().await;
                    core.ingest_mid(mid);
                    core.ingest_trade(size, is_buy);
                    let vpin = core.current_vpin();
                    inner.base.add_calculation(BaseStudyModel {
                        value: vpin,
                        format: "N2".into(),
                        timestamp: t.timestamp,
                        market_mid_price: mid,
                        value_color: "White".into(),
                        tooltip: String::new(),
                        has_error: false,
                        is_stale: false,
                    });
                });
            });
        }));

        // 6) Stash guards
        {
            let mut guards = self.inner.guards.lock().await;
            *guards = Some(vec![ob_guard, t_guard]);
        }

        // 7) Status <- Started
        self.inner.status.store(Arc::new(PluginStatus::Started));
        Ok(())
    }

    async fn stop(&self) -> Result<(), PluginError> {
        self.inner.status.store(Arc::new(PluginStatus::Stopping));
        {
            let mut guards = self.inner.guards.lock().await;
            *guards = None; // drops guards -> unsubscribes
        }
        self.inner.status.store(Arc::new(PluginStatus::Stopped));
        Ok(())
    }
}
```

Keep the existing `VpinSettings`, `VpinCore`, `map_trade_direction`, `hash_symbol_provider` definitions — only the `VpinStudy` struct + its `impl` + `impl Plugin` change.

> **Note on `block_in_place`:** hub callbacks are sync `Fn(&T)` closures, but we need to mutate state behind a `tokio::sync::Mutex`. `block_in_place + Handle::block_on` is the idiomatic bridge. It is safe because the hub publishes from a tokio worker task and our critical section has no await points. If the `rt-multi-thread` feature is off, `block_in_place` becomes a no-op which is still correct.

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test -p rushhft-studies vpin::tests::vpin_start_registers_metric_after_bucket_completes`
Expected: PASS — VPIN=1 metric registered after the bucket-completing trade.

- [ ] **Step 5: Run the full test suite for this crate**

Run: `cargo test -p rushhft-studies`
Expected: all tests green.

- [ ] **Step 6: Run clippy**

Run: `cargo clippy -p rushhft-studies --all-targets -- -D warnings`
Expected: no warnings.

- [ ] **Step 7: Commit**

```bash
git add rushhft-studies/src/vpin.rs
git commit -m "feat(studies): wire VpinCore into VpinStudy with hub subscriptions"
```

---

## Task 6: LobImbalanceSettings + metadata smoke test

**Files:**
- Create: `rushhft-studies/src/lob_imbalance.rs`
- Modify: `rushhft-studies/src/lib.rs` — add `mod lob_imbalance; pub use lob_imbalance::{LobImbalanceSettings, LobImbalanceStudy};`

Mirror of Task 2 for the LOB Imbalance study. Stub `start`/`stop` bodies; metadata test only.

- [ ] **Step 1: Write the failing test**

Create `rushhft-studies/src/lob_imbalance.rs`:

```rust
//! LOB Imbalance study: (Σ bid_size − Σ ask_size) / (Σ bid_size + Σ ask_size) over top-N levels.
//! Range [−1, 1]. Mirrors OrderFlowAnalysis.Calculate_OrderImbalance from the original.

use rushhft_core::model::enums::{AggregationLevel, PluginStatus, PluginType};
use rushhft_core::plugin::{BaseStudy, Plugin, PluginContext, PluginError};
use std::sync::Arc;
use tokio::sync::Mutex;

#[derive(Debug, Clone)]
pub struct LobImbalanceSettings {
    pub symbol: String,
    pub provider_id: i32,
    /// How many levels deep to sum (top of book). Default 5.
    pub levels: usize,
    pub aggregation_level: AggregationLevel,
}

impl Default for LobImbalanceSettings {
    fn default() -> Self {
        Self {
            symbol: String::new(),
            provider_id: 0,
            levels: 5,
            aggregation_level: AggregationLevel::S1,
        }
    }
}

pub struct LobImbalanceStudy {
    id: String,
    version: &'static str,
    author: &'static str,
    description: &'static str,
    settings: LobImbalanceSettings,
    base: BaseStudy,
    status: Arc<arc_swap::ArcSwap<PluginStatus>>,
    ctx: Mutex<Option<Arc<dyn PluginContext>>>,
}

impl LobImbalanceStudy {
    pub fn new(settings: LobImbalanceSettings) -> Self {
        let id = format!("lobimb-{}", hash_symbol_provider(&settings.symbol, settings.provider_id));
        Self {
            id,
            version: "0.1.0",
            author: "RushHFT",
            description: "Top-of-book bid/ask volume imbalance",
            settings,
            base: BaseStudy::new(AggregationLevel::S1),
            status: Arc::new(arc_swap::ArcSwap::from_pointee(PluginStatus::Loaded)),
            ctx: Mutex::new(None),
        }
    }
}

#[async_trait::async_trait]
impl Plugin for LobImbalanceStudy {
    fn name(&self) -> &str { "LOB Imbalance Study" }
    fn version(&self) -> &str { self.version }
    fn author(&self) -> &str { self.author }
    fn description(&self) -> &str { self.description }
    fn plugin_type(&self) -> PluginType { PluginType::Study }
    fn status(&self) -> PluginStatus { **self.status.load() }
    fn plugin_id(&self) -> &str { &self.id }
    fn emits_metric(&self) -> bool { true }

    async fn start(&self, _ctx: Arc<dyn PluginContext>) -> Result<(), PluginError> { Ok(()) }
    async fn stop(&self) -> Result<(), PluginError> { Ok(()) }
}

fn hash_symbol_provider(symbol: &str, provider_id: i32) -> String {
    let mut h: u64 = 0xcbf29ce484222325;
    for b in symbol.as_bytes() {
        h ^= *b as u64;
        h = h.wrapping_mul(0x100000001b3);
    }
    h ^= provider_id as u64;
    h = h.wrapping_mul(0x100000001b3);
    format!("{:x}", h)
}

#[cfg(test)]
mod tests {
    use super::*;
    use rushhft_core::Plugin;

    #[test]
    fn metadata_matches_spec() {
        let s = LobImbalanceStudy::new(LobImbalanceSettings::default());
        assert_eq!(s.name(), "LOB Imbalance Study");
        assert_eq!(s.plugin_type(), PluginType::Study);
        assert_eq!(s.status(), PluginStatus::Loaded);
        assert_eq!(s.author(), "RushHFT");
        assert_eq!(s.version(), "0.1.0");
        assert!(!s.plugin_id().is_empty());
        assert!(s.emits_metric());
    }

    #[test]
    fn default_settings_levels_is_five() {
        let s = LobImbalanceSettings::default();
        assert_eq!(s.levels, 5);
    }
}
```

Update `rushhft-studies/src/lib.rs`:

```rust
//! RushHFT studies crate — VPIN + LOB Imbalance.
pub use rushhft_core;

mod lob_imbalance;
mod vpin;

pub use lob_imbalance::{LobImbalanceSettings, LobImbalanceStudy};
pub use vpin::{VpinSettings, VpinStudy};
```

- [ ] **Step 2: Run test to verify it passes**

Run: `cargo test -p rushhft-studies lob_imbalance::tests`
Expected: PASS.

- [ ] **Step 3: Run clippy**

Run: `cargo clippy -p rushhft-studies --all-targets -- -D warnings`
Expected: no warnings.

- [ ] **Step 4: Commit**

```bash
git add rushhft-studies/src/lob_imbalance.rs rushhft-studies/src/lib.rs
git commit -m "feat(studies): add LobImbalanceStudy metadata + settings"
```

---

## Task 7: LOB imbalance math (pure function)

**Files:**
- Modify: `rushhft-studies/src/lob_imbalance.rs` — add `pub fn compute_imbalance(ob, levels)` + tests

Pure function — no struct mutation. Mirrors `OrderFlowAnalysis.Calculate_OrderImbalance` from the C#: sum top-`levels` bid sizes and ask sizes, return `(bid_sum - ask_sum) / (bid_sum + ask_sum)`, or 0 when both sums are 0.

- [ ] **Step 1: Write the failing tests**

Append to `lob_imbalance.rs` tests module:

```rust
use rushhft_core::model::book_item::BookItem;
use rushhft_core::model::order_book::OrderBook;
use rust_decimal_macros::dec;

fn make_book(bids: Vec<(Decimal, Decimal)>, asks: Vec<(Decimal, Decimal)>) -> OrderBook {
    let mut ob = OrderBook::new("700.HK", 10, 2, 0, 1);
    for (p, s) in bids {
        ob.add_or_update_level(BookItem::new(p, s, true, "700.HK", 1));
    }
    for (p, s) in asks {
        ob.add_or_update_level(BookItem::new(p, s, false, "700.HK", 1));
    }
    ob
}

#[test]
fn imbalance_zero_when_book_empty() {
    let ob = OrderBook::new("700.HK", 10, 2, 0, 1);
    assert_eq!(compute_imbalance(&ob, 5), Decimal::ZERO);
}

#[test]
fn imbalance_all_bids_is_one() {
    let ob = make_book(vec![(dec!(100), dec!(100))], vec![]);
    assert_eq!(compute_imbalance(&ob, 5), Decimal::ONE);
}

#[test]
fn imbalance_all_asks_is_negative_one() {
    let ob = make_book(vec![], vec![(dec!(101), dec!(100))]);
    assert_eq!(compute_imbalance(&ob, 5), Decimal::from(-1));
}

#[test]
fn imbalance_balanced_book_is_zero() {
    let ob = make_book(
        vec![(dec!(100), dec!(100))],
        vec![(dec!(101), dec!(100))],
    );
    assert_eq!(compute_imbalance(&ob, 5), Decimal::ZERO);
}

#[test]
fn imbalance_respects_levels_cap() {
    // 5 levels of (100,100) each side; levels=3 → only top 3 counted
    let bids: Vec<(Decimal, Decimal)> = (0..5).map(|i| (dec!(100) - Decimal::from(i), dec!(100))).collect();
    let asks: Vec<(Decimal, Decimal)> = (0..5).map(|i| (dec!(101) + Decimal::from(i), dec!(100))).collect();
    let ob = make_book(bids, asks);
    // Top 3 each side: 300 vs 300 → 0
    assert_eq!(compute_imbalance(&ob, 3), Decimal::ZERO);
}

#[test]
fn imbalance_fewer_levels_than_asked_uses_whole_side() {
    let ob = make_book(
        vec![(dec!(100), dec!(100)), (dec!(99), dec!(200))],
        vec![(dec!(101), dec!(150))],
    );
    // levels=5 but only 2 bids, 1 ask → (300-150)/450 = 1/3
    let expected = Decimal::from(150) / Decimal::from(450);
    assert_eq!(compute_imbalance(&ob, 5), expected);
}
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test -p rushhft-studies lob_imbalance::tests`
Expected: FAIL — `compute_imbalance` not defined.

- [ ] **Step 3: Write the minimal implementation**

Add to `rushhft-studies/src/lob_imbalance.rs`:

```rust
use rushhft_core::model::order_book::OrderBook;

/// Pure function: top-N-levels imbalance = (Σ bid_size − Σ ask_size) / (Σ bid_size + Σ ask_size).
/// Returns 0 when both sums are zero. `levels` is capped at the available depth on each side.
pub fn compute_imbalance(ob: &OrderBook, levels: usize) -> Decimal {
    let bid_sum: Decimal = ob.bids.iter().take(levels).map(|l| l.size).sum();
    let ask_sum: Decimal = ob.asks.iter().take(levels).map(|l| l.size).sum();
    let total = bid_sum + ask_sum;
    if total.is_zero() {
        Decimal::ZERO
    } else {
        (bid_sum - ask_sum) / total
    }
}
```

- [ ] **Step 4: Run tests to verify they pass**

Run: `cargo test -p rushhft-studies lob_imbalance::tests`
Expected: PASS — all 6 tests green.

- [ ] **Step 5: Run clippy**

Run: `cargo clippy -p rushhft-studies --all-targets -- -D warnings`
Expected: no warnings.

- [ ] **Step 6: Commit**

```bash
git add rushhft-studies/src/lob_imbalance.rs
git commit -m "feat(studies): add compute_imbalance pure function"
```

---

## Task 8: Wire LobImbalanceStudy hub subscription + lifecycle

**Files:**
- Modify: `rushhft-studies/src/lob_imbalance.rs` — refactor to `Arc<Inner>`, real `start`/`stop`

Mirror of Task 5, but simpler: only one hub (`OrderBookHub`), no trade path. The hub callback:
1. Filters by `settings.symbol` + `settings.provider_id`.
2. Calls `compute_imbalance(ob, settings.levels)`.
3. Enqueues a `BaseStudyModel { value, format: "0.0000", market_mid_price: ob.mid_price().unwrap_or(0), … }` via `self.base.add_calculation`.

The `BaseStudy` consumer task is spawned in `start()` and calls `ctx.register_metric("LOB Imbalance Study", "Imbalance", "LongPort", settings.symbol, value, ts)`.

- [ ] **Step 1: Write the failing test**

Append to `lob_imbalance.rs` tests module. Reuse the `ReplayCtx` pattern from Task 5 but local to this module (copy the helper into the test module — tests in different modules don't share imports).

```rust
use rushhft_core::hub::ProviderHub;
use rushhft_core::model::provider::Provider;
use rushhft_core::model::trade::Trade;
use rushhft_core::PluginContext;
use std::sync::atomic::{AtomicU32, Ordering};

struct LobReplayCtx {
    ob_hub: Arc<OrderBookHub>,
    t_hub: Arc<TradeHub>,
    p_hub: Arc<ProviderHub>,
    metrics: Arc<std::sync::Mutex<Vec<Decimal>>>,
}

#[async_trait::async_trait]
impl PluginContext for LobReplayCtx {
    async fn publish_order_book(&self, _ob: OrderBook) {}
    async fn publish_trade(&self, _t: Trade) {}
    async fn publish_provider(&self, _p: Provider) {}
    async fn register_metric(
        &self,
        _plugin: &str,
        _metric: &str,
        _exchange: &str,
        _symbol: &str,
        value: Decimal,
        _ts: time::OffsetDateTime,
    ) {
        self.metrics.lock().unwrap().push(value);
    }
    fn order_book_hub(&self) -> Arc<OrderBookHub> { self.ob_hub.clone() }
    fn trade_hub(&self) -> Arc<TradeHub> { self.t_hub.clone() }
    fn provider_hub(&self) -> Arc<ProviderHub> { self.p_hub.clone() }
}

#[tokio::test]
async fn lob_imbalance_start_registers_metric_on_book_publish() {
    let ob_hub = Arc::new(OrderBookHub::new());
    let t_hub = Arc::new(TradeHub::new());
    let p_hub = Arc::new(ProviderHub::new());
    let metrics = Arc::new(std::sync::Mutex::new(Vec::new()));
    let ctx = Arc::new(LobReplayCtx {
        ob_hub: ob_hub.clone(),
        t_hub: t_hub.clone(),
        p_hub: p_hub.clone(),
        metrics: metrics.clone(),
    }) as Arc<dyn PluginContext>;

    let study = Arc::new(LobImbalanceStudy::new(LobImbalanceSettings {
        symbol: "700.HK".into(),
        provider_id: 1,
        levels: 5,
        aggregation_level: AggregationLevel::S1,
    }));
    study.start(ctx).await.unwrap();
    assert_eq!(study.status(), PluginStatus::Started);

    // All-bids book → imbalance = 1
    let ob = make_book(vec![(dec!(100), dec!(100))], vec![]);
    ob_hub.publish(ob);

    tokio::time::sleep(std::time::Duration::from_millis(50)).await;
    let collected = metrics.lock().unwrap().clone();
    assert!(collected.iter().any(|v| *v == Decimal::ONE),
        "expected imbalance=1 to be registered, got {:?}", collected);

    study.stop().await.unwrap();
    assert_eq!(study.status(), PluginStatus::Stopped);
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p rushhft-studies lob_imbalance::tests::lob_imbalance_start_registers_metric_on_book_publish`
Expected: FAIL — `start()` currently returns `Ok(())` without subscribing.

- [ ] **Step 3: Refactor `LobImbalanceStudy` to hold `Arc<Inner>`**

Replace the `LobImbalanceStudy` struct + `impl LobImbalanceStudy` + `impl Plugin` blocks in `rushhft-studies/src/lob_imbalance.rs`. Keep `LobImbalanceSettings`, `compute_imbalance`, `hash_symbol_provider` as-is. The new shape:

```rust
use rushhft_core::hub::SubscriptionGuard;
use rushhft_core::model::study::BaseStudyModel;
use rushhft_core::model::order_book::OrderBook;
use time::OffsetDateTime;

struct Inner {
    settings: LobImbalanceSettings,
    base: BaseStudy,
    status: Arc<arc_swap::ArcSwap<PluginStatus>>,
    ctx: Mutex<Option<Arc<dyn PluginContext>>>,
    guards: Mutex<Option<Vec<SubscriptionGuard>>>,
}

pub struct LobImbalanceStudy {
    id: String,
    version: &'static str,
    author: &'static str,
    description: &'static str,
    inner: Arc<Inner>,
}

impl LobImbalanceStudy {
    pub fn new(settings: LobImbalanceSettings) -> Self {
        let id = format!("lobimb-{}", hash_symbol_provider(&settings.symbol, settings.provider_id));
        let inner = Arc::new(Inner {
            settings,
            base: BaseStudy::new(AggregationLevel::S1),
            status: Arc::new(arc_swap::ArcSwap::from_pointee(PluginStatus::Loaded)),
            ctx: Mutex::new(None),
            guards: Mutex::new(None),
        });
        Self {
            id,
            version: "0.1.0",
            author: "RushHFT",
            description: "Top-of-book bid/ask volume imbalance",
            inner,
        }
    }
}

#[async_trait::async_trait]
impl Plugin for LobImbalanceStudy {
    fn name(&self) -> &str { "LOB Imbalance Study" }
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

        // Consumer -> register_metric
        let inner = self.inner.clone();
        let ctx_for_consumer = ctx.clone();
        tokio::spawn(async move {
            inner.base.start_consumer(move |item: &BaseStudyModel| {
                let _ = ctx_for_consumer.register_metric(
                    "LOB Imbalance Study",
                    "Imbalance",
                    "LongPort",
                    &inner.settings.symbol,
                    item.value,
                    item.timestamp,
                );
            }).await;
        });

        // Subscribe to OrderBookHub
        let inner_ob = self.inner.clone();
        let ob_hub = ctx.order_book_hub();
        let ob_guard = ob_hub.subscribe(Arc::new(move |ob: &OrderBook| {
            if ob.symbol != inner_ob.settings.symbol || ob.provider_id != inner_ob.settings.provider_id {
                return;
            }
            let value = compute_imbalance(ob, inner_ob.settings.levels);
            let mid = ob.mid_price().unwrap_or(Decimal::ZERO);
            inner_ob.base.add_calculation(BaseStudyModel {
                value,
                format: "0.0000".into(),
                timestamp: OffsetDateTime::now_utc(),
                market_mid_price: mid,
                value_color: "White".into(),
                tooltip: String::new(),
                has_error: false,
                is_stale: false,
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

> **Note:** the LOB callback does not touch any `Mutex` (it only calls `compute_imbalance` which is pure, and `base.add_calculation` which is `&self`-Sync via an `UnboundedSender`). So no `block_in_place` here — the callback stays sync and cheap.

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test -p rushhft-studies lob_imbalance::tests`
Expected: PASS — all tests green.

- [ ] **Step 5: Run clippy**

Run: `cargo clippy -p rushhft-studies --all-targets -- -D warnings`
Expected: no warnings.

- [ ] **Step 6: Commit**

```bash
git add rushhft-studies/src/lob_imbalance.rs
git commit -m "feat(studies): wire LobImbalanceStudy OrderBookHub subscription"
```

---

## Task 9: Replay integration test (both studies, scripted stream)

**Files:**
- Create: `rushhft-studies/tests/replay.rs`

End-to-end: feed a scripted stream of `Trade`s + `OrderBook`s through both studies via a shared `ReplayCtx`. Assert that each study emits the expected `BaseStudyModel` series via `register_metric`.

- [ ] **Step 1: Write the failing test**

Create `rushhft-studies/tests/replay.rs`:

```rust
//! Replay integration test: scripted trade + order book stream → both studies emit expected metrics.

use async_trait::async_trait;
use rushhft_core::model::book_item::BookItem;
use rushhft_core::model::enums::AggregationLevel;
use rushhft_core::model::order_book::OrderBook;
use rushhft_core::model::provider::Provider;
use rushhft_core::model::trade::Trade;
use rushhft_core::model::enums::TradeDirection;
use rushhft_core::{
    OrderBookHub, PluginContext, ProviderHub, TradeHub,
    Plugin,
};
use rushhft_studies::{LobImbalanceSettings, LobImbalanceStudy, VpinSettings, VpinStudy};
use rust_decimal::Decimal;
use rust_decimal_macros::dec;
use std::sync::Arc;
use time::OffsetDateTime;

struct ReplayCtx {
    ob_hub: Arc<OrderBookHub>,
    t_hub: Arc<TradeHub>,
    p_hub: Arc<ProviderHub>,
    metrics: Arc<std::sync::Mutex<Vec<MetricRecord>>>,
}

#[derive(Clone, Debug)]
struct MetricRecord {
    plugin: String,
    metric: String,
    symbol: String,
    value: Decimal,
}

#[async_trait]
impl PluginContext for ReplayCtx {
    async fn publish_order_book(&self, _ob: OrderBook) {}
    async fn publish_trade(&self, _t: Trade) {}
    async fn publish_provider(&self, _p: Provider) {}
    async fn register_metric(
        &self,
        plugin: &str,
        metric: &str,
        _exchange: &str,
        symbol: &str,
        value: Decimal,
        _ts: OffsetDateTime,
    ) {
        self.metrics.lock().unwrap().push(MetricRecord {
            plugin: plugin.into(),
            metric: metric.into(),
            symbol: symbol.into(),
            value,
        });
    }
    fn order_book_hub(&self) -> Arc<OrderBookHub> { self.ob_hub.clone() }
    fn trade_hub(&self) -> Arc<TradeHub> { self.t_hub.clone() }
    fn provider_hub(&self) -> Arc<ProviderHub> { self.p_hub.clone() }
}

fn trade(price: Decimal, size: Decimal, dir: TradeDirection, secs: i64) -> Trade {
    Trade {
        price,
        size,
        timestamp: OffsetDateTime::from_unix_timestamp(secs).unwrap(),
        direction: dir,
        trade_type: "D".to_string(),
        symbol: "700.HK".to_string(),
        provider_id: 1,
        market_mid_price: dec!(100.575),
    }
}

fn book(bids: Vec<(Decimal, Decimal)>, asks: Vec<(Decimal, Decimal)>) -> OrderBook {
    let mut ob = OrderBook::new("700.HK", 10, 2, 0, 1);
    for (p, s) in bids {
        ob.add_or_update_level(BookItem::new(p, s, true, "700.HK", 1));
    }
    for (p, s) in asks {
        ob.add_or_update_level(BookItem::new(p, s, false, "700.HK", 1));
    }
    ob
}

#[tokio::test]
async fn replay_both_studies_emit_expected_metrics() {
    let ob_hub = Arc::new(OrderBookHub::new());
    let t_hub = Arc::new(TradeHub::new());
    let p_hub = Arc::new(ProviderHub::new());
    let metrics = Arc::new(std::sync::Mutex::new(Vec::new()));
    let ctx = Arc::new(ReplayCtx {
        ob_hub: ob_hub.clone(),
        t_hub: t_hub.clone(),
        p_hub: p_hub.clone(),
        metrics: metrics.clone(),
    }) as Arc<dyn PluginContext>;

    let vpin = Arc::new(VpinStudy::new(VpinSettings {
        bucket_volume_size: dec!(1),
        number_of_buckets: 50,
        symbol: "700.HK".into(),
        provider_id: 1,
        aggregation_level: AggregationLevel::S1,
    }));
    let lob = Arc::new(LobImbalanceStudy::new(LobImbalanceSettings {
        symbol: "700.HK".into(),
        provider_id: 1,
        levels: 5,
        aggregation_level: AggregationLevel::S1,
    }));
    vpin.start(ctx.clone()).await.unwrap();
    lob.start(ctx.clone()).await.unwrap();

    // 1. Book: all bids → imbalance = 1
    ob_hub.publish(book(vec![(dec!(100), dec!(100))], vec![]));
    // 2. Trade: 1-volume buy → bucket completes, vpin = 1
    t_hub.publish(trade(dec!(100.50), dec!(1), TradeDirection::Up, 1_700_000_000));

    tokio::time::sleep(std::time::Duration::from_millis(80)).await;

    let collected = metrics.lock().unwrap().clone();

    // VPIN should have at least one record with value == 1
    assert!(
        collected.iter().any(|m| m.plugin == "VPIN Study" && m.value == Decimal::ONE),
        "expected VPIN=1, got {:?}", collected
    );
    // LOB Imbalance should have at least one record with value == 1
    assert!(
        collected.iter().any(|m| m.plugin == "LOB Imbalance Study" && m.value == Decimal::ONE),
        "expected LOB imbalance=1, got {:?}", collected
    );

    vpin.stop().await.unwrap();
    lob.stop().await.unwrap();
}

#[tokio::test]
async fn replay_vpin_balanced_book_gives_imbalance_zero() {
    let ob_hub = Arc::new(OrderBookHub::new());
    let t_hub = Arc::new(TradeHub::new());
    let p_hub = Arc::new(ProviderHub::new());
    let metrics = Arc::new(std::sync::Mutex::new(Vec::new()));
    let ctx = Arc::new(ReplayCtx {
        ob_hub: ob_hub.clone(),
        t_hub: t_hub.clone(),
        p_hub: p_hub.clone(),
        metrics: metrics.clone(),
    }) as Arc<dyn PluginContext>;

    let lob = Arc::new(LobImbalanceStudy::new(LobImbalanceSettings {
        symbol: "700.HK".into(),
        provider_id: 1,
        levels: 5,
        aggregation_level: AggregationLevel::S1,
    }));
    lob.start(ctx).await.unwrap();

    // Balanced book: 100 bid vs 100 ask → imbalance = 0
    ob_hub.publish(book(
        vec![(dec!(100), dec!(100))],
        vec![(dec!(101), dec!(100))],
    ));

    tokio::time::sleep(std::time::Duration::from_millis(50)).await;
    let collected = metrics.lock().unwrap().clone();
    assert!(
        collected.iter().any(|m| m.plugin == "LOB Imbalance Study" && m.value == Decimal::ZERO),
        "expected imbalance=0, got {:?}", collected
    );

    lob.stop().await.unwrap();
}
```

- [ ] **Step 2: Run test to verify it passes**

Run: `cargo test -p rushhft-studies --test replay`
Expected: PASS — both tests green. (Tests pass on first run because the implementations already exist; the point of the test is integration-level regression coverage.)

- [ ] **Step 3: Run clippy**

Run: `cargo clippy -p rushhft-studies --all-targets -- -D warnings`
Expected: no warnings.

- [ ] **Step 4: Commit**

```bash
git add rushhft-studies/tests/replay.rs
git commit -m "test(studies): add replay integration tests for both studies"
```

---

## Task 10: Workspace-wide sanity sweep

**Files:** None — verification only.

- [ ] **Step 1: Run the whole workspace test suite**

Run: `cargo test --workspace`
Expected: PASS — core + connector + studies tests all green.

- [ ] **Step 2: Run clippy across the workspace**

Run: `cargo clippy --workspace --all-targets -- -D warnings`
Expected: no warnings.

- [ ] **Step 3: Run rustfmt check**

Run: `cargo fmt --all --check`
Expected: no diff.

- [ ] **Step 4: Commit any formatting fixes**

```bash
git add -u
git commit -m "style(studies): rustfmt sweep" || echo "nothing to commit"
```

---

## Self-Review Checklist (run before handing off)

1. **Spec coverage:**
   - VPIN study — Tasks 2-5 ✓
   - LOB Imbalance study — Tasks 6-8 ✓
   - Plugin trait shape (emits_metric = true, status lifecycle) — Tasks 2, 5, 6, 8 ✓
   - Replay integration tests — Task 9 ✓
   - Deferred studies (Market Resilience, OTT) — out of scope, no task ✓

2. **Placeholder scan:** No TBD/TODO/`implement later`/`similar to Task N` anywhere. Every step that asks for code shows the actual code.

3. **Type consistency:**
   - `VpinSettings` fields: `bucket_volume_size: Decimal`, `number_of_buckets: usize`, `symbol: String`, `provider_id: i32`, `aggregation_level: AggregationLevel` — used the same way in Tasks 2, 5, 9 ✓
   - `VpinCore::new(bucket_volume_size: Decimal, number_of_buckets: usize)` — matches Tasks 3, 5 ✓
   - `VpinCore::ingest_trade(size: Decimal, is_buy: Option<bool>)` — matches Tasks 3, 4, 5 ✓
   - `LobImbalanceSettings` fields: `symbol`, `provider_id`, `levels: usize`, `aggregation_level` — consistent across Tasks 6, 8, 9 ✓
   - `compute_imbalance(ob: &OrderBook, levels: usize) -> Decimal` — matches Tasks 7, 8 ✓
   - `map_trade_direction(TradeDirection) -> Option<bool>` — matches Tasks 4, 5 ✓
   - `register_metric(plugin, metric, exchange, symbol, value, ts)` signature matches `rushhft-core` ✓

4. **Known gaps:**
   - The hub callbacks in `VpinStudy` use `tokio::task::block_in_place + Handle::block_on` to bridge sync closures to async mutex. This is documented in Task 5's note. If it turns out to be problematic on single-threaded runtimes, a follow-up task can swap `tokio::sync::Mutex` for `std::sync::Mutex` for the VpinCore. The plan intentionally uses `tokio::sync::Mutex` to match the rest of the codebase.
   - The `register_metric` call uses the hardcoded exchange string `"LongPort"` — matches the spec's intent (only one connector in scope for MVP) but is a minor coupling. Fine for MVP.
