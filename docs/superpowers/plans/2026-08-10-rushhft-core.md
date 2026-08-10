# RushHFT Core Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Build the `rushhft-core` crate — the shared library containing domain models, lock-free pub/sub hubs, object pools, plugin trait + base implementations, trigger engine, and settings — that all other RushHFT crates depend on.

**Architecture:** A pure-Rust library crate with no Tauri or connector dependencies. Models use `rust_decimal::Decimal` for prices/sizes and `time::OffsetDateTime` for timestamps. Pub/sub hubs use `arc_swap::ArcSwap` for lock-free subscriber lists and `dashmap::DashMap` for per-symbol storage. The trigger engine is a direct port of VisualHFT's `TriggerEngineService.cs` with `tokio::sync::mpsc` replacing C# `Channel<T>`.

**Tech Stack:** Rust (edition 2024), `rust_decimal`, `time`, `tokio`, `async-trait`, `arc-swap`, `dashmap`, `crossbeam-queue`, `thiserror`, `tracing`, `serde` + `toml`, `dirs`

---

## File Structure

```
/Cargo.toml                              # workspace root (members = ["rushhft-core"] for now)
/rust-toolchain.toml
/rushhft-core/
├── Cargo.toml
├── src/
│   ├── lib.rs                            # root module, re-exports
│   ├── model/
│   │   ├── mod.rs                        # module declarations
│   │   ├── enums.rs                      # SessionStatus, TradeDirection, PluginType, etc.
│   │   ├── book_item.rs                  # BookItem struct
│   │   ├── order_book.rs                 # OrderBook struct + add/update/delete/imbalance/delta
│   │   ├── trade.rs                      # Trade struct
│   │   ├── provider.rs                   # Provider struct
│   │   └── study.rs                      # BaseStudyModel struct
│   ├── pool/
│   │   ├── mod.rs                        # module declarations + re-exports
│   │   ├── object_pool.rs                # ObjectPool<T>, PoolGuard<T>
│   │   └── rolling_window.rs             # RollingWindow (Decimal ring buffer)
│   ├── hub/
│   │   ├── mod.rs                        # OrderBookHub, TradeHub, ProviderHub, SubscriptionGuard
│   ├── plugin/
│   │   ├── mod.rs                        # Plugin trait, PluginContext trait, PluginStatus enum
│   │   ├── base_data_retriever.rs        # reconnection orchestration
│   │   └── base_study.rs                 # BaseStudy + AggregatedCollection
│   ├── trigger/
│   │   └── mod.rs                        # TriggerEngine + all trigger types
│   └── settings/
│       └── mod.rs                        # Settings (TOML load/save)
```

**Design notes:**

- Delta counters on `OrderBook` use `u64` (not `AtomicU64` as in the spec). Rationale: in Rust, `OrderBook` is mutated via `&mut self` (exclusive access in the connector's `DashMap::get_mut`) and published via `Arc<OrderBook>` (immutable shared). `AtomicU64` would prevent `Clone` and gains nothing — there's no `&self` mutation path. The `u64` fields are `Copy`, so `OrderBook` can derive `Clone` for the hub publication path.
- `RollingWindow` is specialized for `Decimal` (the only numeric type we need). The spec says `RollingWindow<T: Copy + Default>` but making it fully generic over arithmetic adds complexity with no MVP benefit.
- `AggregatedCollection` is folded into `base_study.rs` rather than a separate file — it's a small helper struct used only by `BaseStudy`.

---

### Task 1: Workspace bootstrap

**Files:**
- Create: `/Cargo.toml` (workspace root)
- Create: `/rust-toolchain.toml`
- Create: `/rushhft-core/Cargo.toml`
- Create: `/rushhft-core/src/lib.rs`

- [ ] **Step 1: Create workspace root Cargo.toml**

```toml
# /Cargo.toml
[workspace]
resolver = "3"
members = ["rushhft-core"]

[workspace.package]
edition = "2024"
license = "Apache-2.0"
version = "0.1.0"
```

- [ ] **Step 2: Create rust-toolchain.toml**

```toml
# /rust-toolchain.toml
[toolchain]
channel = "stable"
components = ["rustfmt", "clippy"]
```

- [ ] **Step 3: Create rushhft-core/Cargo.toml**

```toml
# /rushhft-core/Cargo.toml
[package]
name = "rushhft-core"
version = { workspace = true }
edition = { workspace = true }
license = { workspace = true }

[dependencies]
rust_decimal = "1"
rust_decimal_macros = "1"
time = { version = "0.3", features = ["serde-human-readable", "formatting", "macros"] }
tokio = { version = "1", features = ["rt", "macros", "sync", "time"] }
async-trait = "0.1"
tracing = "0.1"
thiserror = "1"
dashmap = "6"
arc-swap = "1"
crossbeam-queue = "0.3"
dirs = "5"
serde = { version = "1", features = ["derive"] }
toml = "0.8"
```

- [ ] **Step 4: Create lib.rs (empty root)**

```rust
// /rushhft-core/src/lib.rs
```

(Empty file — modules added in later tasks.)

- [ ] **Step 5: Verify it compiles**

Run: `cargo check`
Expected: compiles with no errors (may show "unused" warnings — that's fine).

- [ ] **Step 6: Commit**

```bash
git add Cargo.toml rust-toolchain.toml rushhft-core/Cargo.toml rushhft-core/src/lib.rs
git commit -m "build(core): workspace bootstrap + rushhft-core crate skeleton"
```

---

### Task 2: Enums

**Files:**
- Create: `/rushhft-core/src/model/mod.rs`
- Create: `/rushhft-core/src/model/enums.rs`
- Modify: `/rushhft-core/src/lib.rs`

- [ ] **Step 1: Write the failing test**

Create `/rushhft-core/src/model/enums.rs`:

```rust
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum SessionStatus {
    Connecting,
    Connected,
    ConnectedWithWarnings,
    DisconnectedFailed,
    Disconnected,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum TradeDirection {
    Neutral,
    Down,
    Up,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum LobSide {
    None,
    Bid,
    Ask,
    Both,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum PluginType {
    Unknown,
    Study,
    MultiStudy,
    MarketConnector,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum PluginStatus {
    Loaded,
    Starting,
    Started,
    Stopping,
    Stopped,
    StoppedFailed,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum MdUpdateAction {
    New,
    Change,
    Delete,
    ChangeAdjust,
    Replace,
    None,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum AggregationLevel {
    None,
    Ms1,
    Ms10,
    Ms100,
    Ms500,
    S1,
    S3,
    S5,
    D1,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn session_status_serializes() {
        let json = serde_json::to_string(&SessionStatus::Connected).unwrap();
        assert_eq!(json, "\"Connected\"");
    }

    #[test]
    fn trade_direction_deserializes() {
        let dir: TradeDirection = serde_json::from_str("\"Up\"").unwrap();
        assert_eq!(dir, TradeDirection::Up);
    }

    #[test]
    fn plugin_status_all_variants() {
        let statuses = vec![
            PluginStatus::Loaded,
            PluginStatus::Starting,
            PluginStatus::Started,
            PluginStatus::Stopping,
            PluginStatus::Stopped,
            PluginStatus::StoppedFailed,
        ];
        for s in &statuses {
            let json = serde_json::to_string(s).unwrap();
            let back: PluginStatus = serde_json::from_str(&json).unwrap();
            assert_eq!(*s, back);
        }
    }

    #[test]
    fn aggregation_level_roundtrip() {
        for level in [
            AggregationLevel::None,
            AggregationLevel::S1,
            AggregationLevel::Ms100,
            AggregationLevel::D1,
        ] {
            let json = serde_json::to_string(&level).unwrap();
            let back: AggregationLevel = serde_json::from_str(&json).unwrap();
            assert_eq!(level, back);
        }
    }
}
```

Create `/rushhft-core/src/model/mod.rs`:

```rust
pub mod enums;
```

Update `/rushhft-core/src/lib.rs`:

```rust
pub mod model;
```

- [ ] **Step 2: Run tests to verify they pass (serde is already a dep)**

Run: `cargo test --lib -p rushhft-core model::enums`
Expected: PASS (4 tests)

Note: tests pass immediately because we wrote the implementation alongside. For subsequent tasks, we'll follow stricter TDD — test first, then implement. The enums are trivial data types where the test + implementation are naturally co-located.

- [ ] **Step 3: Commit**

```bash
git add rushhft-core/src/model/ rushhft-core/src/lib.rs
git commit -m "feat(core): add domain enums (SessionStatus, TradeDirection, etc.)"
```

---

### Task 3: BookItem model

**Files:**
- Create: `/rushhft-core/src/model/book_item.rs`
- Modify: `/rushhft-core/src/model/mod.rs`

- [ ] **Step 1: Write the failing test**

Create `/rushhft-core/src/model/book_item.rs`:

```rust
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
```

- [ ] **Step 2: Update model/mod.rs**

```rust
pub mod book_item;
pub mod enums;
```

- [ ] **Step 3: Run tests to verify they pass**

Run: `cargo test --lib -p rushhft-core model::book_item`
Expected: PASS (2 tests)

- [ ] **Step 4: Commit**

```bash
git add rushhft-core/src/model/book_item.rs rushhft-core/src/model/mod.rs
git commit -m "feat(core): add BookItem model"
```

---

### Task 4: OrderBook — construction + basic structure

**Files:**
- Create: `/rushhft-core/src/model/order_book.rs`
- Modify: `/rushhft-core/src/model/mod.rs`

- [ ] **Step 1: Write the failing test**

Create `/rushhft-core/src/model/order_book.rs`:

```rust
use crate::model::book_item::BookItem;
use rust_decimal::Decimal;
use time::OffsetDateTime;

#[derive(Debug, Clone)]
pub struct OrderBook {
    pub symbol: String,
    pub bids: Vec<BookItem>,
    pub asks: Vec<BookItem>,
    pub max_depth: usize,
    pub price_decimal_places: u8,
    pub size_decimal_places: u8,
    pub provider_id: i32,
    pub sequence: i64,
    pub last_updated: OffsetDateTime,
    pub imbalance_value: Decimal,
    pub added_levels: u64,
    pub deleted_levels: u64,
    pub updated_levels: u64,
    pub added_volume_scaled: u64,
    pub deleted_volume_scaled: u64,
}

impl OrderBook {
    pub fn new(
        symbol: &str,
        max_depth: usize,
        price_decimal_places: u8,
        size_decimal_places: u8,
        provider_id: i32,
    ) -> Self {
        Self {
            symbol: symbol.to_string(),
            bids: Vec::new(),
            asks: Vec::new(),
            max_depth,
            price_decimal_places,
            size_decimal_places,
            provider_id,
            sequence: 0,
            last_updated: OffsetDateTime::now_utc(),
            imbalance_value: Decimal::ZERO,
            added_levels: 0,
            deleted_levels: 0,
            updated_levels: 0,
            added_volume_scaled: 0,
            deleted_volume_scaled: 0,
        }
    }

    pub fn compute_scale(&self) -> i64 {
        let mut scale: i64 = 1;
        for _ in 0..self.size_decimal_places {
            scale *= 10;
        }
        scale
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rust_decimal_macros::dec;

    #[test]
    fn new_order_book_is_empty() {
        let ob = OrderBook::new("700.HK", 10, 2, 0, 1);
        assert_eq!(ob.symbol, "700.HK");
        assert!(ob.bids.is_empty());
        assert!(ob.asks.is_empty());
        assert_eq!(ob.max_depth, 10);
        assert_eq!(ob.sequence, 0);
        assert_eq!(ob.imbalance_value, Decimal::ZERO);
        assert_eq!(ob.added_levels, 0);
    }

    #[test]
    fn compute_scale_with_zero_decimal_places() {
        let ob = OrderBook::new("700.HK", 10, 2, 0, 1);
        assert_eq!(ob.compute_scale(), 1);
    }

    #[test]
    fn compute_scale_with_two_decimal_places() {
        let ob = OrderBook::new("700.HK", 10, 2, 2, 1);
        assert_eq!(ob.compute_scale(), 100);
    }
}
```

- [ ] **Step 2: Update model/mod.rs**

```rust
pub mod book_item;
pub mod enums;
pub mod order_book;
```

- [ ] **Step 3: Run tests to verify they pass**

Run: `cargo test --lib -p rushhft-core model::order_book`
Expected: PASS (3 tests)

- [ ] **Step 4: Commit**

```bash
git add rushhft-core/src/model/order_book.rs rushhft-core/src/model/mod.rs
git commit -m "feat(core): add OrderBook struct with construction + compute_scale"
```

---

### Task 5: OrderBook — add_or_update_level

**Files:**
- Modify: `/rushhft-core/src/model/order_book.rs`

- [ ] **Step 1: Write the failing tests**

Add to the `#[cfg(test)] mod tests` block in `order_book.rs`:

```rust
    use crate::model::book_item::BookItem;

    #[test]
    fn add_bid_level_inserts_in_descending_order() {
        let mut ob = OrderBook::new("700.HK", 10, 2, 0, 1);
        ob.add_or_update_level(BookItem::new(dec!(100.50), dec!(500), true, "700.HK", 1));
        ob.add_or_update_level(BookItem::new(dec!(100.55), dec!(300), true, "700.HK", 1));
        ob.add_or_update_level(BookItem::new(dec!(100.45), dec!(200), true, "700.HK", 1));

        assert_eq!(ob.bids.len(), 3);
        assert_eq!(ob.bids[0].price, dec!(100.55));
        assert_eq!(ob.bids[1].price, dec!(100.50));
        assert_eq!(ob.bids[2].price, dec!(100.45));
        assert_eq!(ob.added_levels, 3);
    }

    #[test]
    fn add_ask_level_inserts_in_ascending_order() {
        let mut ob = OrderBook::new("700.HK", 10, 2, 0, 1);
        ob.add_or_update_level(BookItem::new(dec!(100.60), dec!(400), false, "700.HK", 1));
        ob.add_or_update_level(BookItem::new(dec!(100.55), dec!(200), false, "700.HK", 1));

        assert_eq!(ob.asks.len(), 2);
        assert_eq!(ob.asks[0].price, dec!(100.55));
        assert_eq!(ob.asks[1].price, dec!(100.60));
    }

    #[test]
    fn update_existing_bid_level_changes_size() {
        let mut ob = OrderBook::new("700.HK", 10, 2, 0, 1);
        ob.add_or_update_level(BookItem::new(dec!(100.50), dec!(500), true, "700.HK", 1));
        ob.add_or_update_level(BookItem::new(dec!(100.50), dec!(800), true, "700.HK", 1));

        assert_eq!(ob.bids.len(), 1);
        assert_eq!(ob.bids[0].size, dec!(800));
        assert_eq!(ob.added_levels, 1);
        assert_eq!(ob.updated_levels, 1);
    }

    #[test]
    fn add_level_beyond_max_depth_is_dropped() {
        let mut ob = OrderBook::new("700.HK", 2, 2, 0, 1);
        ob.add_or_update_level(BookItem::new(dec!(100.50), dec!(100), true, "700.HK", 1));
        ob.add_or_update_level(BookItem::new(dec!(100.45), dec!(200), true, "700.HK", 1));
        ob.add_or_update_level(BookItem::new(dec!(100.40), dec!(300), true, "700.HK", 1));

        assert_eq!(ob.bids.len(), 2);
        assert_eq!(ob.bids[0].price, dec!(100.50));
        assert_eq!(ob.bids[1].price, dec!(100.45));
    }
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test --lib -p rushhft-core model::order_book`
Expected: FAIL — `add_or_update_level` method not found.

- [ ] **Step 3: Implement add_or_update_level**

Add this `impl` block (or extend the existing one) in `order_book.rs`:

```rust
impl OrderBook {
    pub fn add_or_update_level(&mut self, mut item: BookItem) {
        let side = if item.is_bid { &mut self.bids } else { &mut self.asks };

        // Find position by price
        let pos = side.iter().position(|l| l.price == item.price);

        match pos {
            Some(idx) => {
                // Update existing level
                let old_size = side[idx].size;
                side[idx].size = item.size;
                side[idx].server_timestamp = item.server_timestamp;
                side[idx].local_timestamp = item.local_timestamp;
                // Merge broker_ids (don't lose existing brokers)
                if !item.broker_ids.is_empty() {
                    let mut merged = std::mem::take(&mut side[idx].broker_ids);
                    for bid in item.broker_ids.drain(..) {
                        if !merged.contains(&bid) {
                            merged.push(bid);
                        }
                    }
                    side[idx].broker_ids = merged;
                }
                self.updated_levels += 1;
                let _ = old_size; // could compute volume delta
            }
            None => {
                // Insert new level in sorted position
                if item.is_bid {
                    // Bids descending
                    let pos = side.iter().position(|l| l.price < item.price);
                    match pos {
                        Some(idx) => side.insert(idx, item),
                        None => side.push(item),
                    }
                } else {
                    // Asks ascending
                    let pos = side.iter().position(|l| l.price > item.price);
                    match pos {
                        Some(idx) => side.insert(idx, item),
                        None => side.push(item),
                    }
                }
                self.added_levels += 1;
                let scaled = (item.size * Decimal::from(self.compute_scale())).to_i64().unwrap_or(0);
                self.added_volume_scaled += scaled as u64;
            }
        }

        // Enforce max_depth
        if side.len() > self.max_depth {
            side.truncate(self.max_depth);
        }

        self.compute_cumulative_sizes();
        self.calculate_metrics();
        self.sequence += 1;
        self.last_updated = OffsetDateTime::now_utc();
    }

    pub fn compute_cumulative_sizes(&mut self) {
        // Bids: cumulative from top (highest price) down
        let mut cum = Decimal::ZERO;
        for level in &mut self.bids {
            cum += level.size;
            level.cumulative_size = cum;
        }
        // Asks: cumulative from top (lowest price) up
        cum = Decimal::ZERO;
        for level in &mut self.asks {
            cum += level.size;
            level.cumulative_size = cum;
        }
    }

    pub fn calculate_metrics(&mut self) {
        self.imbalance_value = self.compute_imbalance();
    }

    fn compute_imbalance(&self) -> Decimal {
        let bid_vol: Decimal = self.bids.iter().map(|l| l.size).sum();
        let ask_vol: Decimal = self.asks.iter().map(|l| l.size).sum();
        let total = bid_vol + ask_vol;
        if total.is_zero() {
            return Decimal::ZERO;
        }
        (bid_vol - ask_vol) / total
    }
}
```

You also need this import at the top of the file:
```rust
use rust_decimal::prelude::ToPrimitive;
```

- [ ] **Step 4: Run tests to verify they pass**

Run: `cargo test --lib -p rushhft-core model::order_book`
Expected: PASS (7 tests)

- [ ] **Step 5: Commit**

```bash
git add rushhft-core/src/model/order_book.rs
git commit -m "feat(core): add OrderBook add_or_update_level + cumulative sizes + imbalance"
```

---

### Task 6: OrderBook — delete_level + snapshots + mid_price + spread

**Files:**
- Modify: `/rushhft-core/src/model/order_book.rs`

- [ ] **Step 1: Write the failing tests**

Add to the test module in `order_book.rs`:

```rust
    #[test]
    fn delete_bid_level_removes_it() {
        let mut ob = OrderBook::new("700.HK", 10, 2, 0, 1);
        ob.add_or_update_level(BookItem::new(dec!(100.50), dec!(500), true, "700.HK", 1));
        ob.add_or_update_level(BookItem::new(dec!(100.45), dec!(200), true, "700.HK", 1));

        ob.delete_level(dec!(100.45), true);

        assert_eq!(ob.bids.len(), 1);
        assert_eq!(ob.bids[0].price, dec!(100.50));
        assert_eq!(ob.deleted_levels, 1);
    }

    #[test]
    fn delete_nonexistent_level_is_noop() {
        let mut ob = OrderBook::new("700.HK", 10, 2, 0, 1);
        ob.add_or_update_level(BookItem::new(dec!(100.50), dec!(500), true, "700.HK", 1));

        ob.delete_level(dec!(999.00), true);

        assert_eq!(ob.bids.len(), 1);
        assert_eq!(ob.deleted_levels, 0);
    }

    #[test]
    fn mid_price_and_spread() {
        let mut ob = OrderBook::new("700.HK", 10, 2, 0, 1);
        ob.add_or_update_level(BookItem::new(dec!(100.50), dec!(500), true, "700.HK", 1));
        ob.add_or_update_level(BookItem::new(dec!(100.60), dec!(300), false, "700.HK", 1));

        assert_eq!(ob.mid_price().unwrap(), dec!(100.55));
        assert_eq!(ob.spread().unwrap(), dec!(0.10));
    }

    #[test]
    fn mid_price_none_when_book_empty() {
        let ob = OrderBook::new("700.HK", 10, 2, 0, 1);
        assert!(ob.mid_price().is_none());
        assert!(ob.spread().is_none());
    }

    #[test]
    fn cumulative_sizes_correct() {
        let mut ob = OrderBook::new("700.HK", 10, 2, 0, 1);
        ob.add_or_update_level(BookItem::new(dec!(100.50), dec!(500), true, "700.HK", 1));
        ob.add_or_update_level(BookItem::new(dec!(100.45), dec!(200), true, "700.HK", 1));

        assert_eq!(ob.bids[0].cumulative_size, dec!(500));
        assert_eq!(ob.bids[1].cumulative_size, dec!(700));
    }

    #[test]
    fn get_bids_snapshot_returns_slice() {
        let mut ob = OrderBook::new("700.HK", 10, 2, 0, 1);
        ob.add_or_update_level(BookItem::new(dec!(100.50), dec!(500), true, "700.HK", 1));

        let snap = ob.get_bids_snapshot();
        assert_eq!(snap.len(), 1);
        assert_eq!(snap[0].price, dec!(100.50));
    }
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test --lib -p rushhft-core model::order_book`
Expected: FAIL — `delete_level`, `mid_price`, `spread`, `get_bids_snapshot` not found.

- [ ] **Step 3: Implement the missing methods**

Add to the `impl OrderBook` block in `order_book.rs`:

```rust
    pub fn delete_level(&mut self, price: Decimal, is_bid: bool) {
        let side = if is_bid { &mut self.bids } else { &mut self.asks };
        let scale = self.compute_scale();

        if let Some(pos) = side.iter().position(|l| l.price == price) {
            let removed = side.remove(pos);
            self.deleted_levels += 1;
            let scaled = (removed.size * Decimal::from(scale)).to_i64().unwrap_or(0);
            self.deleted_volume_scaled += scaled as u64;

            self.compute_cumulative_sizes();
            self.calculate_metrics();
            self.sequence += 1;
            self.last_updated = OffsetDateTime::now_utc();
        }
    }

    pub fn mid_price(&self) -> Option<Decimal> {
        match (self.bids.first(), self.asks.first()) {
            (Some(bid), Some(ask)) => Some((bid.price + ask.price) / Decimal::from(2)),
            _ => None,
        }
    }

    pub fn spread(&self) -> Option<Decimal> {
        match (self.bids.first(), self.asks.first()) {
            (Some(bid), Some(ask)) => Some(ask.price - bid.price),
            _ => None,
        }
    }

    pub fn get_bids_snapshot(&self) -> &[BookItem] {
        &self.bids
    }

    pub fn get_asks_snapshot(&self) -> &[BookItem] {
        &self.asks
    }
```

- [ ] **Step 4: Run tests to verify they pass**

Run: `cargo test --lib -p rushhft-core model::order_book`
Expected: PASS (13 tests)

- [ ] **Step 5: Commit**

```bash
git add rushhft-core/src/model/order_book.rs
git commit -m "feat(core): add OrderBook delete_level + mid_price + spread + snapshots"
```

---

### Task 7: Trade + Provider + BaseStudyModel models

**Files:**
- Create: `/rushhft-core/src/model/trade.rs`
- Create: `/rushhft-core/src/model/provider.rs`
- Create: `/rushhft-core/src/model/study.rs`
- Modify: `/rushhft-core/src/model/mod.rs`

- [ ] **Step 1: Write the failing tests**

Create `/rushhft-core/src/model/trade.rs`:

```rust
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
```

Create `/rushhft-core/src/model/provider.rs`:

```rust
use crate::model::enums::SessionStatus;

#[derive(Debug, Clone, PartialEq)]
pub struct Provider {
    pub id: i32,
    pub name: String,
    pub status: SessionStatus,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn provider_construction() {
        let p = Provider {
            id: 1,
            name: "LongPort".to_string(),
            status: SessionStatus::Connected,
        };
        assert_eq!(p.id, 1);
        assert_eq!(p.name, "LongPort");
        assert_eq!(p.status, SessionStatus::Connected);
    }
}
```

Create `/rushhft-core/src/model/study.rs`:

```rust
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
```

- [ ] **Step 2: Update model/mod.rs**

```rust
pub mod book_item;
pub mod enums;
pub mod order_book;
pub mod provider;
pub mod study;
pub mod trade;
```

- [ ] **Step 3: Run tests to verify they pass**

Run: `cargo test --lib -p rushhft-core model`
Expected: PASS (all model tests)

- [ ] **Step 4: Commit**

```bash
git add rushhft-core/src/model/trade.rs rushhft-core/src/model/provider.rs rushhft-core/src/model/study.rs rushhft-core/src/model/mod.rs
git commit -m "feat(core): add Trade, Provider, BaseStudyModel models"
```

---

### Task 8: ObjectPool<T>

**Files:**
- Create: `/rushhft-core/src/pool/mod.rs`
- Create: `/rushhft-core/src/pool/object_pool.rs`
- Modify: `/rushhft-core/src/lib.rs`

- [ ] **Step 1: Write the failing test**

Create `/rushhft-core/src/pool/object_pool.rs`:

```rust
use crossbeam_queue::ArrayQueue;
use std::ops::{Deref, DerefMut};
use std::sync::Arc;

pub struct ObjectPool<T: Default + Clone + Send + Sync> {
    queue: Arc<ArrayQueue<T>>,
}

pub struct PoolGuard<T: Default + Clone + Send + Sync> {
    queue: Arc<ArrayQueue<T>>,
    item: Option<T>,
}

impl<T: Default + Clone + Send + Sync> ObjectPool<T> {
    pub fn new(capacity: usize) -> Self {
        let queue = Arc::new(ArrayQueue::new(capacity));
        for _ in 0..capacity {
            let _ = queue.push(T::default());
        }
        Self { queue }
    }

    pub fn get(&self) -> PoolGuard<T> {
        let item = self.queue.pop().unwrap_or_else(T::default);
        PoolGuard {
            queue: self.queue.clone(),
            item: Some(item),
        }
    }

    pub fn try_get(&self) -> Option<PoolGuard<T>> {
        self.queue
            .pop()
            .map(|item| PoolGuard {
                queue: self.queue.clone(),
                item: Some(item),
            })
    }

    pub fn len(&self) -> usize {
        self.queue.len()
    }
}

impl<T: Default + Clone + Send + Sync> PoolGuard<T> {
    pub fn get(&self) -> &T {
        self.item.as_ref().unwrap()
    }

    pub fn get_mut(&mut self) -> &mut T {
        self.item.as_mut().unwrap()
    }

    pub fn into_inner(mut self) -> T {
        self.item.take().unwrap()
    }
}

impl<T: Default + Clone + Send + Sync> Deref for PoolGuard<T> {
    type Target = T;
    fn deref(&self) -> &Self::Target {
        self.item.as_ref().unwrap()
    }
}

impl<T: Default + Clone + Send + Sync> DerefMut for PoolGuard<T> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        self.item.as_mut().unwrap()
    }
}

impl<T: Default + Clone + Send + Sync> Drop for PoolGuard<T> {
    fn drop(&mut self) {
        if let Some(item) = self.item.take() {
            let _ = self.queue.push(item);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Debug, Default, Clone, PartialEq)]
    struct TestItem {
        value: i32,
    }

    #[test]
    fn get_returns_default_from_pool() {
        let pool: ObjectPool<TestItem> = ObjectPool::new(2);
        let g = pool.get();
        assert_eq!(*g, TestItem { value: 0 });
    }

    #[test]
    fn guard_returns_item_to_pool_on_drop() {
        let pool: ObjectPool<TestItem> = ObjectPool::new(1);
        assert_eq!(pool.len(), 1);
        {
            let _g = pool.get();
            assert_eq!(pool.len(), 0);
        }
        assert_eq!(pool.len(), 1);
    }

    #[test]
    fn into_inner_does_not_return_to_pool() {
        let pool: ObjectPool<TestItem> = ObjectPool::new(1);
        let g = pool.get();
        let item = g.into_inner();
        assert_eq!(item, TestItem { value: 0 });
        assert_eq!(pool.len(), 0);
    }

    #[test]
    fn modified_item_is_returned_to_pool() {
        let pool: ObjectPool<TestItem> = ObjectPool::new(1);
        {
            let mut g = pool.get();
            g.value = 42;
        }
        let g2 = pool.get();
        assert_eq!(g2.value, 42);
    }

    #[test]
    fn try_get_returns_none_when_empty() {
        let pool: ObjectPool<TestItem> = ObjectPool::new(1);
        let _g = pool.get();
        assert!(pool.try_get().is_none());
    }
}
```

Create `/rushhft-core/src/pool/mod.rs`:

```rust
pub mod object_pool;

pub use object_pool::{ObjectPool, PoolGuard};
```

Update `/rushhft-core/src/lib.rs`:

```rust
pub mod model;
pub mod pool;
```

- [ ] **Step 2: Run tests to verify they pass**

Run: `cargo test --lib -p rushhft-core pool`
Expected: PASS (5 tests)

- [ ] **Step 3: Commit**

```bash
git add rushhft-core/src/pool/ rushhft-core/src/lib.rs
git commit -m "feat(core): add ObjectPool<T> with RAII PoolGuard"
```

---

### Task 9: RollingWindow

**Files:**
- Create: `/rushhft-core/src/pool/rolling_window.rs`
- Modify: `/rushhft-core/src/pool/mod.rs`

- [ ] **Step 1: Write the failing test**

Create `/rushhft-core/src/pool/rolling_window.rs`:

```rust
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
            // Subtract the oldest value being overwritten
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
```

- [ ] **Step 2: Update pool/mod.rs**

```rust
pub mod object_pool;
pub mod rolling_window;

pub use object_pool::{ObjectPool, PoolGuard};
pub use rolling_window::RollingWindow;
```

- [ ] **Step 3: Run tests to verify they pass**

Run: `cargo test --lib -p rushhft-core pool::rolling_window`
Expected: PASS (5 tests)

- [ ] **Step 4: Commit**

```bash
git add rushhft-core/src/pool/rolling_window.rs rushhft-core/src/pool/mod.rs
git commit -m "feat(core): add RollingWindow with O(1) push + sliding avg"
```

---

### Task 10: SubscriptionGuard + hub basics

**Files:**
- Create: `/rushhft-core/src/hub/mod.rs`
- Modify: `/rushhft-core/src/lib.rs`

- [ ] **Step 1: Write the failing tests**

Create `/rushhft-core/src/hub/mod.rs`:

```rust
use crate::model::order_book::OrderBook;
use crate::model::provider::Provider;
use crate::model::trade::Trade;
use arc_swap::ArcSwap;
use dashmap::DashMap;
use std::panic::{catch_unwind, AssertUnwindSafe};
use std::sync::Arc;

type Subscriber<T> = Arc<dyn Fn(&T) + Send + Sync>;

pub struct SubscriptionGuard {
    unsubscribe: Option<Box<dyn FnOnce() + Send + Sync>>,
}

impl SubscriptionGuard {
    fn new(unsubscribe: Box<dyn FnOnce() + Send + Sync>) -> Self {
        Self {
            unsubscribe: Some(unsubscribe),
        }
    }
}

impl Drop for SubscriptionGuard {
    fn drop(&mut self) {
        if let Some(f) = self.unsubscribe.take() {
            f();
        }
    }
}

pub struct OrderBookHub {
    subscribers: ArcSwap<Vec<Subscriber<OrderBook>>>,
    latest: DashMap<String, ArcSwap<OrderBook>>,
}

impl OrderBookHub {
    pub fn new() -> Self {
        Self {
            subscribers: ArcSwap::from_pointee(Vec::new()),
            latest: DashMap::new(),
        }
    }

    pub fn subscribe(&self, f: Subscriber<OrderBook>) -> SubscriptionGuard {
        self.add_subscriber(f.clone());
        let subs = self.subscribers.clone();
        SubscriptionGuard::new(Box::new(move || {
            remove_subscriber(&subs, &f);
        }))
    }

    fn add_subscriber(&self, f: Subscriber<OrderBook>) {
        loop {
            let current = self.subscribers.load();
            let mut new_list = (**current).clone();
            new_list.push(f.clone());
            match self.subscribers.compare_exchange(current, new_list) {
                Ok(_) => break,
                Err(_) => continue,
            }
        }
    }

    pub fn publish(&self, ob: OrderBook) {
        let arc = Arc::new(ob.clone());
        let symbol = arc.symbol.clone();

        // Update latest snapshot
        self.latest
            .entry(symbol.clone())
            .or_insert_with(|| ArcSwap::from_pointee(ob.clone()));
        if let Some(entry) = self.latest.get(&symbol) {
            entry.store(arc.clone());
        }

        // Fan out to subscribers
        let subs = self.subscribers.load();
        for sub in subs.iter() {
            let sub = sub.clone();
            let arc = arc.clone();
            let _ = catch_unwind(AssertUnwindSafe(move || sub(&arc)));
        }
    }

    pub fn snapshot(&self, symbol: &str) -> Option<Arc<OrderBook>> {
        self.latest.get(symbol).map(|e| e.load_full())
    }

    pub fn symbols(&self) -> Vec<String> {
        self.latest.iter().map(|e| e.key().clone()).collect()
    }
}

impl Default for OrderBookHub {
    fn default() -> Self {
        Self::new()
    }
}

fn remove_subscriber(
    subs: &ArcSwap<Vec<Subscriber<OrderBook>>>,
    target: &Subscriber<OrderBook>,
) {
    loop {
        let current = subs.load();
        let mut new_list = (**current).clone();
        new_list.retain(|s| !Arc::ptr_eq(s, target));
        match subs.compare_exchange(current, new_list) {
            Ok(_) => break,
            Err(_) => continue,
        }
    }
}

// --- TradeHub ---

pub struct TradeHub {
    subscribers: ArcSwap<Vec<Subscriber<Trade>>>,
    latest: DashMap<String, Vec<Trade>>,
}

impl TradeHub {
    pub fn new() -> Self {
        Self {
            subscribers: ArcSwap::from_pointee(Vec::new()),
            latest: DashMap::new(),
        }
    }

    pub fn subscribe(&self, f: Subscriber<Trade>) -> SubscriptionGuard {
        self.add_subscriber(f.clone());
        let subs = self.subscribers.clone();
        SubscriptionGuard::new(Box::new(move || {
            loop {
                let current = subs.load();
                let mut new_list = (**current).clone();
                new_list.retain(|s: &Subscriber<Trade>| !Arc::ptr_eq(s, &f));
                match subs.compare_exchange(current, new_list) {
                    Ok(_) => break,
                    Err(_) => continue,
                }
            }
        }))
    }

    fn add_subscriber(&self, f: Subscriber<Trade>) {
        loop {
            let current = self.subscribers.load();
            let mut new_list = (**current).clone();
            new_list.push(f.clone());
            match self.subscribers.compare_exchange(current, new_list) {
                Ok(_) => break,
                Err(_) => continue,
            }
        }
    }

    pub fn publish(&self, t: Trade) {
        let symbol = t.symbol.clone();
        self.latest
            .entry(symbol.clone())
            .or_insert_with(Vec::new)
            .push(t.clone());

        // Cap recent trades at 200
        if let Some(mut entry) = self.latest.get_mut(&symbol) {
            if entry.len() > 200 {
                let drain_from = entry.len() - 200;
                entry.drain(0..drain_from);
            }
        }

        let subs = self.subscribers.load();
        for sub in subs.iter() {
            let sub = sub.clone();
            let _ = catch_unwind(AssertUnwindSafe(move || sub(&t)));
        }
    }

    pub fn recent_trades(&self, symbol: &str) -> Vec<Trade> {
        self.latest.get(symbol).map(|e| e.clone()).unwrap_or_default()
    }
}

impl Default for TradeHub {
    fn default() -> Self {
        Self::new()
    }
}

// --- ProviderHub ---

pub struct ProviderHub {
    subscribers: ArcSwap<Vec<Subscriber<Provider>>>,
    latest: ArcSwap<Vec<Provider>>,
}

impl ProviderHub {
    pub fn new() -> Self {
        Self {
            subscribers: ArcSwap::from_pointee(Vec::new()),
            latest: ArcSwap::from_pointee(Vec::new()),
        }
    }

    pub fn subscribe(&self, f: Subscriber<Provider>) -> SubscriptionGuard {
        self.add_subscriber(f.clone());
        let subs = self.subscribers.clone();
        SubscriptionGuard::new(Box::new(move || {
            loop {
                let current = subs.load();
                let mut new_list = (**current).clone();
                new_list.retain(|s: &Subscriber<Provider>| !Arc::ptr_eq(s, &f));
                match subs.compare_exchange(current, new_list) {
                    Ok(_) => break,
                    Err(_) => continue,
                }
            }
        }))
    }

    fn add_subscriber(&self, f: Subscriber<Provider>) {
        loop {
            let current = self.subscribers.load();
            let mut new_list = (**current).clone();
            new_list.push(f.clone());
            match self.subscribers.compare_exchange(current, new_list) {
                Ok(_) => break,
                Err(_) => continue,
            }
        }
    }

    pub fn publish(&self, p: Provider) {
        // Update latest list
        let current = self.latest.load();
        let mut new_list: Vec<Provider> = current
            .iter()
            .filter(|x| x.id != p.id)
            .cloned()
            .collect();
        new_list.push(p.clone());
        self.latest.store(Arc::new(new_list));

        let subs = self.subscribers.load();
        for sub in subs.iter() {
            let sub = sub.clone();
            let _ = catch_unwind(AssertUnwindSafe(move || sub(&p)));
        }
    }

    pub fn providers(&self) -> Vec<Provider> {
        (**self.latest.load()).clone()
    }
}

impl Default for ProviderHub {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::book_item::BookItem;
    use rust_decimal_macros::dec;
    use std::sync::atomic::{AtomicU32, Ordering};
    use std::sync::Mutex;

    #[test]
    fn order_book_hub_subscribe_and_publish() {
        let hub = OrderBookHub::new();
        let call_count = Arc::new(AtomicU32::new(0));
        let cc = call_count.clone();
        let _guard = hub.subscribe(Arc::new(move |_ob| {
            cc.fetch_add(1, Ordering::Relaxed);
        }));

        let mut ob = OrderBook::new("700.HK", 10, 2, 0, 1);
        ob.add_or_update_level(BookItem::new(dec!(100.50), dec!(500), true, "700.HK", 1));
        hub.publish(ob);

        assert_eq!(call_count.load(Ordering::Relaxed), 1);
    }

    #[test]
    fn subscription_guard_unsubscribes_on_drop() {
        let hub = OrderBookHub::new();
        let call_count = Arc::new(AtomicU32::new(0));
        let cc = call_count.clone();
        {
            let _guard = hub.subscribe(Arc::new(move |_| {
                cc.fetch_add(1, Ordering::Relaxed);
            }));
        }

        let ob = OrderBook::new("TEST.HK", 10, 2, 0, 1);
        hub.publish(ob);

        assert_eq!(call_count.load(Ordering::Relaxed), 0);
    }

    #[test]
    fn order_book_hub_snapshot_returns_latest() {
        let hub = OrderBookHub::new();
        let mut ob = OrderBook::new("700.HK", 10, 2, 0, 1);
        ob.add_or_update_level(BookItem::new(dec!(100.50), dec!(500), true, "700.HK", 1));
        hub.publish(ob);

        let snap = hub.snapshot("700.HK").unwrap();
        assert_eq!(snap.symbol, "700.HK");
        assert_eq!(snap.bids.len(), 1);
    }

    #[test]
    fn panicking_subscriber_does_not_break_fanout() {
        let hub = OrderBookHub::new();
        let call_count = Arc::new(AtomicU32::new(0));
        let cc = call_count.clone();

        let _g1 = hub.subscribe(Arc::new(move |_| {
            panic!("boom");
        }));
        let _g2 = hub.subscribe(Arc::new(move |_| {
            cc.fetch_add(1, Ordering::Relaxed);
        }));

        let ob = OrderBook::new("TEST.HK", 10, 2, 0, 1);
        hub.publish(ob);

        assert_eq!(call_count.load(Ordering::Relaxed), 1);
    }

    #[test]
    fn trade_hub_publish_and_recent_trades() {
        let hub = TradeHub::new();
        let t1 = Trade {
            price: dec!(100.00),
            size: dec!(50),
            timestamp: OffsetDateTime::now_utc(),
            direction: crate::model::enums::TradeDirection::Up,
            trade_type: "D".to_string(),
            symbol: "700.HK".to_string(),
            provider_id: 1,
            market_mid_price: dec!(99.95),
        };
        hub.publish(t1.clone());

        let trades = hub.recent_trades("700.HK");
        assert_eq!(trades.len(), 1);
        assert_eq!(trades[0].price, dec!(100.00));
    }

    #[test]
    fn provider_hub_publish_and_list() {
        let hub = ProviderHub::new();
        hub.publish(Provider {
            id: 1,
            name: "LongPort".to_string(),
            status: crate::model::enums::SessionStatus::Connected,
        });

        let providers = hub.providers();
        assert_eq!(providers.len(), 1);
        assert_eq!(providers[0].name, "LongPort");
    }
}
```

Update `/rushhft-core/src/lib.rs`:

```rust
pub mod hub;
pub mod model;
pub mod pool;
```

- [ ] **Step 2: Run tests to verify they pass**

Run: `cargo test --lib -p rushhft-core hub`
Expected: PASS (6 tests)

- [ ] **Step 3: Commit**

```bash
git add rushhft-core/src/hub/mod.rs rushhft-core/src/lib.rs
git commit -m "feat(core): add OrderBookHub, TradeHub, ProviderHub with lock-free subscribers"
```

---

### Task 11: Plugin trait + PluginContext trait

**Files:**
- Create: `/rushhft-core/src/plugin/mod.rs`
- Modify: `/rushhft-core/src/lib.rs`

- [ ] **Step 1: Write the failing tests**

Create `/rushhft-core/src/plugin/mod.rs`:

```rust
use crate::hub::{OrderBookHub, ProviderHub, TradeHub};
use crate::model::enums::{PluginStatus, PluginType};
use crate::model::order_book::OrderBook;
use crate::model::provider::Provider;
use crate::model::trade::Trade;
use async_trait::async_trait;
use rushhft_core_model_reexports::*;
use rust_decimal::Decimal;
use std::sync::Arc;
use time::OffsetDateTime;

// Re-export model types for convenience
mod rushhft_core_model_reexports {
    pub use crate::model::enums::*;
    pub use crate::model::order_book::OrderBook;
    pub use crate::model::provider::Provider;
    pub use crate::model::trade::Trade;
}

pub struct SnapshotStore; // placeholder — full impl in rushhft-app crate

#[async_trait]
pub trait Plugin: Send + Sync {
    fn name(&self) -> &str;
    fn version(&self) -> &str;
    fn author(&self) -> &str {
        "RushHFT"
    }
    fn description(&self) -> &str {
        ""
    }
    fn plugin_type(&self) -> PluginType;
    fn status(&self) -> PluginStatus;
    fn plugin_id(&self) -> &str;
    fn emits_metric(&self) -> bool {
        false
    }
    async fn start(&self, ctx: Arc<dyn PluginContext>) -> Result<(), PluginError>;
    async fn stop(&self) -> Result<(), PluginError>;
}

#[async_trait]
pub trait PluginContext: Send + Sync {
    async fn publish_order_book(&self, ob: OrderBook);
    async fn publish_trade(&self, t: Trade);
    async fn publish_provider(&self, p: Provider);
    async fn register_metric(
        &self,
        plugin: &str,
        metric: &str,
        exchange: &str,
        symbol: &str,
        value: Decimal,
        ts: OffsetDateTime,
    );
    fn order_book_hub(&self) -> Arc<OrderBookHub>;
    fn trade_hub(&self) -> Arc<TradeHub>;
    fn provider_hub(&self) -> Arc<ProviderHub>;
}

#[derive(Debug, thiserror::Error)]
pub enum PluginError {
    #[error("plugin error: {0}")]
    Generic(String),
    #[error("plugin not started: {0}")]
    NotStarted(String),
    #[error("plugin already running: {0}")]
    AlreadyRunning(String),
    #[error("plugin start failed: {0}")]
    StartFailed(String),
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicBool, Ordering};

    struct MockPlugin {
        id: String,
        started: AtomicBool,
    }

    #[async_trait]
    impl Plugin for MockPlugin {
        fn name(&self) -> &str {
            "Mock"
        }
        fn version(&self) -> &str {
            "0.1.0"
        }
        fn plugin_type(&self) -> PluginType {
            PluginType::Study
        }
        fn status(&self) -> PluginStatus {
            if self.started.load(Ordering::Relaxed) {
                PluginStatus::Started
            } else {
                PluginStatus::Stopped
            }
        }
        fn plugin_id(&self) -> &str {
            &self.id
        }
        async fn start(&self, _ctx: Arc<dyn PluginContext>) -> Result<(), PluginError> {
            self.started.store(true, Ordering::Relaxed);
            Ok(())
        }
        async fn stop(&self) -> Result<(), PluginError> {
            self.started.store(false, Ordering::Relaxed);
            Ok(())
        }
    }

    #[async_trait]
    impl PluginContext for MockPlugin {
        async fn publish_order_book(&self, _ob: OrderBook) {}
        async fn publish_trade(&self, _t: Trade) {}
        async fn publish_provider(&self, _p: Provider) {}
        async fn register_metric(
            &self,
            _: &str,
            _: &str,
            _: &str,
            _: &str,
            _: Decimal,
            _: OffsetDateTime,
        ) {
        }
        fn order_book_hub(&self) -> Arc<OrderBookHub> {
            Arc::new(OrderBookHub::new())
        }
        fn trade_hub(&self) -> Arc<TradeHub> {
            Arc::new(TradeHub::new())
        }
        fn provider_hub(&self) -> Arc<ProviderHub> {
            Arc::new(ProviderHub::new())
        }
    }

    #[tokio::test]
    async fn plugin_start_stop_lifecycle() {
        let plugin = MockPlugin {
            id: "mock-1".to_string(),
            started: AtomicBool::new(false),
        };
        let ctx: Arc<dyn PluginContext> = Arc::new(MockPlugin {
            id: "ctx".to_string(),
            started: AtomicBool::new(false),
        });

        assert_eq!(plugin.status(), PluginStatus::Stopped);
        plugin.start(ctx).await.unwrap();
        assert_eq!(plugin.status(), PluginStatus::Started);
        plugin.stop().await.unwrap();
        assert_eq!(plugin.status(), PluginStatus::Stopped);
    }
}
```

- [ ] **Step 2: Update lib.rs**

```rust
pub mod hub;
pub mod model;
pub mod plugin;
pub mod pool;
```

- [ ] **Step 3: Run tests to verify they pass**

Run: `cargo test --lib -p rushhft-core plugin`
Expected: PASS (1 test)

- [ ] **Step 4: Commit**

```bash
git add rushhft-core/src/plugin/mod.rs rushhft-core/src/lib.rs
git commit -m "feat(core): add Plugin trait + PluginContext trait"
```

---

### Task 12: BaseDataRetriever

**Files:**
- Create: `/rushhft-core/src/plugin/base_data_retriever.rs`
- Modify: `/rushhft-core/src/plugin/mod.rs`

- [ ] **Step 1: Write the failing tests**

Create `/rushhft-core/src/plugin/base_data_retriever.rs`:

```rust
use crate::model::enums::PluginStatus;
use crate::plugin::{PluginContext, PluginError};
use std::future::Future;
use std::pin::Pin;
use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};
use std::sync::Arc;
use std::time::Duration;

type BoxFuture<'a> = Pin<Box<dyn Future<Output = Result<(), PluginError>> + Send + 'a>>;

pub struct BaseDataRetriever {
    is_reconnecting: AtomicBool,
    attempt_count: AtomicU32,
    max_attempts: u32,
    base_delay: Duration,
    max_delay: Duration,
}

impl BaseDataRetriever {
    pub fn new(max_attempts: u32, base_delay: Duration, max_delay: Duration) -> Self {
        Self {
            is_reconnecting: AtomicBool::new(false),
            attempt_count: AtomicU32::new(0),
            max_attempts,
            base_delay,
            max_delay,
        }
    }

    pub fn new_default() -> Self {
        Self::new(5, Duration::from_millis(500), Duration::from_secs(30))
    }

    pub fn is_reconnecting(&self) -> bool {
        self.is_reconnecting.load(Ordering::Relaxed)
    }

    pub fn attempt_count(&self) -> u32 {
        self.attempt_count.load(Ordering::Relaxed)
    }

    pub async fn start_with_reconnect<F>(
        &self,
        _ctx: Arc<dyn PluginContext>,
        internal_start: F,
    ) -> Result<(), PluginError>
    where
        F: Fn() -> BoxFuture<'static>,
    {
        // Atomic check-and-set to prevent concurrent reconnection storms
        if self
            .is_reconnecting
            .compare_exchange(false, true, Ordering::SeqCst, Ordering::SeqCst)
            .is_err()
        {
            tracing::warn!("reconnection already in progress, skipping");
            return Ok(());
        }

        self.attempt_count.store(0, Ordering::Relaxed);
        let result = self.reconnect_loop(internal_start).await;

        self.is_reconnecting.store(false, Ordering::Relaxed);
        result
    }

    async fn reconnect_loop<F>(&self, internal_start: F) -> Result<(), PluginError>
    where
        F: Fn() -> BoxFuture<'static>,
    {
        loop {
            let attempt = self.attempt_count.fetch_add(1, Ordering::Relaxed) + 1;

            let result = internal_start().await;

            match result {
                Ok(()) => {
                    self.attempt_count.store(0, Ordering::Relaxed);
                    return Ok(());
                }
                Err(e) => {
                    if attempt >= self.max_attempts {
                        tracing::error!(
                            attempt,
                            max = self.max_attempts,
                            error = %e,
                            "reconnection exhausted"
                        );
                        return Err(PluginError::StartFailed(format!(
                            "after {} attempts: {}",
                            attempt, e
                        )));
                    }

                    let delay = self.backoff_delay(attempt);
                    tracing::warn!(
                        attempt,
                        delay_ms = delay.as_millis(),
                        error = %e,
                        "reconnect attempt failed, backing off"
                    );
                    tokio::time::sleep(delay).await;
                }
            }
        }
    }

    fn backoff_delay(&self, attempt: u32) -> Duration {
        let exp = 2u32.saturating_pow(attempt);
        let base = self.base_delay.as_millis() as u64 * exp as u64;
        let capped = base.min(self.max_delay.as_millis() as u64);
        // Jitter: add 0-10% random
        let jitter = (capped / 10).max(1);
        let total = capped + (jitter); // deterministic for tests; real code would use rand
        Duration::from_millis(total)
    }
}

impl Default for BaseDataRetriever {
    fn default() -> Self {
        Self::new_default()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::AtomicU32;

    fn make_internal_start(
        fail_times: u32,
        counter: Arc<AtomicU32>,
    ) -> impl Fn() -> BoxFuture<'static> {
        move || {
            let c = counter.clone();
            Box::pin(async move {
                let n = c.fetch_add(1, Ordering::Relaxed) + 1;
                if n <= fail_times {
                    Err(PluginError::Generic(format!("fail {}", n)))
                } else {
                    Ok(())
                }
            })
        }
    }

    #[tokio::test]
    async fn succeeds_on_first_try() {
        let retriever = BaseDataRetriever::new(5, Duration::from_millis(1), Duration::from_millis(10));
        let counter = Arc::new(AtomicU32::new(0));
        let f = make_internal_start(0, counter);
        let ctx = MockCtx;
        retriever.start_with_reconnect(Arc::new(ctx), f).await.unwrap();
        assert_eq!(retriever.attempt_count(), 0);
    }

    #[tokio::test]
    async fn retries_then_succeeds() {
        let retriever = BaseDataRetriever::new(5, Duration::from_millis(1), Duration::from_millis(10));
        let counter = Arc::new(AtomicU32::new(0));
        let f = make_internal_start(2, counter);
        let ctx = MockCtx;
        retriever.start_with_reconnect(Arc::new(ctx), f).await.unwrap();
    }

    #[tokio::test]
    async fn fails_after_max_attempts() {
        let retriever = BaseDataRetriever::new(3, Duration::from_millis(1), Duration::from_millis(10));
        let counter = Arc::new(AtomicU32::new(0));
        let f = make_internal_start(99, counter); // always fails
        let ctx = MockCtx;
        let result = retriever.start_with_reconnect(Arc::new(ctx), f).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn concurrent_reconnect_is_skipped() {
        let retriever = Arc::new(BaseDataRetriever::new(5, Duration::from_millis(1), Duration::from_millis(10)));
        // Set the flag manually to simulate an in-progress reconnection
        retriever.is_reconnecting.store(true, Ordering::Relaxed);
        let counter = Arc::new(AtomicU32::new(0));
        let f = make_internal_start(0, counter);
        let ctx = MockCtx;
        retriever.start_with_reconnect(Arc::new(ctx), f).await.unwrap();
        // The internal_start should not have been called (skipped)
        assert_eq!(counter.load(Ordering::Relaxed), 0);
    }

    // --- Mock PluginContext ---

    struct MockCtx;

    #[async_trait::async_trait]
    impl crate::plugin::PluginContext for MockCtx {
        async fn publish_order_book(&self, _: crate::model::order_book::OrderBook) {}
        async fn publish_trade(&self, _: crate::model::trade::Trade) {}
        async fn publish_provider(&self, _: crate::model::provider::Provider) {}
        async fn register_metric(&self, _: &str, _: &str, _: &str, _: &str, _: rust_decimal::Decimal, _: time::OffsetDateTime) {}
        fn order_book_hub(&self) -> Arc<crate::hub::OrderBookHub> { Arc::new(crate::hub::OrderBookHub::new()) }
        fn trade_hub(&self) -> Arc<crate::hub::TradeHub> { Arc::new(crate::hub::TradeHub::new()) }
        fn provider_hub(&self) -> Arc<crate::hub::ProviderHub> { Arc::new(crate::hub::ProviderHub::new()) }
    }
}
```

Update `/rushhft-core/src/plugin/mod.rs` to include the module:

Add at the top of `plugin/mod.rs`:
```rust
pub mod base_data_retriever;

pub use base_data_retriever::BaseDataRetriever;
```

- [ ] **Step 2: Run tests to verify they pass**

Run: `cargo test --lib -p rushhft-core plugin::base_data_retriever`
Expected: PASS (4 tests)

- [ ] **Step 3: Commit**

```bash
git add rushhft-core/src/plugin/base_data_retriever.rs rushhft-core/src/plugin/mod.rs
git commit -m "feat(core): add BaseDataRetriever with exponential backoff + atomic guard"
```

---

### Task 13: BaseStudy + AggregatedCollection

**Files:**
- Create: `/rushhft-core/src/plugin/base_study.rs`
- Modify: `/rushhft-core/src/plugin/mod.rs`

- [ ] **Step 1: Write the failing tests**

Create `/rushhft-core/src/plugin/base_study.rs`:

```rust
use crate::model::enums::AggregationLevel;
use crate::model::study::BaseStudyModel;
use std::collections::VecDeque;
use std::sync::Arc;
use tokio::sync::Mutex;

pub struct AggregatedCollection {
    level: AggregationLevel,
    items: VecDeque<(i64, BaseStudyModel)>, // (bucket_epoch_secs, item)
}

impl AggregatedCollection {
    pub fn new(level: AggregationLevel) -> Self {
        Self {
            level,
            items: VecDeque::new(),
        }
    }

    pub fn push(&mut self, item: BaseStudyModel) {
        let bucket = self.bucket_start(item.timestamp);
        match self.items.back() {
            Some((last_bucket, _)) if *last_bucket == bucket => {
                // Same bucket — replace
                if let Some(back) = self.items.back_mut() {
                    back.1 = item;
                }
            }
            _ => {
                self.items.push_back((bucket, item));
                // Trim to last 1000 buckets
                while self.items.len() > 1000 {
                    self.items.pop_front();
                }
            }
        }
    }

    pub fn last(&self) -> Option<&BaseStudyModel> {
        self.items.back().map(|(_, i)| i)
    }

    fn bucket_start(&self, ts: time::OffsetDateTime) -> i64 {
        let unix = ts.unix_timestamp();
        match self.level {
            AggregationLevel::None | AggregationLevel::Ms1 | AggregationLevel::Ms10
            | AggregationLevel::Ms100 | AggregationLevel::Ms500 => unix,
            AggregationLevel::S1 => unix,
            AggregationLevel::S3 => unix / 3 * 3,
            AggregationLevel::S5 => unix / 5 * 5,
            AggregationLevel::D1 => unix / 86400 * 86400,
        }
    }
}

pub struct BaseStudy {
    tx: tokio::sync::mpsc::UnboundedSender<BaseStudyModel>,
    rx: Mutex<Option<tokio::sync::mpsc::UnboundedReceiver<BaseStudyModel>>>,
    agg_level: AggregationLevel,
}

impl BaseStudy {
    pub fn new(agg_level: AggregationLevel) -> Self {
        let (tx, rx) = tokio::sync::mpsc::unbounded_channel();
        Self {
            tx,
            rx: Mutex::new(Some(rx)),
            agg_level,
        }
    }

    pub fn add_calculation(&self, e: BaseStudyModel) {
        let _ = self.tx.send(e);
    }

    pub async fn start_consumer<F>(&self, on_calculated: F)
    where
        F: Fn(&BaseStudyModel) + Send + Sync + 'static,
    {
        let on_calculated = Arc::new(on_calculated);
        let mut rx_guard = self.rx.lock().await;
        let mut rx = rx_guard.take().expect("consumer already started");
        let mut agg = AggregatedCollection::new(self.agg_level);
        while let Some(item) = rx.recv().await {
            agg.push(item.clone());
            if let Some(last) = agg.last() {
                on_calculated(last);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rust_decimal_macros::dec;
    use std::sync::atomic::{AtomicU32, Ordering};

    #[test]
    fn aggregated_collection_replaces_within_same_second() {
        let mut agg = AggregatedCollection::new(AggregationLevel::S1);
        let ts = time::OffsetDateTime::from_unix_timestamp(1000).unwrap();

        agg.push(BaseStudyModel {
            value: dec!(0.1),
            format: "0.00".into(),
            timestamp: ts,
            market_mid_price: dec!(100),
            value_color: "".into(),
            tooltip: "".into(),
            has_error: false,
            is_stale: false,
        });
        agg.push(BaseStudyModel {
            value: dec!(0.2),
            format: "0.00".into(),
            timestamp: ts,
            market_mid_price: dec!(100),
            value_color: "".into(),
            tooltip: "".into(),
            has_error: false,
            is_stale: false,
        });

        assert_eq!(agg.items.len(), 1);
        assert_eq!(agg.last().unwrap().value, dec!(0.2));
    }

    #[test]
    fn aggregated_collection_pushes_new_bucket() {
        let mut agg = AggregatedCollection::new(AggregationLevel::S1);
        let ts1 = time::OffsetDateTime::from_unix_timestamp(1000).unwrap();
        let ts2 = time::OffsetDateTime::from_unix_timestamp(1001).unwrap();

        agg.push(BaseStudyModel {
            value: dec!(0.1),
            format: "0.00".into(),
            timestamp: ts1,
            market_mid_price: dec!(100),
            value_color: "".into(),
            tooltip: "".into(),
            has_error: false,
            is_stale: false,
        });
        agg.push(BaseStudyModel {
            value: dec!(0.2),
            format: "0.00".into(),
            timestamp: ts2,
            market_mid_price: dec!(100),
            value_color: "".into(),
            tooltip: "".into(),
            has_error: false,
            is_stale: false,
        });

        assert_eq!(agg.items.len(), 2);
    }

    #[tokio::test]
    async fn base_study_consumer_receives_items() {
        let study = BaseStudy::new(AggregationLevel::S1);
        let received = Arc::new(AtomicU32::new(0));
        let r = received.clone();

        let study = Arc::new(study);
        let s = study.clone();
        tokio::spawn(async move {
            s.start_consumer(move |_item| {
                r.fetch_add(1, Ordering::Relaxed);
            })
            .await;
        });

        // Give the consumer time to start
        tokio::time::sleep(Duration::from_millis(10)).await;

        study.add_calculation(BaseStudyModel {
            value: dec!(0.5),
            format: "0.00".into(),
            timestamp: time::OffsetDateTime::now_utc(),
            market_mid_price: dec!(100),
            value_color: "".into(),
            tooltip: "".into(),
            has_error: false,
            is_stale: false,
        });

        tokio::time::sleep(Duration::from_millis(50)).await;
        assert!(received.load(Ordering::Relaxed) >= 1);
    }
}
```

Add the `Duration` import to the test module:
```rust
use std::time::Duration;
```

Update `/rushhft-core/src/plugin/mod.rs`:

Add:
```rust
pub mod base_study;

pub use base_study::{AggregatedCollection, BaseStudy};
```

- [ ] **Step 2: Run tests to verify they pass**

Run: `cargo test --lib -p rushhft-core plugin::base_study`
Expected: PASS (3 tests)

- [ ] **Step 3: Commit**

```bash
git add rushhft-core/src/plugin/base_study.rs rushhft-core/src/plugin/mod.rs
git commit -m "feat(core): add BaseStudy + AggregatedCollection"
```

---

### Task 14: TriggerEngine — types + channel setup

**Files:**
- Create: `/rushhft-core/src/trigger/mod.rs`
- Modify: `/rushhft-core/src/lib.rs`

- [ ] **Step 1: Write the failing tests**

Create `/rushhft-core/src/trigger/mod.rs`:

```rust
use crate::model::enums::AggregationLevel;
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use time::OffsetDateTime;
use tokio::sync::mpsc;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MetricEvent {
    pub plugin: String,
    pub metric: String,
    pub exchange: String,
    pub symbol: String,
    pub value: Decimal,
    pub timestamp: OffsetDateTime,
    pub is_replay: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ConditionOperator {
    Equals,
    GreaterThan,
    LessThan,
    CrossesAbove,
    CrossesBelow,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum TimeWindowUnit {
    Seconds,
    Minutes,
    Hours,
    Days,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TimeWindow {
    pub value: i32,
    pub unit: TimeWindowUnit,
}

impl TimeWindow {
    pub fn as_duration(&self) -> std::time::Duration {
        let secs = match self.unit {
            TimeWindowUnit::Seconds => self.value as u64,
            TimeWindowUnit::Minutes => self.value as u64 * 60,
            TimeWindowUnit::Hours => self.value as u64 * 3600,
            TimeWindowUnit::Days => self.value as u64 * 86400,
        };
        std::time::Duration::from_secs(secs)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ActionType {
    RestApi,
    UIAlert,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RestApiConfig {
    pub url: String,
    pub method: String,
    pub headers: std::collections::HashMap<String, String>,
    pub body: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TriggerCondition {
    pub condition_id: i64,
    pub plugin: String,
    pub metric: String,
    pub exchange: String,
    pub symbol: String,
    pub operator: ConditionOperator,
    pub threshold: Decimal,
    pub window: Option<TimeWindow>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TriggerAction {
    pub action_type: ActionType,
    pub cooldown_duration: i32,
    pub cooldown_unit: TimeWindowUnit,
    pub rest_api: Option<RestApiConfig>,
}

impl TriggerAction {
    pub fn cooldown(&self) -> std::time::Duration {
        let secs = match self.cooldown_unit {
            TimeWindowUnit::Seconds => self.cooldown_duration as u64,
            TimeWindowUnit::Minutes => self.cooldown_duration as u64 * 60,
            TimeWindowUnit::Hours => self.cooldown_duration as u64 * 3600,
            TimeWindowUnit::Days => self.cooldown_duration as u64 * 86400,
        };
        std::time::Duration::from_secs(secs)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TriggerRule {
    pub rule_id: i64,
    pub name: String,
    pub is_enabled: bool,
    pub conditions: Vec<TriggerCondition>,
    pub actions: Vec<TriggerAction>,
}

#[derive(Debug, Clone)]
pub struct TriggerFiredEventArgs {
    pub rule: TriggerRule,
    pub metric_event: MetricEvent,
    pub action_index: usize,
}

fn metric_key(plugin: &str, metric: &str, exchange: &str, symbol: &str) -> String {
    format!("{}|{}|{}|{}", plugin, metric, exchange, symbol)
}

fn condition_key(rule_id: i64, condition_id: i64) -> String {
    format!("r{}|c{}", rule_id, condition_id)
}

fn action_key(rule_id: i64, action_index: usize) -> String {
    format!("r{}|a{}", rule_id, action_index)
}

pub struct TriggerEngine {
    rules: tokio::sync::RwLock<Vec<TriggerRule>>,
    last_metric_values: dashmap::DashMap<String, (Decimal, OffsetDateTime)>,
    condition_start_times: dashmap::DashMap<String, OffsetDateTime>,
    action_last_fired_times: dashmap::DashMap<String, OffsetDateTime>,
    metric_tx: mpsc::UnboundedSender<MetricEvent>,
    metric_rx: tokio::sync::Mutex<Option<mpsc::UnboundedReceiver<MetricEvent>>>,
    on_trigger_fired: arc_swap::ArcSwap<Vec<Arc<dyn Fn(TriggerFiredEventArgs) + Send + Sync>>>,
}

impl TriggerEngine {
    pub fn new() -> Self {
        let (tx, rx) = mpsc::unbounded_channel();
        Self {
            rules: tokio::sync::RwLock::new(Vec::new()),
            last_metric_values: dashmap::DashMap::new(),
            condition_start_times: dashmap::DashMap::new(),
            action_last_fired_times: dashmap::DashMap::new(),
            metric_tx: tx,
            metric_rx: tokio::sync::Mutex::new(Some(rx)),
            on_trigger_fired: arc_swap::ArcSwap::from_pointee(Vec::new()),
        }
    }

    pub fn register_metric(&self, event: MetricEvent) {
        let _ = self.metric_tx.send(event);
    }

    pub async fn add_or_update_rule(&self, rule: TriggerRule) {
        let mut rules = self.rules.write().await;
        let key = rule.rule_id;
        if let Some(existing) = rules.iter_mut().find(|r| r.rule_id == key) {
            *existing = rule;
        } else {
            rules.push(rule);
        }
        drop(rules);
        // After rule edit, replay latest metrics (re-evaluate)
        self.replay_latest_metrics().await;
    }

    pub async fn remove_rule(&self, rule_id: i64) {
        let mut rules = self.rules.write().await;
        rules.retain(|r| r.rule_id != rule_id);
    }

    pub async fn get_rules(&self) -> Vec<TriggerRule> {
        self.rules.read().await.clone()
    }

    pub fn on_trigger_fired(&self, f: Arc<dyn Fn(TriggerFiredEventArgs) + Send + Sync>) {
        loop {
            let current = self.on_trigger_fired.load();
            let mut new_list = (**current).clone();
            new_list.push(f.clone());
            match self.on_trigger_fired.compare_exchange(current, new_list) {
                Ok(_) => break,
                Err(_) => continue,
            }
        }
    }

    pub async fn start(self: Arc<Self>) {
        let mut rx_guard = self.metric_rx.lock().await;
        let mut rx = rx_guard.take().expect("engine already started");
        drop(rx_guard);
        while let Some(event) = rx.recv().await {
            self.process_metric(event).await;
        }
    }

    async fn process_metric(&self, event: MetricEvent) {
        let key = metric_key(&event.plugin, &event.metric, &event.exchange, &event.symbol);

        // Get previous value (for crosses operators)
        let prev_value = self
            .last_metric_values
            .get(&key)
            .map(|e| e.0)
            .unwrap_or(event.value); // if no prev, use current (no cross will fire)

        // Update latest metric value
        self.last_metric_values
            .insert(key.clone(), (event.value, event.timestamp));

        // Skip action firing for replays
        if event.is_replay {
            // Only update state (condition_start_times, last_metric_values)
            self.update_condition_state(&event, prev_value).await;
            return;
        }

        // Evaluate rules
        let rules = self.rules.read().await;
        for rule in rules.iter() {
            if !rule.is_enabled {
                continue;
            }
            // Check if any condition matches this metric
            let matches = rule.conditions.iter().any(|c| {
                c.plugin == event.plugin
                    && c.metric == event.metric
                    && c.exchange == event.exchange
                    && c.symbol == event.symbol
            });
            if !matches {
                continue;
            }

            // Evaluate all conditions
            let all_satisfied = self
                .evaluate_all_conditions(rule, &event, prev_value)
                .await;
            if all_satisfied {
                // Fire actions (with cooldown)
                for (idx, action) in rule.actions.iter().enumerate() {
                    let akey = action_key(rule.rule_id, idx);
                    if self.is_in_cooldown(&akey, event.timestamp, action.cooldown()) {
                        continue;
                    }
                    // Fire
                    self.action_last_fired_times
                        .insert(akey, event.timestamp);
                    let args = TriggerFiredEventArgs {
                        rule: rule.clone(),
                        metric_event: event.clone(),
                        action_index: idx,
                    };
                    self.fire_callbacks(args);
                }
            }
        }
    }

    async fn update_condition_state(&self, event: &MetricEvent, prev_value: Decimal) {
        let rules = self.rules.read().await;
        for rule in rules.iter() {
            if !rule.is_enabled {
                continue;
            }
            let _ = self.evaluate_all_conditions(rule, event, prev_value).await;
        }
    }

    async fn evaluate_all_conditions(
        &self,
        rule: &TriggerRule,
        event: &MetricEvent,
        prev_value: Decimal,
    ) -> bool {
        for cond in &rule.conditions {
            let key = metric_key(&cond.plugin, &cond.metric, &cond.exchange, &cond.symbol);
            let (current_val, current_ts) = match self.last_metric_values.get(&key) {
                Some(e) => (e.0, e.1),
                None => return false,
            };

            let ckey = condition_key(rule.rule_id, cond.condition_id);

            let satisfied = self.evaluate_condition(cond, current_val, prev_value);

            if satisfied {
                // Check sustained window
                if let Some(window) = &cond.window {
                    let dur = window.as_duration();
                    let start = match self.condition_start_times.get(&ckey) {
                        Some(s) => s.value(),
                        None => {
                            // First time condition is satisfied — record start
                            drop(self.condition_start_times.insert(ckey.clone(), current_ts));
                            return false; // not sustained yet
                        }
                    };
                    let elapsed = current_ts - start;
                    if elapsed < time::Duration::seconds(dur.as_secs() as i64) {
                        return false; // not sustained long enough
                    }
                }
            } else {
                // Condition not satisfied — clear start time
                self.condition_start_times.remove(&ckey);
                return false;
            }
        }
        true
    }

    fn evaluate_condition(
        &self,
        cond: &TriggerCondition,
        current: Decimal,
        prev: Decimal,
    ) -> bool {
        match cond.operator {
            ConditionOperator::Equals => current == cond.threshold,
            ConditionOperator::GreaterThan => current > cond.threshold,
            ConditionOperator::LessThan => current < cond.threshold,
            ConditionOperator::CrossesAbove => prev <= cond.threshold && current > cond.threshold,
            ConditionOperator::CrossesBelow => prev >= cond.threshold && current < cond.threshold,
        }
    }

    fn is_in_cooldown(
        &self,
        key: &str,
        now: OffsetDateTime,
        cooldown: std::time::Duration,
    ) -> bool {
        if let Some(last) = self.action_last_fired_times.get(key) {
            let elapsed = now - last.value();
            let elapsed_dur = std::time::Duration::from_secs(elapsed.whole_seconds().max(0) as u64);
            return elapsed_dur < cooldown;
        }
        false
    }

    fn fire_callbacks(&self, args: TriggerFiredEventArgs) {
        let subs = self.on_trigger_fired.load();
        for sub in subs.iter() {
            let sub = sub.clone();
            let args = args.clone();
            let _ = std::panic::catch_unwind(std::panic::AssertUnwindSafe(move || sub(args)));
        }
    }

    async fn replay_latest_metrics(&self) {
        let snapshots: Vec<(String, (Decimal, OffsetDateTime))> = self
            .last_metric_values
            .iter()
            .map(|e| (e.key().clone(), e.value().clone()))
            .collect();
        for (key, (value, ts)) in snapshots {
            let parts: Vec<&str> = key.split('|').collect();
            if parts.len() < 4 {
                continue;
            }
            let event = MetricEvent {
                plugin: parts[0].to_string(),
                metric: parts[1].to_string(),
                exchange: parts[2].to_string(),
                symbol: parts[3].to_string(),
                value,
                timestamp: ts,
                is_replay: true,
            };
            let _ = self.metric_tx.send(event);
        }
    }
}

impl Default for TriggerEngine {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rust_decimal_macros::dec;

    fn make_event(value: Decimal, ts: i64) -> MetricEvent {
        MetricEvent {
            plugin: "VPIN".into(),
            metric: "vpin".into(),
            exchange: "LongPort".into(),
            symbol: "700.HK".into(),
            value,
            timestamp: OffsetDateTime::from_unix_timestamp(ts).unwrap(),
            is_replay: false,
        }
    }

    #[test]
    fn register_metric_sends_to_channel() {
        let engine = TriggerEngine::new();
        let event = make_event(dec!(0.5), 1000);
        engine.register_metric(event);
        // The event should be buffered in the channel
        // We can't recv without starting the consumer, so just verify no error
    }

    #[test]
    fn time_window_duration() {
        let w = TimeWindow { value: 5, unit: TimeWindowUnit::Seconds };
        assert_eq!(w.as_duration(), std::time::Duration::from_secs(5));
        let w = TimeWindow { value: 3, unit: TimeWindowUnit::Minutes };
        assert_eq!(w.as_duration(), std::time::Duration::from_secs(180));
    }

    #[test]
    fn trigger_action_cooldown() {
        let a = TriggerAction {
            action_type: ActionType::UIAlert,
            cooldown_duration: 30,
            cooldown_unit: TimeWindowUnit::Seconds,
            rest_api: None,
        };
        assert_eq!(a.cooldown(), std::time::Duration::from_secs(30));
    }
}
```

Update `/rushhft-core/src/lib.rs`:

```rust
pub mod hub;
pub mod model;
pub mod plugin;
pub mod pool;
pub mod trigger;
```

- [ ] **Step 2: Run tests to verify they pass**

Run: `cargo test --lib -p rushhft-core trigger`
Expected: PASS (3 tests)

- [ ] **Step 3: Commit**

```bash
git add rushhft-core/src/trigger/mod.rs rushhft-core/src/lib.rs
git commit -m "feat(core): add TriggerEngine types + channel + process_metric skeleton"
```

---

### Task 15: TriggerEngine — direct condition evaluation

**Files:**
- Modify: `/rushhft-core/src/trigger/mod.rs`

- [ ] **Step 1: Write the failing tests**

Add to the test module in `trigger/mod.rs`:

```rust
    use std::sync::atomic::{AtomicU32, Ordering};
    use std::time::Duration;

    fn make_rule(operator: ConditionOperator, threshold: Decimal) -> TriggerRule {
        TriggerRule {
            rule_id: 1,
            name: "test".into(),
            is_enabled: true,
            conditions: vec![TriggerCondition {
                condition_id: 1,
                plugin: "VPIN".into(),
                metric: "vpin".into(),
                exchange: "LongPort".into(),
                symbol: "700.HK".into(),
                operator,
                threshold,
                window: None,
            }],
            actions: vec![TriggerAction {
                action_type: ActionType::UIAlert,
                cooldown_duration: 0,
                cooldown_unit: TimeWindowUnit::Seconds,
                rest_api: None,
            }],
        }
    }

    #[tokio::test]
    async fn equals_fires_when_equal() {
        let engine = Arc::new(TriggerEngine::new());
        engine.add_or_update_rule(make_rule(ConditionOperator::Equals, dec!(0.5))).await;
        engine.on_trigger_fired(Arc::new(|_| {})); // no-op subscriber so callbacks exist

        let fired = Arc::new(AtomicU32::new(0));
        let f = fired.clone();
        // Clear existing callbacks and add ours
        // (on_trigger_fired is additive — we need a fresh engine for isolation)
        let engine = Arc::new(TriggerEngine::new());
        engine.add_or_update_rule(make_rule(ConditionOperator::Equals, dec!(0.5))).await;
        engine.on_trigger_fired(Arc::new(move |_| {
            f.fetch_add(1, Ordering::Relaxed);
        }));

        let e = engine.clone();
        tokio::spawn(async move { e.start().await });
        tokio::time::sleep(Duration::from_millis(10)).await;

        engine.register_metric(make_event(dec!(0.5), 1000));
        tokio::time::sleep(Duration::from_millis(50)).await;

        assert_eq!(fired.load(Ordering::Relaxed), 1);
    }

    #[tokio::test]
    async fn greater_than_fires() {
        let fired = Arc::new(AtomicU32::new(0));
        let f = fired.clone();
        let engine = Arc::new(TriggerEngine::new());
        engine.add_or_update_rule(make_rule(ConditionOperator::GreaterThan, dec!(0.7))).await;
        engine.on_trigger_fired(Arc::new(move |_| {
            f.fetch_add(1, Ordering::Relaxed);
        }));

        let e = engine.clone();
        tokio::spawn(async move { e.start().await });
        tokio::time::sleep(Duration::from_millis(10)).await;

        engine.register_metric(make_event(dec!(0.8), 1000));
        tokio::time::sleep(Duration::from_millis(50)).await;

        assert_eq!(fired.load(Ordering::Relaxed), 1);
    }

    #[tokio::test]
    async fn greater_than_does_not_fire_below_threshold() {
        let fired = Arc::new(AtomicU32::new(0));
        let f = fired.clone();
        let engine = Arc::new(TriggerEngine::new());
        engine.add_or_update_rule(make_rule(ConditionOperator::GreaterThan, dec!(0.7))).await;
        engine.on_trigger_fired(Arc::new(move |_| {
            f.fetch_add(1, Ordering::Relaxed);
        }));

        let e = engine.clone();
        tokio::spawn(async move { e.start().await });
        tokio::time::sleep(Duration::from_millis(10)).await;

        engine.register_metric(make_event(dec!(0.6), 1000));
        tokio::time::sleep(Duration::from_millis(50)).await;

        assert_eq!(fired.load(Ordering::Relaxed), 0);
    }

    #[tokio::test]
    async fn less_than_fires() {
        let fired = Arc::new(AtomicU32::new(0));
        let f = fired.clone();
        let engine = Arc::new(TriggerEngine::new());
        engine.add_or_update_rule(make_rule(ConditionOperator::LessThan, dec!(0.3))).await;
        engine.on_trigger_fired(Arc::new(move |_| {
            f.fetch_add(1, Ordering::Relaxed);
        }));

        let e = engine.clone();
        tokio::spawn(async move { e.start().await });
        tokio::time::sleep(Duration::from_millis(10)).await;

        engine.register_metric(make_event(dec!(0.2), 1000));
        tokio::time::sleep(Duration::from_millis(50)).await;

        assert_eq!(fired.load(Ordering::Relaxed), 1);
    }
```

- [ ] **Step 2: Run tests to verify they pass**

Run: `cargo test --lib -p rushhft-core trigger`
Expected: PASS (7 tests — 3 existing + 4 new)

Note: these tests pass immediately because the condition evaluation logic was already implemented in Task 14's `process_metric`. This task verifies the end-to-end flow works correctly.

- [ ] **Step 3: Commit**

```bash
git add rushhft-core/src/trigger/mod.rs
git commit -m "test(core): verify TriggerEngine direct condition evaluation (Equals/GT/LT)"
```

---

### Task 16: TriggerEngine — CrossesAbove / CrossesBelow

**Files:**
- Modify: `/rushhft-core/src/trigger/mod.rs`

- [ ] **Step 1: Write the failing tests**

Add to the test module:

```rust
    #[tokio::test]
    async fn crosses_above_fires_on_crossing_up() {
        let fired = Arc::new(AtomicU32::new(0));
        let f = fired.clone();
        let engine = Arc::new(TriggerEngine::new());
        engine.add_or_update_rule(make_rule(ConditionOperator::CrossesAbove, dec!(0.5))).await;
        engine.on_trigger_fired(Arc::new(move |_| {
            f.fetch_add(1, Ordering::Relaxed);
        }));

        let e = engine.clone();
        tokio::spawn(async move { e.start().await });
        tokio::time::sleep(Duration::from_millis(10)).await;

        // First value below threshold — no fire
        engine.register_metric(make_event(dec!(0.4), 1000));
        tokio::time::sleep(Duration::from_millis(50)).await;
        assert_eq!(fired.load(Ordering::Relaxed), 0);

        // Second value above threshold — crosses above fires
        engine.register_metric(make_event(dec!(0.6), 1001));
        tokio::time::sleep(Duration::from_millis(50)).await;
        assert_eq!(fired.load(Ordering::Relaxed), 1);
    }

    #[tokio::test]
    async fn crosses_below_fires_on_crossing_down() {
        let fired = Arc::new(AtomicU32::new(0));
        let f = fired.clone();
        let engine = Arc::new(TriggerEngine::new());
        engine.add_or_update_rule(make_rule(ConditionOperator::CrossesBelow, dec!(0.5))).await;
        engine.on_trigger_fired(Arc::new(move |_| {
            f.fetch_add(1, Ordering::Relaxed);
        }));

        let e = engine.clone();
        tokio::spawn(async move { e.start().await });
        tokio::time::sleep(Duration::from_millis(10)).await;

        // First value above threshold
        engine.register_metric(make_event(dec!(0.6), 1000));
        tokio::time::sleep(Duration::from_millis(50)).await;
        assert_eq!(fired.load(Ordering::Relaxed), 0);

        // Second value below threshold — crosses below fires
        engine.register_metric(make_event(dec!(0.4), 1001));
        tokio::time::sleep(Duration::from_millis(50)).await;
        assert_eq!(fired.load(Ordering::Relaxed), 1);
    }
```

- [ ] **Step 2: Run tests to verify they pass**

Run: `cargo test --lib -p rushhft-core trigger`
Expected: PASS (9 tests)

- [ ] **Step 3: Commit**

```bash
git add rushhft-core/src/trigger/mod.rs
git commit -m "test(core): verify TriggerEngine CrossesAbove/CrossesBelow operators"
```

---

### Task 17: TriggerEngine — sustained window

**Files:**
- Modify: `/rushhft-core/src/trigger/mod.rs`

- [ ] **Step 1: Write the failing tests**

Add to the test module:

```rust
    fn make_windowed_rule(operator: ConditionOperator, threshold: Decimal, window_secs: i32) -> TriggerRule {
        TriggerRule {
            rule_id: 2,
            name: "windowed".into(),
            is_enabled: true,
            conditions: vec![TriggerCondition {
                condition_id: 2,
                plugin: "VPIN".into(),
                metric: "vpin".into(),
                exchange: "LongPort".into(),
                symbol: "700.HK".into(),
                operator,
                threshold,
                window: Some(TimeWindow { value: window_secs, unit: TimeWindowUnit::Seconds }),
            }],
            actions: vec![TriggerAction {
                action_type: ActionType::UIAlert,
                cooldown_duration: 0,
                cooldown_unit: TimeWindowUnit::Seconds,
                rest_api: None,
            }],
        }
    }

    #[tokio::test]
    async fn sustained_window_fires_after_duration() {
        let fired = Arc::new(AtomicU32::new(0));
        let f = fired.clone();
        let engine = Arc::new(TriggerEngine::new());
        engine.add_or_update_rule(make_windowed_rule(ConditionOperator::GreaterThan, dec!(0.5), 5)).await;
        engine.on_trigger_fired(Arc::new(move |_| {
            f.fetch_add(1, Ordering::Relaxed);
        }));

        let e = engine.clone();
        tokio::spawn(async move { e.start().await });
        tokio::time::sleep(Duration::from_millis(10)).await;

        // First observation at t=0 — condition satisfied, start time recorded
        engine.register_metric(make_event(dec!(0.6), 1000));
        tokio::time::sleep(Duration::from_millis(50)).await;
        assert_eq!(fired.load(Ordering::Relaxed), 0); // not sustained yet

        // Second observation at t=6 — elapsed >= 5s → fire
        engine.register_metric(make_event(dec!(0.6), 1006));
        tokio::time::sleep(Duration::from_millis(50)).await;
        assert_eq!(fired.load(Ordering::Relaxed), 1);
    }

    #[tokio::test]
    async fn sustained_window_does_not_fire_before_duration() {
        let fired = Arc::new(AtomicU32::new(0));
        let f = fired.clone();
        let engine = Arc::new(TriggerEngine::new());
        engine.add_or_update_rule(make_windowed_rule(ConditionOperator::GreaterThan, dec!(0.5), 10)).await;
        engine.on_trigger_fired(Arc::new(move |_| {
            f.fetch_add(1, Ordering::Relaxed);
        }));

        let e = engine.clone();
        tokio::spawn(async move { e.start().await });
        tokio::time::sleep(Duration::from_millis(10)).await;

        engine.register_metric(make_event(dec!(0.6), 1000));
        tokio::time::sleep(Duration::from_millis(50)).await;

        engine.register_metric(make_event(dec!(0.6), 1005));
        tokio::time::sleep(Duration::from_millis(50)).await;

        assert_eq!(fired.load(Ordering::Relaxed), 0); // only 5s elapsed, need 10s
    }

    #[tokio::test]
    async fn condition_becoming_false_resets_window() {
        let fired = Arc::new(AtomicU32::new(0));
        let f = fired.clone();
        let engine = Arc::new(TriggerEngine::new());
        engine.add_or_update_rule(make_windowed_rule(ConditionOperator::GreaterThan, dec!(0.5), 5)).await;
        engine.on_trigger_fired(Arc::new(move |_| {
            f.fetch_add(1, Ordering::Relaxed);
        }));

        let e = engine.clone();
        tokio::spawn(async move { e.start().await });
        tokio::time::sleep(Duration::from_millis(10)).await;

        // Satisfied at t=0
        engine.register_metric(make_event(dec!(0.6), 1000));
        tokio::time::sleep(Duration::from_millis(50)).await;

        // Not satisfied at t=3
        engine.register_metric(make_event(dec!(0.3), 1003));
        tokio::time::sleep(Duration::from_millis(50)).await;

        // Satisfied again at t=4 — start time should reset
        engine.register_metric(make_event(dec!(0.6), 1004));
        tokio::time::sleep(Duration::from_millis(50)).await;

        // At t=10 — only 6s since reset, need 5s → should fire
        engine.register_metric(make_event(dec!(0.6), 1010));
        tokio::time::sleep(Duration::from_millis(50)).await;
        assert_eq!(fired.load(Ordering::Relaxed), 1);
    }
```

- [ ] **Step 2: Run tests to verify they pass**

Run: `cargo test --lib -p rushhft-core trigger`
Expected: PASS (12 tests)

- [ ] **Step 3: Commit**

```bash
git add rushhft-core/src/trigger/mod.rs
git commit -m "test(core): verify TriggerEngine sustained window logic"
```

---

### Task 18: TriggerEngine — cooldown

**Files:**
- Modify: `/rushhft-core/src/trigger/mod.rs`

- [ ] **Step 1: Write the failing tests**

Add to the test module:

```rust
    fn make_cooldown_rule(threshold: Decimal, cooldown_secs: i32) -> TriggerRule {
        TriggerRule {
            rule_id: 3,
            name: "cooldown".into(),
            is_enabled: true,
            conditions: vec![TriggerCondition {
                condition_id: 3,
                plugin: "VPIN".into(),
                metric: "vpin".into(),
                exchange: "LongPort".into(),
                symbol: "700.HK".into(),
                operator: ConditionOperator::GreaterThan,
                threshold,
                window: None,
            }],
            actions: vec![TriggerAction {
                action_type: ActionType::UIAlert,
                cooldown_duration: cooldown_secs,
                cooldown_unit: TimeWindowUnit::Seconds,
                rest_api: None,
            }],
        }
    }

    #[tokio::test]
    async fn cooldown_prevents_refire_within_period() {
        let fired = Arc::new(AtomicU32::new(0));
        let f = fired.clone();
        let engine = Arc::new(TriggerEngine::new());
        engine.add_or_update_rule(make_cooldown_rule(dec!(0.5), 10)).await;
        engine.on_trigger_fired(Arc::new(move |_| {
            f.fetch_add(1, Ordering::Relaxed);
        }));

        let e = engine.clone();
        tokio::spawn(async move { e.start().await });
        tokio::time::sleep(Duration::from_millis(10)).await;

        // First fire
        engine.register_metric(make_event(dec!(0.6), 1000));
        tokio::time::sleep(Duration::from_millis(50)).await;
        assert_eq!(fired.load(Ordering::Relaxed), 1);

        // Second metric within cooldown — should NOT fire
        engine.register_metric(make_event(dec!(0.6), 1005));
        tokio::time::sleep(Duration::from_millis(50)).await;
        assert_eq!(fired.load(Ordering::Relaxed), 1);

        // Third metric after cooldown — should fire
        engine.register_metric(make_event(dec!(0.6), 1011));
        tokio::time::sleep(Duration::from_millis(50)).await;
        assert_eq!(fired.load(Ordering::Relaxed), 2);
    }
```

- [ ] **Step 2: Run tests to verify they pass**

Run: `cargo test --lib -p rushhft-core trigger`
Expected: PASS (13 tests)

- [ ] **Step 3: Commit**

```bash
git add rushhft-core/src/trigger/mod.rs
git commit -m "test(core): verify TriggerEngine cooldown behavior"
```

---

### Task 19: TriggerEngine — replay suppression

**Files:**
- Modify: `/rushhft-core/src/trigger/mod.rs`

- [ ] **Step 1: Write the failing tests**

Add to the test module:

```rust
    #[tokio::test]
    async fn replay_does_not_fire_actions() {
        let fired = Arc::new(AtomicU32::new(0));
        let f = fired.clone();
        let engine = Arc::new(TriggerEngine::new());
        engine.add_or_update_rule(make_rule(ConditionOperator::GreaterThan, dec!(0.5))).await;
        engine.on_trigger_fired(Arc::new(move |_| {
            f.fetch_add(1, Ordering::Relaxed);
        }));

        let e = engine.clone();
        tokio::spawn(async move { e.start().await });
        tokio::time::sleep(Duration::from_millis(10)).await;

        // Normal metric — fires
        engine.register_metric(make_event(dec!(0.6), 1000));
        tokio::time::sleep(Duration::from_millis(50)).await;
        assert_eq!(fired.load(Ordering::Relaxed), 1);

        // Replay the same metric — should NOT fire
        let mut replay_event = make_event(dec!(0.6), 1000);
        replay_event.is_replay = true;
        engine.register_metric(replay_event);
        tokio::time::sleep(Duration::from_millis(50)).await;
        assert_eq!(fired.load(Ordering::Relaxed), 1);
    }

    #[tokio::test]
    async fn replay_updates_state_only() {
        let engine = Arc::new(TriggerEngine::new());
        engine.add_or_update_rule(make_rule(ConditionOperator::GreaterThan, dec!(0.5))).await;

        let e = engine.clone();
        tokio::spawn(async move { e.start().await });
        tokio::time::sleep(Duration::from_millis(10)).await;

        // Send a replay metric
        let mut replay = make_event(dec!(0.6), 1000);
        replay.is_replay = true;
        engine.register_metric(replay);
        tokio::time::sleep(Duration::from_millis(50)).await;

        // State should be updated (last_metric_values)
        let key = "VPIN|vpin|LongPort|700.HK";
        let val = engine.last_metric_values.get(key).unwrap();
        assert_eq!(val.0, dec!(0.6));
    }
```

- [ ] **Step 2: Run tests to verify they pass**

Run: `cargo test --lib -p rushhft-core trigger`
Expected: PASS (15 tests)

- [ ] **Step 3: Commit**

```bash
git add rushhft-core/src/trigger/mod.rs
git commit -m "test(core): verify TriggerEngine replay suppression"
```

---

### Task 20: TriggerEngine — additive fan-out with catch_unwind

**Files:**
- Modify: `/rushhft-core/src/trigger/mod.rs`

- [ ] **Step 1: Write the failing tests**

Add to the test module:

```rust
    #[tokio::test]
    async fn panicking_callback_does_not_break_others() {
        let fired = Arc::new(AtomicU32::new(0));
        let f = fired.clone();

        let engine = Arc::new(TriggerEngine::new());
        engine.add_or_update_rule(make_rule(ConditionOperator::GreaterThan, dec!(0.5))).await;

        // First callback panics
        engine.on_trigger_fired(Arc::new(|_| {
            panic!("boom in trigger callback");
        }));
        // Second callback should still fire
        engine.on_trigger_fired(Arc::new(move |_| {
            f.fetch_add(1, Ordering::Relaxed);
        }));

        let e = engine.clone();
        tokio::spawn(async move { e.start().await });
        tokio::time::sleep(Duration::from_millis(10)).await;

        engine.register_metric(make_event(dec!(0.6), 1000));
        tokio::time::sleep(Duration::from_millis(50)).await;

        assert_eq!(fired.load(Ordering::Relaxed), 1);
    }
```

- [ ] **Step 2: Run tests to verify they pass**

Run: `cargo test --lib -p rushhft-core trigger`
Expected: PASS (16 tests)

- [ ] **Step 3: Commit**

```bash
git add rushhft-core/src/trigger/mod.rs
git commit -m "test(core): verify TriggerEngine additive fan-out with catch_unwind"
```

---

### Task 21: Settings

**Files:**
- Create: `/rushhft-core/src/settings/mod.rs`
- Modify: `/rushhft-core/src/lib.rs`

- [ ] **Step 1: Write the failing tests**

Create `/rushhft-core/src/settings/mod.rs`:

```rust
use crate::model::enums::AggregationLevel;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Settings {
    pub app_key: String,
    pub app_secret: String,
    pub access_token: String,
    pub default_symbols: Vec<String>,
    pub depth_levels: usize,
    pub aggregation_level: AggregationLevel,
    pub log_level: String,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            app_key: String::new(),
            app_secret: String::new(),
            access_token: String::new(),
            default_symbols: vec!["700.HK".to_string()],
            depth_levels: 10,
            aggregation_level: AggregationLevel::S1,
            log_level: "info".to_string(),
        }
    }
}

impl Settings {
    pub fn config_dir() -> std::path::PathBuf {
        let mut path = dirs::config_dir().unwrap_or_else(|| std::path::PathBuf::from("."));
        path.push("RushHFT");
        path
    }

    pub fn config_path() -> std::path::PathBuf {
        Self::config_dir().join("config.toml")
    }

    pub fn load() -> Result<Self, SettingsError> {
        let path = Self::config_path();
        if !path.exists() {
            return Ok(Self::default());
        }
        let content = std::fs::read_to_string(&path)
            .map_err(|e| SettingsError::ReadFailed(path.clone(), e.to_string()))?;
        let settings: Settings = toml::from_str(&content)
            .map_err(|e| SettingsError::ParseFailed(e.to_string()))?;
        Ok(settings)
    }

    pub fn save(&self) -> Result<(), SettingsError> {
        let dir = Self::config_dir();
        std::fs::create_dir_all(&dir)
            .map_err(|e| SettingsError::WriteFailed(Self::config_path(), e.to_string()))?;
        let content = toml::to_string_pretty(self)
            .map_err(|e| SettingsError::SerializeFailed(e.to_string()))?;
        std::fs::write(Self::config_path(), content)
            .map_err(|e| SettingsError::WriteFailed(Self::config_path(), e.to_string()))?;
        Ok(())
    }
}

#[derive(Debug, thiserror::Error)]
pub enum SettingsError {
    #[error("failed to read config file {0}: {1}")]
    ReadFailed(std::path::PathBuf, String),
    #[error("failed to parse config: {0}")]
    ParseFailed(String),
    #[error("failed to write config file {0}: {1}")]
    WriteFailed(std::path::PathBuf, String),
    #[error("failed to serialize config: {0}")]
    SerializeFailed(String),
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_settings() {
        let s = Settings::default();
        assert!(s.app_key.is_empty());
        assert_eq!(s.depth_levels, 10);
        assert_eq!(s.aggregation_level, AggregationLevel::S1);
        assert_eq!(s.log_level, "info");
        assert!(!s.default_symbols.is_empty());
    }

    #[test]
    fn serialize_deserialize_roundtrip() {
        let s = Settings {
            app_key: "key123".into(),
            app_secret: "secret".into(),
            access_token: "token".into(),
            default_symbols: vec!["700.HK".into(), "AAPL.US".into()],
            depth_levels: 20,
            aggregation_level: AggregationLevel::S5,
            log_level: "debug".into(),
        };
        let toml_str = toml::to_string_pretty(&s).unwrap();
        let back: Settings = toml::from_str(&toml_str).unwrap();
        assert_eq!(back.app_key, "key123");
        assert_eq!(back.depth_levels, 20);
        assert_eq!(back.aggregation_level, AggregationLevel::S5);
        assert_eq!(back.default_symbols, vec!["700.HK", "AAPL.US"]);
    }

    #[test]
    fn config_dir_ends_with_rushhft() {
        let dir = Settings::config_dir();
        assert!(dir.ends_with("RushHFT"));
    }
}
```

Update `/rushhft-core/src/lib.rs`:

```rust
pub mod hub;
pub mod model;
pub mod plugin;
pub mod pool;
pub mod settings;
pub mod trigger;
```

- [ ] **Step 2: Run tests to verify they pass**

Run: `cargo test --lib -p rushhft-core settings`
Expected: PASS (3 tests)

- [ ] **Step 3: Commit**

```bash
git add rushhft-core/src/settings/mod.rs rushhft-core/src/lib.rs
git commit -m "feat(core): add Settings with TOML load/save"
```

---

### Task 22: TriggerEngine — rule persistence (TOML)

**Files:**
- Modify: `/rushhft-core/src/trigger/mod.rs`

- [ ] **Step 1: Write the failing tests**

Add to the test module in `trigger/mod.rs`:

```rust
    #[test]
    fn trigger_rule_serialize_deserialize() {
        let rule = TriggerRule {
            rule_id: 1,
            name: "VPIN alert".into(),
            is_enabled: true,
            conditions: vec![TriggerCondition {
                condition_id: 1,
                plugin: "VPIN".into(),
                metric: "vpin".into(),
                exchange: "LongPort".into(),
                symbol: "700.HK".into(),
                operator: ConditionOperator::GreaterThan,
                threshold: dec!(0.7),
                window: Some(TimeWindow { value: 5, unit: TimeWindowUnit::Seconds }),
            }],
            actions: vec![TriggerAction {
                action_type: ActionType::RestApi,
                cooldown_duration: 60,
                cooldown_unit: TimeWindowUnit::Seconds,
                rest_api: Some(RestApiConfig {
                    url: "https://example.com/hook".into(),
                    method: "POST".into(),
                    headers: std::collections::HashMap::from([("Authorization".into(), "Bearer xxx".into())]),
                    body: "{\"alert\":\"vpin\"}".into(),
                }),
            }],
        };

        let toml_str = toml::to_string_pretty(&rule).unwrap();
        let back: TriggerRule = toml::from_str(&toml_str).unwrap();

        assert_eq!(back.rule_id, 1);
        assert_eq!(back.name, "VPIN alert");
        assert!(back.is_enabled);
        assert_eq!(back.conditions.len(), 1);
        assert_eq!(back.conditions[0].operator, ConditionOperator::GreaterThan);
        assert_eq!(back.conditions[0].threshold, dec!(0.7));
        assert!(back.conditions[0].window.is_some());
        assert_eq!(back.actions.len(), 1);
        assert_eq!(back.actions[0].action_type, ActionType::RestApi);
        assert!(back.actions[0].rest_api.is_some());
    }
```

- [ ] **Step 2: Run tests to verify they pass**

Run: `cargo test --lib -p rushhft-core trigger`
Expected: PASS (all trigger tests + 1 new = 17+)

- [ ] **Step 3: Commit**

```bash
git add rushhft-core/src/trigger/mod.rs
git commit -m "test(core): verify TriggerRule TOML serialization roundtrip"
```

---

### Task 23: Re-exports + clippy + full test run

**Files:**
- Modify: `/rushhft-core/src/lib.rs`

- [ ] **Step 1: Add convenience re-exports to lib.rs**

```rust
pub mod hub;
pub mod model;
pub mod plugin;
pub mod pool;
pub mod settings;
pub mod trigger;

pub use model::book_item::BookItem;
pub use model::enums::*;
pub use model::order_book::OrderBook;
pub use model::provider::Provider;
pub use model::study::BaseStudyModel;
pub use model::trade::Trade;
pub use hub::{OrderBookHub, ProviderHub, SubscriptionGuard, TradeHub};
pub use plugin::{Plugin, PluginContext, PluginError, BaseDataRetriever, BaseStudy, AggregatedCollection};
pub use pool::{ObjectPool, PoolGuard, RollingWindow};
pub use settings::{Settings, SettingsError};
pub use trigger::{
    ActionType, ConditionOperator, MetricEvent, RestApiConfig, TimeWindow, TimeWindowUnit,
    TriggerAction, TriggerCondition, TriggerEngine, TriggerFiredEventArgs, TriggerRule,
};
```

- [ ] **Step 2: Run clippy**

Run: `cargo clippy --lib -p rushhft-core -- -D warnings`
Expected: no warnings (fix any that appear)

- [ ] **Step 3: Run all tests**

Run: `cargo test -p rushhft-core`
Expected: ALL PASS

- [ ] **Step 4: Run fmt check**

Run: `cargo fmt --all -- --check`
Expected: no diff (run `cargo fmt --all` if needed)

- [ ] **Step 5: Commit**

```bash
git add rushhft-core/src/lib.rs
git commit -m "feat(core): add re-exports, pass clippy + full test suite"
```

---

## Self-Review

### 1. Spec coverage

| Spec section | Tasks covering it |
|---|---|
| Domain models (OrderBook, BookItem, Trade, BaseStudyModel, Provider, enums) | Tasks 2–7 |
| Pub/sub hub (OrderBookHub, TradeHub, ProviderHub, lock-free subscribers, catch_unwind fan-out) | Task 10 |
| Pools (ObjectPool, RollingWindow, BookItemPool, TradePool) | Tasks 8–9 (BookItemPool/TradePool are type aliases — `ObjectPool<BookItem>` / `ObjectPool<Trade>` — no separate task needed) |
| Plugin trait + PluginContext | Task 11 |
| BaseDataRetriever (reconnection, exponential backoff, max 5, atomic guard) | Task 12 |
| BaseStudy + AggregatedCollection | Task 13 |
| TriggerEngine (channel, process_metric, direct ops, crosses, sustained window, first-fire, cooldown, replay suppression, additive fan-out, rule persistence) | Tasks 14–22 |
| Settings (TOML load/save, dirs::config_dir) | Task 21 |
| Error handling (thiserror per module) | Integrated into Tasks 11 (PluginError), 21 (SettingsError) |

**Gaps:** None. All spec sections for `rushhft-core` are covered.

### 2. Placeholder scan

No "TBD", "TODO", "implement later", or incomplete sections found. Every code step has full implementation code.

### 3. Type consistency

- `MetricEvent` fields: `plugin`, `metric`, `exchange`, `symbol`, `value`, `timestamp`, `is_replay` — consistent across Tasks 14–22.
- `TriggerRule`: `rule_id`, `name`, `is_enabled`, `conditions`, `actions` — consistent.
- `TriggerCondition`: `condition_id`, `plugin`, `metric`, `exchange`, `symbol`, `operator`, `threshold`, `window` — consistent.
- `TriggerAction`: `action_type`, `cooldown_duration`, `cooldown_unit`, `rest_api` — consistent.
- `OrderBook::add_or_update_level` signature: `(&mut self, BookItem)` — consistent in Tasks 5–6.
- `OrderBookHub::subscribe` takes `Arc<dyn Fn(&OrderBook) + Send + Sync>` and returns `SubscriptionGuard` — consistent.
- `BaseDataRetriever::start_with_reconnect` takes `Arc<dyn PluginContext>` and `impl Fn() -> BoxFuture<'static>` — consistent with spec.
- `BaseStudy::add_calculation` takes `BaseStudyModel` — consistent.
- `BaseStudy::start_consumer` takes `F: Fn(&BaseStudyModel) + Send + Sync + 'static` — consistent.

No naming inconsistencies found.

---

## Execution Handoff

Plan complete and saved to `docs/superpowers/plans/2026-08-10-rushhft-core.md`. Two execution options:

**1. Subagent-Driven (recommended)** — I dispatch a fresh subagent per task, review between tasks, fast iteration

**2. Inline Execution** — Execute tasks in this session using executing-plans, batch execution with checkpoints

**Which approach?**
