# RushHFT — Rust + Tauri Rewrite of VisualHFT (Design Spec)

**Status:** Approved 2026-08-10
**Scope:** MVP / Foundation
**License:** Apache-2.0
**Source of inspiration:** `../VisualHFT` (C# WPF, .NET 10, Windows-only, ~47k LOC C# + 49 XAML views)
**Data source:** LongPort OpenAPI (HK/US equities) via the sibling `longport` Rust crate at `../openapi/rust`

---

## 1. Overview

RushHFT is a cross-platform (macOS / Windows / Linux) desktop application for real-time market microstructure analysis. It is a fresh Rust + Tauri implementation that takes VisualHFT's architecture as its starting point, but uses **LongPort** (HK/US equities) as its sole data source for the MVP rather than crypto exchanges.

The architecture is a one-way pipeline:

```
LongPort WS (protobuf, handled by the `longport` crate)
        ↓
LongPortConnector (maps PushEvent → normalized models)
        ↓
Pub/sub hub in rushhft-core (lock-free)
        ↓
SnapshotStore (per-symbol RwLock, polled by UI) + Studies (compute off-thread)
        ↓
Tauri IPC `get_snapshot` → Svelte 5 frontend (60fps via requestAnimationFrame)
```

Low-frequency events (notifications, trigger fires, provider status) bypass the polling path and flow through a Tauri `Channel` to the frontend.

## 2. Scope

### In scope (MVP)

- 4-crate Cargo workspace: `rushhft-core`, `rushhft-connector-longport`, `rushhft-studies`, `rushhft-app`.
- **1 connector**: LongPort (depth + brokers + trade + quote pushes).
- **2 studies**: VPIN, LOB Imbalance.
- Trigger engine with rule persistence, sustained-window conditions, cooldown, replay semantics, UI + REST actions.
- Tauri 2 app with Svelte 5 + SvelteKit (static adapter) frontend.
- Dashboard layout A (classic HFT terminal — dense, single-screen).
- Canvas-rendered depth ladder **with broker-queue column** (LongPort's signature HK microstructure view).
- uPlot study time-series chart.
- Settings (manual paste of LongPort `app_key`/`app_secret`/`access_token`, plaintext TOML file).
- Compile-time plugin model (no dynamic loading).
- Cross-platform builds (macOS / Windows / Linux) via GitHub Actions matrix.

### Out of scope (deferred)

- Other venue connectors (Binance, Coinbase, Bitstamp, Kraken, KuCoin, Bitfinex, Gemini, generic WebSocket). Plugin trait is generic — adding later is a new crate, no core changes.
- Market Resilience + OTT Ratio studies. `BaseStudy` is designed so these slot in later.
- Dynamic plugin loading (DLL/.so scanning). Compile-time only.
- License tiers (COMMUNITY/AMATEUR/PRO/ENTERPRISE). Single tier for MVP.
- Strategy / Position / PnL views (`demoTradingCore` equivalent). Deferred.
- Multi-venue price comparison view.
- Statistics / Strategy Overview views.
- LongPort `TradeContext` (private order stream) — quote/depth/trade/brokers only.
- OAuth flow — manual token paste only for MVP. Add `tauri-plugin-shell` + keychain storage later.
- Candlestick subscriptions — the uPlot study chart streams `BaseStudyModel` values, not candles.

## 3. Architecture

```
┌─────────────────────────────────────────────────────────────────────────┐
│                         rushhft-app  (Tauri binary)                     │
│                                                                         │
│  Plugin Registry   Snapshot Store   Tauri IPC commands                  │
│  (starts/stops    (per-symbol      • get_snapshot                      │
│   connectors +      RwLock<OrderBook • get_studies                      │
│   studies)          + studies>)       • start/stop_plugin               │
│                                       • save_settings                  │
│                                       • get_triggers / save_trigger     │
│                                       • test_trigger_rest              │
│                                       • get_providers                  │
│  ┌────────────────────────────────────────────────────────────┐         │
│  │ 60fps polling (requestAnimationFrame → get_snapshot →     │         │
│  │ Svelte stores). Low-freq events (Tauri Channel):           │         │
│  │ notifications, trigger fires, status.                       │         │
│  └────────────────────────────────────────────────────────────┘         │
└─────────┬────────────────────────────────────────────┬────────────────┘
          │ Plugin trait                                 │ serde DTOs
          ▼                                              ▼
┌────────────────────────────┐   ┌──────────────────────────────────────┐
│   rushhft-core (lib)        │   │  Frontend (Svelte 5 + SvelteKit)      │
│                            │   │                                      │
│  Models:                   │   │  TopBar (provider/symbol selector)    │
│   • OrderBook (Decimal)    │   │  Depth Ladder (canvas + broker queue) │
│   • BookItem (+broker_ids)│   │  L2 List   Trades                     │
│   • Trade                  │   │  Study Tiles   Study Chart (uPlot)     │
│   • BaseStudyModel         │   │  Plugin Manager • Trigger Config •    │
│   • Provider, enums         │   │  Settings • Notifications             │
│                            │   │                                      │
│  Plugin trait              │   │  Stores (Svelte 5 runes):             │
│  Pub/sub hub (lock-free)   │   │   • snapshot (polled 60fps)           │
│  Pools                     │   │   • providers, symbols                │
│  TriggerEngine             │   │   • notifications (Tauri Channel)      │
│  Settings (TOML)           │   └──────────────────────────────────────┘
└─────────────┬──────────────┘
              │ impl Plugin
              ▼
┌──────────────────────────┐   ┌──────────────────────────────────────┐
│ rushhft-connector-       │   │  rushhft-studies (lib)                │
│   longport (lib)         │   │                                      │
│                          │   │  • VpinStudy (impl Plugin, emits metric)│
│  LongPortConnector       │   │    — trade-direction buckets          │
│   (impl Plugin)          │   │    (uses LongPort Trade.direction     │
│                          │   │     directly — no tick rule)           │
│  Wraps `longport` crate  │   │                                       │
│   (path dep ../openapi)  │   │  • LobImbalanceStudy                   │
│                          │   │    (depth bid/ask imbalance)          │
│  Maps PushEvent →         │   └──────────────────────────────────────┘
│   normalized OrderBook /  │
│   Trade / Provider        │
│                          │
│  Subscribes:              │
│   SubFlags::DEPTH |       │
│   BROKER | TRADE | QUOTE  │
└──────────────────────────┘
```

### Workspace layout

```toml
# /Cargo.toml (workspace root)
[workspace]
resolver = "3"
members = [
    "rushhft-core",
    "rushhft-connector-longport",
    "rushhft-studies",
    "rushhft-app",
]
```

The sibling `openapi` repo (containing the `longport` Rust SDK) is referenced via a path dependency at `../openapi/rust` and committed as a git submodule for reproducibility (mirrors the original VisualHFT/oxyplot sibling-repo pattern).

## 4. Crate: `rushhft-core`

The shared core: models, plugin trait, pub/sub hub, pools, trigger engine, settings. No deps on tokio WSDL or Tauri.

### Domain models (`rushhft_core::model`)

Prices and sizes use `rust_decimal::Decimal` (LongPort gives us this — `f64` is avoided throughout). Timestamps use `time::OffsetDateTime` (what the `longport` crate uses).

```rust
pub struct OrderBook {
    pub symbol: String,
    pub bids: Vec<BookItem>,   // sorted desc by price
    pub asks: Vec<BookItem>,   // sorted asc by price
    pub max_depth: usize,
    pub price_decimal_places: u8,
    pub size_decimal_places: u8,
    pub provider_id: i32,
    pub sequence: i64,
    pub last_updated: OffsetDateTime,
    pub imbalance_value: Decimal,
    // delta counters (added/deleted/updated levels + scaled volumes) — ported from original
    pub added_levels: AtomicU64,
    pub deleted_levels: AtomicU64,
    pub updated_levels: AtomicU64,
    pub added_volume_scaled: AtomicU64,
    pub deleted_volume_scaled: AtomicU64,
}

pub struct BookItem {
    pub price: Decimal,
    pub size: Decimal,
    pub cumulative_size: Decimal,
    pub is_bid: bool,
    pub broker_ids: Vec<i32>,             // NEW vs VisualHFT — LongPort Brokers push
    pub entry_id: Option<String>,
    pub local_timestamp: OffsetDateTime,
    pub server_timestamp: OffsetDateTime,
    pub symbol: String,
    pub provider_id: i32,
}

pub struct Trade {
    pub price: Decimal,
    pub size: Decimal,                    // volume in shares
    pub timestamp: OffsetDateTime,
    pub direction: TradeDirection,         // Neutral/Down/Up — direct from LongPort
    pub trade_type: String,               // "D"/"M"/"P"/"U"/"X"/"Y"/"" (HK); "A"/"B"/"D"... (US)
    pub symbol: String,
    pub provider_id: i32,
    pub market_mid_price: Decimal,
}

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

pub struct Provider { pub id: i32, pub name: String, pub status: SessionStatus }

pub enum SessionStatus { Connecting, Connected, ConnectedWithWarnings,
                          DisconnectedFailed, Disconnected }
pub enum TradeDirection { Neutral, Down, Up }
pub enum LobSide { None, Bid, Ask, Both }       // bitflags
pub enum PluginType { Unknown, Study, MultiStudy, MarketConnector }
pub enum PluginStatus { Loaded, Starting, Started, Stopping, Stopped, StoppedFailed }
pub enum MdUpdateAction { New, Change, Delete, ChangeAdjust, Replace, None }
pub enum AggregationLevel { None, Ms1, Ms10, Ms100, Ms500, S1, S3, S5, D1 }
```

Enums are a 1:1 port from `VisualHFT.Commons/Model/enums.cs` where they overlap.

### Plugin trait (`rushhft_core::plugin`)

```rust
#[async_trait::async_trait]
pub trait Plugin: Send + Sync {
    fn name(&self) -> &str;
    fn version(&self) -> &str;
    fn author(&self) -> &str;
    fn description(&self) -> &str;
    fn plugin_type(&self) -> PluginType;
    fn status(&self) -> PluginStatus;
    fn plugin_id(&self) -> &str;           // SHA256 of name+author+version+description
    fn emits_metric(&self) -> bool { false }
    async fn start(&self, ctx: Arc<dyn PluginContext>) -> Result<()>;
    async fn stop(&self) -> Result<()>;
    fn get_ui_settings(&self) -> Option<Box<dyn std::any::Any>> { None }
}

#[async_trait::async_trait]
pub trait PluginContext: Send + Sync {
    async fn publish_order_book(&self, ob: OrderBook);
    async fn publish_trade(&self, t: Trade);
    async fn publish_provider(&self, p: Provider);
    async fn register_metric(&self, plugin: &str, metric: &str, exchange: &str,
                             symbol: &str, value: Decimal, ts: OffsetDateTime);
    fn snapshot_store(&self) -> Arc<SnapshotStore>;
    fn order_book_hub(&self) -> Arc<OrderBookHub>;
    fn trade_hub(&self) -> Arc<TradeHub>;
    fn provider_hub(&self) -> Arc<ProviderHub>;
}

pub struct BaseDataRetriever { /* reconnection state, semaphore, atomic flags */ }
impl BaseDataRetriever {
    pub async fn start_with_reconnect(&self, ctx: Arc<dyn PluginContext>,
                                       internal_start: impl Fn() -> BoxFuture<()>);
    // Exponential backoff with jitter, max 5 attempts. Atomic check-and-set prevents
    // concurrent reconnection storms. Status transitions: Starting → Started,
    // or → StoppedFailed after max attempts. Mirrors BasePluginDataRetriever.cs.
}

pub struct BaseStudy { /* queue + aggregation */
    queue: tokio::sync::mpsc::UnboundedSender<BaseStudyModel>,
    agg: AggregatedCollection<BaseStudyModel>,
}
impl BaseStudy {
    pub fn add_calculation(&self, e: BaseStudyModel);   // sends to queue
    pub async fn start_consumer(&self, on_calculated: Arc<dyn Fn(&BaseStudyModel) + Send + Sync>);
    // AggregationLevel configurable per study. onDataAggregation callback
    // default: invoke on_calculated with the last item.
}
```

### Pub/sub hub (`rushhft_core::hub`)

```rust
pub struct OrderBookHub {
    // Lock-free subscribers via arc_swap::ArcSwap<Vec<Arc<dyn Fn(&OrderBook) + Send + Sync>>>
    // (replaces C# ImmutableInterlocked<ImmutableArray<Action<OrderBook>>>)
    subscribers: ArcSwap<Vec<Arc<dyn Fn(&OrderBook) + Send + Sync>>>,
    latest: DashMap<String, ArcSwap<OrderBook>>,    // per-symbol latest snapshot
}
impl OrderBookHub {
    pub fn subscribe(&self, f: Arc<dyn Fn(&OrderBook) + Send + Sync>) -> SubscriptionGuard;
    pub fn publish(&self, ob: OrderBook);
    pub fn snapshot(&self, symbol: &str) -> Option<Arc<OrderBook>>;
    pub fn symbols(&self) -> Vec<String>;
}
```

`TradeHub`, `ProviderHub` follow the same shape.

Key difference from C#: subscribers receive `&OrderBook` via `Arc` (shared ref). No pooled-DTO hand-off needed — `Arc` clone is cheap and the snapshot store holds the long-lived copy. Studies transform to their own working struct before computing.

Subscriber fan-out wraps each call in `std::panic::catch_unwind(AssertUnwindSafe(f))` — one bad subscriber cannot poison the fan-out. Errors are logged via `tracing::error!` and surfaced to `NotificationHub`.

### Pools (`rushhft_core::pool`)

```rust
pub struct ObjectPool<T> { /* crossbeam-queue backed, sized */ }
impl<T: Default + Clone> ObjectPool<T> {
    pub fn get(&self) -> PoolGuard<T>;    // RAII guard, auto-returns on drop
    pub fn try_get(&self) -> Option<PoolGuard<T>>;
}

pub type BookItemPool = ObjectPool<BookItem>;
pub type TradePool = ObjectPool<Trade>;

pub struct RollingWindow<T: Copy + Default> { /* ring buffer with O(1) push + sliding-window avg */ }
```

Less central than in C# (because `Arc` snapshots reduce per-tick allocation), but kept for the connector's per-tick hot path where we map `PushDepth → OrderBook`.

### Trigger engine (`rushhft_core::trigger`)

Direct port of `TriggerEngineService.cs` with `tokio::sync::mpsc::unbounded_channel<MetricEvent>` replacing `Channel<MetricEvent>`:

```rust
pub struct MetricEvent {
    pub plugin: String,
    pub metric: String,
    pub exchange: String,
    pub symbol: String,
    pub value: Decimal,
    pub timestamp: OffsetDateTime,
    pub is_replay: bool,
}

pub struct TriggerRule {
    pub rule_id: i64,
    pub name: String,
    pub is_enabled: bool,
    pub condition: Vec<TriggerCondition>,
    pub actions: Vec<TriggerAction>,
}

pub struct TriggerCondition {
    pub condition_id: i64,
    pub plugin: String,
    pub metric: String,
    pub operator: ConditionOperator,    // Equals | GreaterThan | LessThan | CrossesAbove | CrossesBelow
    pub threshold: Decimal,
    pub window: Option<TimeWindow>,    // sustained condition window
}

pub struct TriggerAction {
    pub action_type: ActionType,        // RestApi | UIAlert
    pub cooldown_duration: i32,
    pub cooldown_unit: TimeWindowUnit,   // Seconds | Minutes | Hours | Days
    pub rest_api: Option<RestApiConfig>,
}

pub struct TriggerEngine {
    rules: RwLock<Vec<TriggerRule>>,
    last_metric_values: DashMap<String, (Decimal, OffsetDateTime)>,
    condition_start_times: DashMap<String, OffsetDateTime>,
    action_last_fired_times: DashMap<String, OffsetDateTime>,
    metric_rx: Mutex<mpsc::UnboundedReceiver<MetricEvent>>,
    metric_tx: mpsc::UnboundedSender<MetricEvent>,
    on_trigger_fired: ArcSwap<Vec<Arc<dyn Fn(TriggerFiredEventArgs) + Send + Sync>>>,
}
impl TriggerEngine {
    pub async fn start(self: Arc<Self>);
    pub fn register_metric(&self, e: MetricEvent);
    pub fn add_or_update_rule(&self, rule: TriggerRule);   // persists + replays
    pub fn remove_rule(&self, rule_id: i64);
    pub fn get_rules(&self) -> Vec<TriggerRule>;
    pub fn on_trigger_fired(&self, f: Arc<dyn Fn(TriggerFiredEventArgs) + Send + Sync>);
}
```

Invariants ported line-for-line from `TriggerEngineService.cs` (these are subtle and well-tested in the original — do not deviate):

- Metrics flow through an unbounded channel; consumers process off-thread.
- `is_replay` flags observations re-presented after a rule-config edit. Replays must **never re-fire** an action that already fired for the original observation.
- `on_trigger_fired` is an additive fan-out — subscribers are invoked individually via `catch_unwind` so a throwing handler does not break others.
- `last_metric_values`, `condition_start_times`, `action_last_fired_times` must stay consistent. Sustained-condition window uses the **observation** timestamp, not `OffsetDateTime::now_utc()`.
- First-fire from a clean state must fire (do not just record the timestamp).

Config stored at `config_dir()/RushHFT/triggers.toml`.

### Settings (`rushhft_core::settings`)

```rust
pub struct Settings {
    pub app_key: String,
    pub app_secret: String,
    pub access_token: String,
    pub default_symbols: Vec<String>,     // e.g. ["700.HK", "AAPL.US"]
    pub depth_levels: usize,              // default 10
    pub aggregation_level: AggregationLevel,
    pub log_level: String,                // default "info"
}
impl Settings {
    pub fn load() -> Result<Self>;        // dirs::config_dir()/RushHFT/config.toml
    pub fn save(&self) -> Result<()>;
    pub fn default() -> Self;
}
```

`dirs::config_dir()` → macOS `~/Library/Application Support/RushHFT/`, Linux `~/.config/RushHFT/`, Windows `%APPDATA%\RushHFT\`. Triggers stored in `triggers.toml` alongside.

### Logging & errors

- `tracing` (structured, async-aware) — what the `longport` crate uses. Replaces log4net.
- `thiserror` for `rushhft-core`, `rushhft-connector-longport`, `rushhft-studies` library errors. `anyhow` in `rushhft-app` for top-level orchestration.

## 5. Crate: `rushhft-connector-longport`

Thin wrapper around the sibling `longport` crate. Implements `Plugin`.

### Cargo

```toml
[dependencies]
rushhft-core = { path = "../rushhft-core" }
longport = { path = "../../openapi/rust" }
tokio = { version = "1", features = ["rt-multi-thread", "macros", "sync", "time"] }
async-trait = "0.1"
tracing = "0.1"
thiserror = "1"
rust_decimal = "1"
time = { version = "0.3", features = ["serde-human-readable"] }
dashmap = "6"
arc-swap = "1"
```

### Connector

```rust
pub struct LongPortConnector {
    id: String,
    status: ArcSwap<PluginStatus>,
    settings: Arc<RwLock<ConnectorSettings>>,
    ctx: Arc<dyn PluginContext>,
    quote_ctx: Mutex<Option<Arc<QuoteContext>>>,
    receiver: Mutex<Option<PushEventReceiver>>,
    local_books: DashMap<String, OrderBook>,
    base: BaseDataRetriever,
}

pub struct ConnectorSettings {
    pub app_key: String,
    pub app_secret: String,
    pub access_token: String,
    pub symbols: Vec<String>,
    pub depth_levels: usize,
    pub sub_flags: SubFlags,           // DEPTH | BROKER | TRADE | QUOTE by default
}

#[async_trait]
impl Plugin for LongPortConnector {
    fn name(&self) -> &str { "LongPort Connector" }
    fn plugin_type(&self) -> PluginType { PluginType::MarketConnector }
    fn status(&self) -> PluginStatus { self.status.load_full().clone() }
    fn plugin_id(&self) -> &str { &self.id }
    async fn start(&self, ctx: Arc<dyn PluginContext>) -> Result<()> {
        self.base.start_with_reconnect(ctx.clone(), self.make_start_fn()).await
    }
    async fn stop(&self) -> Result<()> { /* drain receiver, drop quote_ctx */ }
}

impl LongPortConnector {
    async fn internal_start(&self) -> Result<()> {
        // 1. Build Config::new(app_key, app_secret, access_token) from settings
        // 2. QuoteContext::new(Arc::new(config)) → (ctx, receiver)
        // 3. ctx.subscribe(&symbols, sub_flags).await
        // 4. Spawn consumer task: loop { receiver.recv().await → handle_push_event }
        // 5. status ← Started
    }

    fn handle_push_event(&self, event: PushEvent) {
        match event.detail {
            PushEventDetail::Depth(d)    => self.on_depth(&event.symbol, d),
            PushEventDetail::Brokers(b)  => self.on_brokers(&event.symbol, b),
            PushEventDetail::Trade(t)    => self.on_trade(&event.symbol, t),
            PushEventDetail::Quote(q)    => self.on_quote(&event.symbol, q),
            PushEventDetail::Candlestick(_) => {}  // not in MVP
        }
    }

    fn on_depth(&self, symbol: &str, d: PushDepth) {
        // LongPort sends a full snapshot of the depth ladder each push (not a delta).
        // Replace local OrderBook bids/asks with the new ladder,
        // recompute cumulative_size + imbalance_value,
        // preserve any broker_ids we already had (they come on the Brokers push),
        // publish to OrderBookHub (updates latest + fans out to studies).
    }

    fn on_brokers(&self, symbol: &str, b: PushBrokers) {
        // Merge broker_ids into the existing OrderBook at each position.
        // Position 1..N maps to asks[0..N] then bids[0..N] (verify ordering convention
        // in the longport crate during implementation).
        // Publish updated OrderBook.
    }

    fn on_trade(&self, symbol: &str, t: PushTrades) {
        for trade in t.trades {
            let normalized = Trade {
                price: trade.price,
                size: Decimal::from(trade.volume),
                timestamp: trade.timestamp,
                direction: trade.direction,        // Neutral/Down/Up — direct, no tick rule
                trade_type: trade.trade_type,
                symbol: symbol.to_string(),
                provider_id: self.settings.provider_id,
                market_mid_price: self.local_books[symbol].mid_price(),
            };
            self.ctx.publish_trade(normalized);
        }
    }

    fn on_quote(&self, symbol: &str, q: PushQuote) {
        // Store OHLC + last_done + trade_status as a QuoteStats struct in the
        // SnapshotStore (per-symbol). Used for the top-bar ticker + status
        // indicator. The QuoteStats surfaces as `quote_stats: Option<QuoteStatsDto>`
        // on the polled SnapshotDto.
    }
}
```

### What this connector does NOT do

- OAuth flow — manual paste only for MVP.
- Private trade context (`TradeContext` in longport crate) — no order/execution/position stream. Quote + Depth + Brokers + Trade only.
- Candlestick subscriptions — not needed; the uPlot study chart streams `BaseStudyModel` values, not candles.
- Multi-region failover — LongPort is one venue.

### Reconnection

`BaseDataRetriever` in `rushhft-core` provides the orchestration (atomic check-and-set, exponential backoff with jitter, max 5 attempts, status transitions). The connector supplies `internal_start` as the retry closure — same pattern as `BinancePlugin.SetReconnectionAction(InternalStartAsync)` in the original.

LongPort's own WS client also has reconnection logic internally — we let it handle transient disconnects and only escalate to our reconnection orchestration when the SDK gives up (or fails to start). This avoids double-reconnect loops.

### Testing

- **Unit tests** (`tests/depth_mapping.rs`): feed a captured `PushDepth` protobuf blob → assert the normalized `OrderBook` is correct (bids desc, asks asc, cumulative sizes, imbalance value).
- **Integration tests** (`tests/replay.rs`): replay a recorded stream of `PushEvent`s (JSON fixtures under `tests/fixtures/`) through the connector, assert the snapshot store state at the end.
- **No live network in tests** — fixtures cover the surface. Live-connection smoke test is a manual `cargo tauri dev` step.

## 6. Crate: `rushhft-studies`

Two studies for MVP: VPIN, LOB Imbalance. Both implement `Plugin` (variant `Study`), extend `BaseStudy` for queue/aggregation, set `emits_metric = true` so the trigger engine picker lists them.

### Cargo

```toml
[dependencies]
rushhft-core = { path = "../rushhft-core" }
tokio = { version = "1", features = ["rt-multi-thread", "macros", "sync"] }
async-trait = "0.1"
tracing = "0.1"
rust_decimal = "1"
time = "0.3"
dashmap = "6"
arc-swap = "1"
```

### VPIN (`VpinStudy`)

Volume-Synchronized Probability of Informed Trading — Easley/Lopez de Prado & O'Hara (2012).

`VPIN = (1/n) × Σ|V_buy_i − V_sell_i| / V_bucket` over n completed buckets. Range [0, 1].

```rust
pub struct VpinStudy {
    id: String,
    status: ArcSwap<PluginStatus>,
    settings: Arc<RwLock<VpinSettings>>,
    ctx: Arc<dyn PluginContext>,
    base: BaseStudy,
    // VPIN state (same as original VPINStudy.cs)
    bucket_volume_size: Decimal,
    current_bucket_volume: Decimal,
    current_buy_volume: Decimal,
    current_sell_volume: Decimal,
    last_market_mid_price: Decimal,
    bucket_imbalances: Vec<Decimal>,      // rolling ring buffer of N buckets
    buffer_index: usize, buffer_count: usize, rolling_sum: Decimal,
    lock: Mutex<()>,                      // serializes on_trades / on_order_book
}

pub struct VpinSettings {
    pub bucket_volume_size: Decimal,     // default 1
    pub number_of_buckets: usize,         // default 50
    pub symbol: String,
    pub provider_id: i32,
    pub aggregation_level: AggregationLevel,   // forced to S1 (1 second)
}
```

**Key simplification vs. the original:** the C# VPINStudy classifies trades via tick rule (price ≥ mid → buy, price < mid → sell), with a fallback to the provider's `IsBuy` flag. **LongPort gives `Trade.direction` directly** (Neutral/Down/Up) — `Down` → sell, `Up` → buy, `Neutral` → split 50/50 or skip. No tick rule, no mid-price dependency for classification. Mid-price is still tracked for the `BaseStudyModel.market_mid_price` field.

The rest of the algorithm (bucket overflow handling, rolling window with O(1) average, interim updates) is a direct line-for-line port of `VPINStudy.TRADES_OnDataReceived` + `DoCalculation`.

### LOB Imbalance (`LobImbalanceStudy`)

```rust
pub struct LobImbalanceStudy {
    id: String, status: ArcSwap<PluginStatus>,
    settings: Arc<RwLock<LobImbalanceSettings>>,
    ctx: Arc<dyn PluginContext>,
    base: BaseStudy,
}

pub struct LobImbalanceSettings {
    pub symbol: String,
    pub provider_id: i32,
    pub levels: usize,                    // how many levels deep to sum (default 5)
    pub aggregation_level: AggregationLevel,   // default S1
}
```

Formula (mirrors the original `OrderFlowAnalysis.Calculate_OrderImbalance`): for the top `levels` price levels, `imbalance = (Σ bid_size − Σ ask_size) / (Σ bid_size + Σ ask_size)`, range [−1, 1].

Subscribes to `OrderBookHub`. On each publish, recomputes imbalance and calls `add_calculation(BaseStudyModel { value, … })`. The `BaseStudy` queue decouples the hot path (book publish) from the aggregation cadence.

### Plugin trait shape

```rust
#[async_trait]
impl Plugin for VpinStudy {
    fn name(&self) -> &str { "VPIN Study" }
    fn plugin_type(&self) -> PluginType { PluginType::Study }
    fn emits_metric(&self) -> bool { true }
    async fn start(&self, ctx: Arc<dyn PluginContext>) -> Result<()> {
        // ctx.order_book_hub().subscribe(Arc::new(|ob| self.on_order_book(ob)));
        // ctx.trade_hub().subscribe(Arc::new(|t| self.on_trade(t)));
        // reset bucket state
        // status ← Started
    }
    async fn stop(&self) -> Result<()> { /* unsubscribe, status ← Stopped */ }
}
```

`LobImbalanceStudy` follows the same shape but only subscribes to `OrderBookHub`, not `TradeHub`.

### Deferred studies

- **Market Resilience** (+ bias variant, `P2Quantile`, `StatisticalHelper`) — the original's most complex study, ~600 LOC. Deferred.
- **OTT Ratio** (Order-to-Trade) — straightforward but lower value for MVP than Resilience. Deferred.

The plugin trait + `BaseStudy` are designed so adding these later is a new file in `rushhft-studies`, no core changes.

### Testing

- **Unit**: feed a scripted sequence of `Trade`s with known directions → assert bucket completion + VPIN value at each step. Same for `OrderBook` snapshots + imbalance.
- **Replay tests**: reuse the `tests/fixtures/` recorded streams from the connector tests; assert both studies produce expected `BaseStudyModel` series.
- **Property tests** (optional, via `proptest`): VPIN ∈ [0, 1] for any trade sequence; imbalance ∈ [−1, 1] for any book.

## 7. Crate: `rushhft-app` (Tauri binary)

Integration layer. Wires plugins, exposes IPC, hosts Svelte UI, manages lifecycle. Tauri 2.x.

### Cargo

```toml
[dependencies]
rushhft-core = { path = "../rushhft-core" }
rushhft-connector-longport = { path = "../rushhft-connector-longport" }
rushhft-studies = { path = "../rushhft-studies" }
tauri = { version = "2", features = ["devtools"] }
tauri-plugin-shell = "2"
tokio = { version = "1", features = ["rt-multi-thread", "macros", "sync", "time"] }
serde = { version = "1", features = ["derive"] }
serde_json = "1"
tracing = "0.1"
tracing-subscriber = { version = "0.3", features = ["env-filter"] }
anyhow = "1"
rust_decimal = { version = "1", features = ["serde-with-str"] }
time = { version = "0.3", features = ["serde-human-readable", "formatting"] }
dashmap = "6"
arc-swap = "1"

[build-dependencies]
tauri-build = { version = "2" }
```

### App state (Tauri `State`)

```rust
pub struct AppState {
    pub snapshot_store: Arc<SnapshotStore>,
    pub trigger_engine: Arc<TriggerEngine>,
    pub plugins: Vec<Arc<dyn Plugin>>,             // compile-time list (LongPort + VPIN + LOB Imb)
    pub plugin_context: Arc<dyn PluginContext>,
    pub settings: Arc<RwLock<Settings>>,
    pub notification_channel: tauri::ipc::Channel<NotificationPayload>,
}

pub struct SnapshotStore {
    books: DashMap<String, ArcSwap<OrderBook>>,    // latest per symbol, lock-free reads
    studies: DashMap<String, DashMap<String, ArcSwap<BaseStudyModel>>>,  // symbol → study_name → value
    trades: DashMap<String, VecDeque<Trade>>,      // rolling recent trades per symbol (cap 200)
    providers: ArcSwap<Vec<Provider>>,
}
impl SnapshotStore {
    pub fn snapshot(&self, symbol: &str) -> SnapshotDto;
    pub fn update_book(&self, ob: OrderBook);
    pub fn update_study(&self, symbol: &str, name: &str, v: BaseStudyModel);
    pub fn append_trade(&self, t: Trade);
    pub fn providers(&self) -> Vec<Provider>;
}
```

### IPC commands

```rust
#[tauri::command]
async fn get_snapshot(state: State<'_, AppState>, symbol: String) -> SnapshotDto;

#[tauri::command]
async fn get_providers(state: State<'_, AppState>) -> Vec<ProviderDto>;

#[tauri::command]
async fn get_symbols(state: State<'_, AppState>) -> Vec<String>;

#[tauri::command]
async fn get_studies(state: State<'_, AppState>) -> Vec<StudyDescriptorDto>;

#[tauri::command]
async fn start_plugin(state: State<'_, AppState>, plugin_id: String) -> Result<()>;

#[tauri::command]
async fn stop_plugin(state: State<'_, AppState>, plugin_id: String) -> Result<()>;

#[tauri::command]
async fn get_settings(state: State<'_, AppState>) -> SettingsDto;

#[tauri::command]
async fn save_settings(state: State<'_, AppState>, settings: SettingsDto) -> Result<()>;

#[tauri::command]
async fn get_triggers(state: State<'_, AppState>) -> Vec<TriggerRuleDto>;

#[tauri::command]
async fn save_trigger(state: State<'_, AppState>, rule: TriggerRuleDto) -> Result<()>;

#[tauri::command]
async fn delete_trigger(state: State<'_, AppState>, rule_id: i64) -> Result<()>;

#[tauri::command]
async fn test_trigger_rest(state: State<'_, AppState>, rule_id: i64) -> Result<String>;

#[tauri::command]
async fn subscribe_notifications(state: State<'_, AppState>,
                                 channel: tauri::ipc::Channel<NotificationPayload>) -> Result<()>;
```

### DTOs (serde-friendly, frontend-facing)

```rust
#[derive(Serialize)]
pub struct SnapshotDto {
    pub symbol: String,
    pub bids: Vec<BookItemDto>,           // [{ price, size, cumulative, broker_ids }]
    pub asks: Vec<BookItemDto>,
    pub spread: Decimal,
    pub mid_price: Decimal,
    pub last_updated: i64,                 // epoch millis
    pub sequence: i64,
    pub provider_status: SessionStatusDto,
    pub studies: Vec<StudyValueDto>,       // [{ name, value, color, tooltip, stale }]
    pub recent_trades: Vec<TradeDto>,      // last 50
    pub quote_stats: Option<QuoteStatsDto>, // OHLC + last_done + trade_status
}

#[derive(Serialize, Clone)]
pub struct QuoteStatsDto {
    pub last_done: Decimal,
    pub open: Decimal,
    pub high: Decimal,
    pub low: Decimal,
    pub volume: i64,
    pub turnover: Decimal,
    pub trade_status: TradeStatusDto,     // e.g. Normal / Halted / Closing
    pub timestamp: i64,                   // epoch millis
}

#[derive(Serialize, Clone)]
pub struct BookItemDto {
    pub price: Decimal,
    pub size: Decimal,
    pub cumulative_size: Decimal,
    pub is_bid: bool,
    pub broker_ids: Vec<i32>,
}

#[derive(Serialize, Clone)]
pub struct StudyValueDto {
    pub name: String,
    pub value: Decimal,
    pub format: String,
    pub value_color: String,
    pub tooltip: String,
    pub has_error: bool,
    pub is_stale: bool,
    pub timestamp: i64,                   // epoch millis
}

#[derive(Serialize, Clone)]
pub struct TradeDto {
    pub price: Decimal,
    pub size: Decimal,
    pub timestamp: i64,                   // epoch millis
    pub direction: TradeDirectionDto,      // Neutral / Down / Up
    pub trade_type: String,
}

#[derive(Serialize, Clone)]
pub struct ProviderDto {
    pub id: i32,
    pub name: String,
    pub status: SessionStatusDto,
}

#[derive(Serialize, Clone)]
pub struct StudyDescriptorDto {
    pub plugin_id: String,
    pub name: String,
    pub version: String,
    pub description: String,
    pub plugin_type: PluginTypeDto,
    pub status: PluginStatusDto,
    pub emits_metric: bool,
}

#[derive(Serialize, Clone)]
pub struct SettingsDto {
    pub app_key: String,
    pub app_secret: String,               // masked in non-edit responses
    pub access_token: String,
    pub default_symbols: Vec<String>,
    pub depth_levels: usize,
    pub aggregation_level: AggregationLevelDto,
    pub log_level: String,
}

#[derive(Serialize, Clone)]
pub struct NotificationPayload {
    pub source: String,                    // plugin name or "TriggerEngine"
    pub message: String,
    pub level: NotificationLevelDto,       // Info / Warning / Error
    pub category: NotificationCategoryDto, // Plugin / TriggerEngine / System
    pub timestamp: i64,
    pub exception: Option<String>,
}

// Decimal → string (rust_decimal serializes as string by default to preserve precision;
// Svelte parses to number for display). OffsetDateTime → epoch millis (i64).
```

### App lifecycle

```rust
fn main() {
    tracing_subscriber::fmt().with_env_filter(EnvFilter::from_default_env()).init();
    let settings = Arc::new(RwLock::new(Settings::load().unwrap_or_default()));
    let snapshot_store = Arc::new(SnapshotStore::new());
    let trigger_engine = Arc::new(TriggerEngine::load());

    // Compile-time plugin list
    let plugins: Vec<Arc<dyn Plugin>> = vec![
        Arc::new(LongPortConnector::from_settings(&settings.read())?),
        Arc::new(VpinStudy::default()),
        Arc::new(LobImbalanceStudy::default()),
    ];
    let plugin_context = Arc::new(PluginContextImpl {
        snapshot_store: snapshot_store.clone(),
        trigger_engine: trigger_engine.clone(),
        // hubs created once, shared
    });

    // Spawn trigger engine consumer
    let te = trigger_engine.clone();
    tokio::spawn(async move { te.start().await });

    tauri::Builder::default()
        .plugin(tauri_plugin_shell::Builder::default().build())   // for opening LongPort OAuth URL later
        .manage(AppState { snapshot_store, trigger_engine, plugins, plugin_context, settings, /* ... */ })
        .invoke_handler(tauri::generate_handler![
            get_snapshot, get_providers, get_symbols, get_studies,
            start_plugin, stop_plugin, get_settings, save_settings,
            get_triggers, save_trigger, delete_trigger, test_trigger_rest,
            subscribe_notifications,
        ])
        .setup(|app| { /* auto-start LongPort connector + default studies */ Ok(()) })
        .on_window_event(|window, event| { /* graceful stop on close */ })
        .run(tauri::generate_context!())
        .expect("error while running RushHFT");
}
```

### Auto-start behavior

On launch:
1. Load settings.
2. If `app_key`/`app_secret`/`access_token` are present, auto-start `LongPortConnector` with the configured symbols + default sub flags.
3. Auto-start `VpinStudy` + `LobImbalanceStudy` against the first configured symbol.
4. If credentials missing, open the Settings view and prompt.

If the connector fails to start (bad token, network), `PluginStatus::StoppedFailed` is published → notification → user sees the error in the top-bar connectivity indicator + Notifications panel.

### Frontend bundling

- `rushhft-app/` contains both the Rust crate (`src-tauri/`) and the Svelte SPA (`ui/`).
- `tauri.conf.json`: `frontendDist = "../ui/build"`, `devUrl = "http://localhost:5173"`, `beforeBuildCommand = "pnpm build"`, `beforeDevCommand = "pnpm dev"`.
- `pnpm` for JS tooling (Vite + SvelteKit + svelte-preprocess).

## 8. Frontend (Svelte 5 + SvelteKit)

### Project layout

```
rushhft-app/                              # workspace member dir
├── src-tauri/                            # Tauri binary crate (this is `rushhft-app` in the workspace)
│   ├── Cargo.toml
│   ├── build.rs
│   ├── tauri.conf.json
│   ├── icons/
│   └── src/
│       └── main.rs
└── ui/                                   # Svelte 5 SPA (separate from Rust)
    ├── package.json
    ├── vite.config.ts
    ├── svelte.config.js
    ├── tsconfig.json
    └── src/
        ├── app.html
        ├── app.d.ts
        ├── routes/
        │   ├── +layout.svelte          # top-level shell + provider/symbol selector
        │   ├── +page.svelte            # Dashboard (layout A)
        │   ├── settings/+page.svelte
        │   ├── plugins/+page.svelte
        │   └── triggers/+page.svelte
        ├── lib/
        │   ├── ipc.ts                  # typed wrappers around invoke('get_snapshot', …)
        │   ├── stores/
        │   │   ├── snapshot.svelte.ts  # Svelte 5 runes — $state
        │   │   ├── providers.svelte.ts
        │   │   ├── notifications.svelte.ts  # fed by Tauri Channel
        │   │   └── triggers.svelte.ts
        │   ├── polling.ts              # requestAnimationFrame loop, drains to stores
        │   ├── canvas/
        │   │   ├── DepthLadder.ts      # custom canvas renderer (no Svelte reactivity per tick)
        │   │   └── DepthChart.ts       # cumulative-size depth chart (optional, MVP+ stretch)
        │   └── components/
        │       ├── TopBar.svelte
        │       ├── DepthLadder.svelte  # mounts <canvas>, drives DepthLadder.ts
        │       ├── L2OrderBook.svelte
        │       ├── Trades.svelte
        │       ├── StudyTile.svelte
        │       ├── StudyChart.svelte   # wraps uPlot
        │       ├── Notifications.svelte
        │       ├── PluginManager.svelte
        │       ├── TriggerConfig.svelte
        │       └── Settings.svelte
        └── app.css
```

### Polling loop (`lib/polling.ts`)

```typescript
import { invoke } from '@tauri-apps/api/core';
import { snapshotStore } from './stores/snapshot.svelte';

export function startPolling(getSymbol: () => string | null) {
  let stopped = false;
  async function frame() {
    if (stopped) return;
    const symbol = getSymbol();
    if (symbol) {
      try {
        const snap = await invoke<SnapshotDto>('get_snapshot', { symbol });
        snapshotStore.set(symbol, snap);
      } catch (e) { /* swallowed; next frame retries */ }
    }
    requestAnimationFrame(frame);
  }
  requestAnimationFrame(frame);
  return () => { stopped = true; };
}
```

**Coalescing** is automatic: if `get_snapshot` takes longer than 16ms, `requestAnimationFrame` simply schedules the next call after the current one resolves. Bursts of 500 updates/sec from LongPort collapse to ≤60 renders/sec.

### Canvas depth ladder (`lib/canvas/DepthLadder.ts`)

Why canvas: Svelte reconciliation at 60fps for a 20-row ladder with broker queues would burn CPU. Canvas bypasses the VDOM entirely — Rust hands us a `SnapshotDto`, we redraw.

```typescript
export class DepthLadder {
  private ctx: CanvasRenderingContext2D;
  private dpr: number;
  constructor(private canvas: HTMLCanvasElement) { /* setup ctx, dpr */ }

  render(snapshot: SnapshotDto, opts: { showBrokers: boolean; maxLevels: number }) {
    // 1. clear, set font/color
    // 2. render asks (top half, red, ascending by price)
    // 3. render spread row (mid price + spread value)
    // 4. render bids (bottom half, green, descending by price)
    // 5. if showBrokers: render broker_ids column right of size (mono font, 11px)
    // 6. cumulative-size horizontal bar (subtle background fill proportional to size)
  }
  // resize observer keeps the canvas pixel-perfect with devicePixelRatio
}
```

`DepthLadder.svelte` mounts the canvas, instantiates `DepthLadder`, and in a `$effect` calls `ladder.render(snapshot, …)` whenever `snapshotStore` updates. No per-row Svelte component.

### Study chart (`StudyChart.svelte` + uPlot)

```typescript
import uPlot from 'uplot';
import 'uplot/dist/uPlot.min.css';

let chart: uPlot;
let data: [number[], number[]] = [[], []];

onMount(() => {
  chart = new uPlot({
    width: container.clientWidth, height: 200,
    series: [{}, { stroke: '#7ee787', label: 'VPIN' }],
    scales: { x: { time: true }, y: { range: [0, 1] } },
  }, data, container);
  // ResizeObserver → chart.setSize(...)
});

$effect(() => {
  const vpin = snapshotStore.current?.studies.find(s => s.name === 'VPIN');
  if (vpin) appendPoint(vpin.timestamp, vpin.value);
});
```

uPlot holds up to ~100k points without breaking a sweat. Older points age out (sliding window of N minutes).

### Svelte 5 stores (`stores/snapshot.svelte.ts`)

```typescript
class SnapshotStore {
  current = $state<SnapshotDto | null>(null);
  history = $state<Map<string, SnapshotDto>>(new Map());
  set(symbol: string, snap: SnapshotDto) { this.current = snap; }
}
export const snapshotStore = new SnapshotStore();
```

Fine-grained reactivity — only components that read `snapshotStore.current.bids` re-render. `DepthLadder` bypasses that and redraws on the `$effect`.

### Low-freq events via Tauri Channel

```typescript
// in +layout.svelte onMount
import { Channel } from '@tauri-apps/api/core';
const ch = new Channel<NotificationPayload>();
ch.onmessage = (msg) => notificationsStore.push(msg);
await invoke('subscribe_notifications', { channel: ch });
```

Notifications, trigger fires, provider status changes flow through `ch.onmessage`. Svelte stores update; Svelte animates a toast.

### Dashboard view (`+page.svelte`, layout A)

```svelte
<script>
  import TopBar from '$lib/components/TopBar.svelte';
  import DepthLadder from '$lib/components/DepthLadder.svelte';
  import L2OrderBook from '$lib/components/L2OrderBook.svelte';
  import Trades from '$lib/components/Trades.svelte';
  import StudyTile from '$lib/components/StudyTile.svelte';
  import StudyChart from '$lib/components/StudyChart.svelte';
  import { snapshotStore } from '$lib/stores/snapshot.svelte';
  import { startPolling } from '$lib/polling';

  let symbol = $state('700.HK');
  const stop = startPolling(() => symbol);
  onDestroy(stop);
</script>

<TopBar bind:symbol />
<main class="dashboard-grid">
  <section class="depth-ladder"><DepthLadder snapshot={snapshotStore.current} showBrokers /></section>
  <section class="l2-and-trades">
    <L2OrderBook snapshot={snapshotStore.current} />
    <Trades trades={snapshotStore.current?.recent_trades} />
  </section>
  <section class="studies">
    <div class="tiles">
      {#each snapshotStore.current?.studies ?? [] as s}
        <StudyTile study={s} />
      {/each}
    </div>
    <StudyChart studies={snapshotStore.current?.studies ?? []} />
  </section>
</main>

<style>
  .dashboard-grid {
    display: grid;
    grid-template-columns: 220px 1fr 1fr;
    grid-template-rows: 1fr;
    gap: 6px;
    height: calc(100vh - 48px);
  }
</style>
```

### CSS theme

Dark-first, tokens matching the mockup:

- Background `#0d1117`, panel `#161b22`, border `#30363d`
- Bid green `#7ee787`, ask red `#f85149`, accent blue `#58a6ff`, muted `#8b949e`
- Mono font for prices/sizes (`JetBrains Mono` or `ui-monospace`), system sans for chrome

### Tooling

- `pnpm` for JS deps
- `vite` 5 + `@sveltejs/vite-plugin-svelte` 5
- `svelte-check` for type checking
- `@tauri-apps/api` 2.x for `invoke` + `Channel`
- `eslint` + `prettier` + `prettier-plugin-svelte`
- `vitest` for TS unit/component tests

## 9. Error handling

**Layered, idiomatic Rust:**

| Layer | Strategy |
|---|---|
| `rushhft-core` | `thiserror` — typed `Error` enum per module (`HubError`, `PoolError`, `TriggerError`, `SettingsError`). No `anyhow` in library crates. |
| `rushhft-connector-longport` | `thiserror` — `LongPortError` wrapping `longport::Error` variants + our own mapping/normalization errors. |
| `rushhft-studies` | `thiserror` — `StudyError` (mostly internal-state errors). Studies never panic on bad input — they emit `BaseStudyModel { has_error: true, tooltip: msg }` and continue. |
| `rushhft-app` | `anyhow::Result` at the top, `tauri::command` returns `Result<T, String>` (Tauri serializes the error to the frontend). Internal context preserved via `anyhow::Context`. |

**Hot-path invariant** (mirrors original): subscribers (`OrderBookHub::publish`) must not panic on a throwing handler — per-subscriber `catch_unwind` keeps one bad subscriber from poisoning the fan-out. Errors are logged via `tracing::error!` and surfaced to `NotificationHub` (a low-freq channel).

**Connector reconnection failures** escalate `PluginStatus::StoppedFailed` → frontend shows red dot + last error in the connectivity panel + Notifications. User can retry from the Plugin Manager view. Matches original behavior.

## 10. Testing strategy

| Crate | Test type | What |
|---|---|---|
| `rushhft-core` | Unit | `OrderBook` add/update/delete level correctness; delta counters; imbalance calc; object pool get/return; rolling window O(1) avg; trigger engine rule evaluation (direct, sustained window, cooldown, first-fire, replay suppression) — port the test cases from `tests/Integration/VisualHFT.TriggerService.Tests` |
| `rushhft-connector-longport` | Unit | `PushDepth` → `OrderBook` mapping (bids desc / asks asc, cumulative, broker queue merge); `PushTrades` → `Trade` mapping (direction, decimal precision); `PushQuote` → `QuoteStats` |
| `rushhft-connector-longport` | Integration (replay) | Recorded `PushEvent` streams in `tests/fixtures/*.json` → run through connector → assert `SnapshotStore` state at end. No network. |
| `rushhft-studies` | Unit | VPIN bucket completion + rolling avg (ported from original); LOB imbalance at known book states; both with scripted event sequences |
| `rushhft-studies` | Integration (replay) | Reuse connector fixtures; assert both studies produce expected `BaseStudyModel` series |
| `rushhft-app` | Smoke (manual) | `cargo tauri dev` → real LongPort connection with a paper-trading token → observe dashboard |
| Frontend | Unit (vitest) | Store reducers, polling loop logic, IPC type marshaling |
| Frontend | Component (vitest + @testing-library/svelte) | `StudyTile` rendering, `TriggerConfig` form validation |
| End-to-end | Manual | Playwright optional later — Tauri's webview is hard to drive via Playwright; defer |

**Test fixtures**: a `tests/fixtures/` directory shared across crates (workspace-level `tests/` dir) with captured `PushEvent` JSON. Recorded once via a small `examples/capture.rs` binary in `rushhft-connector-longport` that connects to LongPort for 30s and dumps events.

## 11. Tooling & CI

### Rust tooling

- `cargo` workspace, `resolver = "3"` (edition 2024, matches `longport` crate)
- `rust-toolchain.toml` pinning stable + `rustfmt` + `clippy` components
- `cargo deny` (licenses + advisories)
- `cargo nextest` for faster test runs in CI
- `cargo mutants` optional — verify test quality on the core math (VPIN, imbalance)

### Frontend tooling

- `pnpm` (workspace root has `package.json` for shared devDeps if needed)
- `vite` 5 + `@sveltejs/vite-plugin-svelte` 5
- `svelte-check` for type checking
- `eslint` + `prettier` + `prettier-plugin-svelte`
- `vitest` for unit/component tests

### Tauri

- `tauri-cli` 2.x (`cargo install tauri-cli --version "^2"`)
- `tauri.conf.json` lives at `rushhft-app/src-tauri/tauri.conf.json` (Tauri 2 convention). `frontendDist = "../ui/build"`, `devUrl = "http://localhost:5173"`, `beforeBuildCommand = "pnpm build"`, `beforeDevCommand = "pnpm dev"`.
- macOS: `app.macos_minimum_system_version = "11.0"`
- Code signing: defer to a release-engineering pass (MVP ships unsigned `.app`)

### CI (GitHub Actions)

Single workflow on push + PR:

```yaml
jobs:
  rust:
    strategy: { matrix: { os: [ubuntu-latest, macos-latest, windows-latest] } }
    steps:
      - uses: actions/checkout@v4
        with: { submodules: recursive }   # if openapi is a submodule
      - uses: dtolnay/rust-toolchain@stable
      - run: cargo fmt --all -- --check
      - run: cargo clippy --all --all-features -- -D warnings
      - run: cargo nextest run --workspace
  ui:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
      - uses: pnpm/action-setup@v3
      - run: pnpm install --frozen-lockfile
      - run: pnpm run check
      - run: pnpm run lint
      - run: pnpm run test
  build:
    needs: [rust, ui]
    strategy: { matrix: { os: [ubuntu-latest, macos-latest, windows-latest] } }
    steps:
      - uses: actions/checkout@v4
      - uses: tauri-apps/tauri-action@v0    # builds per-platform bundles
```

### Sibling `openapi` repo

Path-depended at `../openapi`. In CI, either (a) check it out as a sibling via a second `actions/checkout` with `path: ../openapi`, or (b) make it a git submodule. **Recommend (b) — submodule** — for reproducibility, with `--recurse-submodules` in the main checkout. Local dev clones both as siblings, matching the original VisualHFT/oxyplot sibling-repo pattern.

### Build & dev commands

```bash
# First-time setup
git clone --recurse-submodules <rushhft> && cd rushhft
cargo build --workspace                  # builds all Rust crates
cd rushhft-app/ui && pnpm install        # installs JS deps

# Dev (hot-reload both backend & frontend)
cargo tauri dev                          # from repo root

# Build release bundle per-platform
cargo tauri build

# Test
cargo nextest run --workspace
cd rushhft-app/ui && pnpm test
```

## 12. Conventions

- **Commit prefixes** (match VisualHFT repo): `feat:`, `fix:`, `docs:`, `test:`, `build:`, `ci:`, `perf:`, `refactor:`, with scope tag like `fix(connector):`, `feat(studies):`.
- **Branch naming**: `feat/<short>`, `fix/<short>`, `chore/<short>`.
- **PRs require**: green CI (fmt + clippy + tests on all 3 OSes), at least one review.
- **Changelog**: `CHANGELOG.md` following Keep a Changelog (mirrors `openapi` repo convention).
- **README**: Quickstart, architecture overview (link to this spec), plugin authoring guide (deferred — only one connector in MVP).

## 13. Decisions log

| Decision | Choice | Rationale |
|---|---|---|
| Scope | MVP / Foundation | Full parity is multi-month; foundation ships, is extendable, and exercises every architectural seam. |
| Plugin model | Compile-time Rust traits | Matches the actual C# workflow (ProjectReference + recompile). Zero ABI overhead, full type safety, easiest to test. |
| Platforms | macOS + Windows + Linux | Tauri + Rust are inherently cross-platform. The only Windows-specific code in VisualHFT was WPF itself. |
| Frontend | Svelte 5 + SvelteKit | Smallest bundle, simplest reactivity, excellent for high-frequency updates. |
| Charting | uPlot + custom canvas + lightweight-charts | Best perf, mirrors the original split (OxyPlot for studies, custom WPF canvas for depth ladder). |
| Real-time data flow | Shared state + 60fps polling | Decouples producer (socket) from consumer (UI). Bursts coalesce into one render. Mirrors original `CachedCollection` pattern. |
| Dashboard layout | Classic HFT terminal (A) | Mirrors original VisualHFT. Dense, single-screen, no tabs. Fastest to build for MVP. |
| App name | RushHFT | Fresh identity, no confusion with upstream C# project. |
| Message formats | Rust-native serde schemas | Cleanest, most maintainable. Field names map where they overlap with VisualHFT's `WS_input_json/` samples. |
| License | Apache-2.0 | Matches original VisualHFT. Permissive, commercial-friendly, lets code flow back upstream. |
| Venue | LongPort only (HK/US equities) | User request. Uses sibling `longport` Rust crate. Plugin trait remains generic for future venues. |
| Broker queue | Surface in depth ladder | LongPort's signature HK-market microstructure feature. The whole reason to use LongPort over a crypto venue. |
| Auth | Manual paste + plaintext config file | Simplest for MVP. Add OAuth + keychain storage later. |
| Workspace layout | Thin workspace, 4 crates | Mirrors VisualHFT's project layout. Enforced boundaries. Compiles in parallel. |
| Decimal type | `rust_decimal::Decimal` | LongPort gives `Decimal` natively. Avoids `f64` precision issues throughout. |
| Timestamp type | `time::OffsetDateTime` | What the `longport` crate uses. |
| Async runtime | `tokio` | What the `longport` crate uses. Dominant Rust async ecosystem. |
| Logging | `tracing` | Modern, structured, async-aware. What the `longport` crate uses. |
| Config format | TOML | Idiomatic Rust. `dirs::config_dir()/RushHFT/`. |
