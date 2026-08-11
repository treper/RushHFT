# VisualHFT Parity — Dashboard, Depth, Studies Implementation Plan

**Goal:** Bring RushHFT's UI and study set to information-parity with VisualHFT — same panels (sidebar + depth + charts + trades + positions stub + 4 toolbar modals), same study set (add Market Resilience + OTT Ratio), Rust/Tauri/Svelte-native visuals.

**Architecture:** Big-bang rebuild of `rushhft-app/ui` from a 3-column MVP into a structured Svelte component tree mirroring VisualHFT's UserControls/Views. Backend extends existing `commands.rs` / `dto.rs` / `state.rs` with chart-series buffers, runtime symbol subscribe, and a multi-venue price command. Two new study plugins in `rushhft-studies` (MR + OTT), backed by new `rushhft-core` stats helpers (P² quantile, rolling window).

**Tech Stack:** Rust 2024 / Tauri 2 / Svelte 5 / TypeScript / uPlot (charting)

---

## Context

VisualHFT is a mature C# WPF app at `/Users/tangning/Documents/workspace/mine/VisualHFT/`. RushHFT currently has a 3-column single-symbol MVP. This spec covers the bundle: **UI shell parity + depth ladder + missing studies**, executed as a big-bang rebuild.

RushHFT already has: LongPort connector, VPIN + LOB Imbalance studies, TriggerEngine, NotificationHub, SnapshotStore (lock-free), Plugin trait system. WebSocket proxy support through HTTP CONNECT is already in place.

## Design decisions (from brainstorming)

- **Fidelity target**: information parity, Rust-native UI (not pixel-matching WPF).
- **Layout**: VisualHFT-faithful sidebar (480px) + main area. Sidebar: toolbar (4 buttons) + provider status + scrollable study tiles. Main: ucOrderBook on top (~70%) + positions stub on bottom (~30%). Notification bell top-right.
- **Depth style**: combined ladder with size bars — single column, asks top (descending), spread row, bids below; each row has a horizontal size bar (background fill proportional to size). Bids bars grow left→right from the spread, asks grow right→left from the spread.
- **Charts**: include all 3 ucOrderBook charts (cumulative bids + cumulative asks + real-time price + spread). Backend adds per-symbol rolling buffers.
- **Positions pane**: stub empty-state ("No broker connected"). Layout matches VisualHFT; pane becomes useful when a broker plugin is added later.
- **Toolbar**: all 4 buttons wired up in this MVP — Plugins, Settings, Triggers, MultiVenue. MultiVenue shows LongPort only for now (single venue), future-proofed.
- **Studies**: VPIN + LOB Imbalance (already exist) + new Market Resilience + new OTT Ratio.
- **Theme**: keep current GitHub-dark (`#0d1117` bg, `#161b22` panel, `#30363d` border, `#58a6ff` accent, `#8b949e` muted). Red-up/green-down already set (`--bid: #f85149`, `--ask: #7ee787`).
- **Implementation**: Big bang — all panels built before shipping.
- **JS charting lib**: uPlot (~40 KB, canvas, time-series-optimized).

## Section 1 — File / component tree

```
rushhft-app/
├── src/
│   ├── main.rs                    (existing; minor: register new commands)
│   ├── commands.rs                (existing; add ~8 new IPC commands)
│   ├── dto.rs                     (existing; extend with chart-series DTOs)
│   ├── state.rs                   (existing; add ChartSeriesBuffer)
│   ├── notification.rs            (existing; wire bell button to it)
│   ├── context.rs                 (existing)
│   └── ui_state.rs                (NEW: tab/modal state, user-symbols registry)
└── ui/
    └── src/
        ├── routes/+page.svelte     (REPLACE: shell + layout A)
        ├── lib/
        │   ├── components/
        │   │   ├── Sidebar.svelte           (NEW)
        │   │   ├── Toolbar.svelte           (NEW: 4 buttons + bell)
        │   │   ├── ProviderStatus.svelte    (NEW)
        │   │   ├── StudyTiles.svelte        (NEW)
        │   │   ├── DepthLadder.svelte       (NEW: combined ladder + size bars)
        │   │   ├── TopOfBook.svelte         (NEW: big bid/ask + spread)
        │   │   ├── LOBImbalanceGauge.svelte (NEW: red↔white↔red gradient + arrow)
        │   │   ├── Charts/
        │   │   │   ├── CumulativeBook.svelte  (NEW, uPlot)
        │   │   │   ├── PriceChart.svelte      (NEW, uPlot)
        │   │   │   └── SpreadChart.svelte     (NEW, uPlot)
        │   │   ├── TradesTape.svelte         (NEW)
        │   │   └── Positions.svelte          (NEW: stub empty state)
        │   ├── modals/
        │   │   ├── PluginManagerModal.svelte (NEW)
        │   │   ├── SettingsModal.svelte       (NEW)
        │   │   ├── TriggersModal.svelte      (NEW)
        │   │   └── MultiVenueModal.svelte    (NEW)
        │   ├── stores/
        │   │   ├── snapshot.ts               (NEW: polling + Svelte stores)
        │   │   ├── symbols.ts                (NEW: symbol list, current symbol)
        │   │   ├── plugins.ts                (NEW)
        │   │   ├── settings.ts               (NEW)
        │   │   ├── triggers.ts               (NEW)
        │   │   └── notifications.ts         (NEW: channel-based)
        │   └── charts/
        │       ├── uPlotSetup.ts             (NEW: theme, scales)
        │       └── series.ts                 (NEW: series builders)
        └── app.css                  (EXTEND: panel/tile/gauge CSS)
```

Rust crate changes:
- `rushhft-studies`: 2 new plugin crates — `MarketResilienceStudy`, `OttRatioStudy`.
- `rushhft-core`: 2 new helpers — `P2Quantile` (port of VisualHFT's `Model/P2Quantile.cs`) → `rushhft-core/src/stats/p2_quantile.rs`; `RollingWindow` already exists at `rushhft-core/src/pool/rolling_window.rs` (re-exported from `pool`), reuse it for Decimal values — if its API is too narrow, add a generic `RollingWindow<T>` alongside it.
- `rushhft-connector-longport`: add `subscribe_symbol(symbol)` + `unsubscribe_symbol(symbol)` runtime methods.

## Section 2 — Data flow & new IPC commands

```
LongPortConnector (plugin)
   → pushes depth/trades/quote to OrderBookHub / TradeHub
   → SnapshotStore (lock-free ArcSwap, already exists) holds latest per-symbol snapshot
   → ChartSeriesBuffer (NEW, per-symbol ring buffer, last 600 points)
   → studies (VPIN, LOB Imb, MR, OTT) read from hubs, write study values back to SnapshotStore
   → Tauri commands (polled by Svelte)
   → Svelte stores
   → Components re-render
```

Polling strategy: keep 500ms snapshot poll; add 250ms poll for chart series (or use Tauri Channel push — see `subscribe_chart_series`). NotificationHub already uses `Channel<NotificationPayload>`; keep.

### New IPC commands (commands.rs)

| Command | Returns | Purpose |
|---|---|---|
| `get_chart_series(symbol, kind, points)` | `ChartSeriesDto` | One-shot fetch of last N points for `cumulative-bids` / `cumulative-asks` / `price` / `spread` |
| `subscribe_chart_series(symbol, channel)` | `()` | Channel-based push for chart updates (less polling) |
| `add_symbol(symbol)` | `Result<()>` | Runtime-subscribe a new symbol (calls connector method) |
| `remove_symbol(symbol)` | `Result<()>` | Unsubscribe |
| `get_plugin_descriptors()` | `Vec<PluginDescriptorDto>` | Generalize existing `get_studies` to describe all plugins (connectors + studies) |
| `get_multi_venue_prices(symbol)` | `Vec<VenuePriceDto>` | Per-venue bid/ask/last for the same symbol (currently just LongPort) |

Existing commands reused unchanged: `get_snapshot`, `get_providers`, `get_symbols`, `start_plugin`, `stop_plugin`, `get_settings`, `save_settings`, `get_triggers`, `save_trigger`, `delete_trigger`, `test_trigger_rest`, `subscribe_notifications`.

### Connector changes

`rushhft-connector-longport/src/lib.rs`: add `subscribe_symbol(symbol: &str)` and `unsubscribe_symbol(symbol: &str)` methods that call `quote_ctx.subscribe(...)` / `unsubscribe(...)` at runtime, not just at startup. `AppState` holds a `user_symbols: Arc<RwLock<HashSet<String>>>` alongside `default_symbols` from settings. On `save_settings`, persist `user_symbols` into settings so the list survives restart.

### New DTOs (dto.rs)

```rust
pub struct ChartPointDto {
    pub t: i64,                   // epoch millis
    pub value: Decimal,           // generic scalar (spread, mid, etc.)
    pub bid: Option<Decimal>,     // for price chart
    pub ask: Option<Decimal>,
    pub mid: Option<Decimal>,
}

pub struct ChartSeriesDto {
    pub kind: String,             // "cumulative-bids" | "cumulative-asks" | "price" | "spread"
    pub points: Vec<ChartPointDto>,
}

pub struct VenuePriceDto {
    pub venue: String,
    pub bid: Decimal,
    pub ask: Decimal,
    pub last: Decimal,
    pub timestamp: i64,
}

// StudyDescriptorDto → renamed/generalized to PluginDescriptorDto
// (already has plugin_id, name, version, description, plugin_type, status, emits_metric)
```

## Section 3 — New studies

### OttRatioStudy (rushhft-studies/src/ott_ratio/)

Algorithm (from `OrderToTradeRatioStudy.cs:1-80`):

```
OTR = (Adds + 2×Updates + Cancels) / max(Trades, 1) − 1
```

LongPort provides L2 (price-level) data, so use the L2 formula: derive `AddedΔ` / `UpdatedΔ` / `DeletedΔ` from `OrderBookHub` delta counters (the existing `OrderBook` model already tracks `_addedLevels` / `_deletedLevels`). Trade counts come from `TradeHub`.

- Aggregation period: configurable, default 1s (matches `AggregationLevel::S1`).
- Output metric: `OTT` (Decimal) — surfaces in trigger-rule picker.
- Thread safety: atomic counters, snapshot-then-reset at period boundary.
- Files: `rushhft-studies/src/ott_ratio/{mod.rs, aggregator.rs}` — port of `OrderToTradeRatioStudy.cs`.

### MarketResilienceStudy (rushhft-studies/src/market_resilience/)

Algorithm (from `MarketResilienceCalculator.cs:1-100`): detect shocks, measure recovery time.

1. Maintain P² quantile estimators (O(1) space, online median) for: spread, bid immediacy depth, ask immediacy depth, plus MAD (median absolute deviation) for each side.
2. Detect shocks: spread > 2σ above baseline, OR depth drops > 3σ below baseline, OR large trade (> 2σ trade-size).
3. On shock: start a timer. Watch for spread/depth to recover to 90% of pre-shock baseline. Record the recovery time in ms.
4. Output two metrics: `MR_SpreadRecovery` and `MR_DepthRecovery` (both rolling medians over last 500 events, in ms).
5. Skip the bias sub-study (Bullish/Bearish/Neutral) for MVP — just emit the two recovery metrics.

Files:
- `rushhft-studies/src/market_resilience/{mod.rs, calculator.rs}` — port of `MarketResilienceCalculator.cs`.

Both plug into the existing `Plugin` trait; `emits_metric = true` so the trigger engine can fire on `OTT > X` or `MR_SpreadRecovery > Y`.

### rushhft-core helpers

- `P2Quantile` (port of VisualHFT's `Model/P2Quantile.cs`) → `rushhft-core/src/stats/p2_quantile.rs`.
- `RollingWindow` already at `rushhft-core/src/pool/rolling_window.rs` — reuse for Decimal values; add a generic variant alongside if the existing API is too narrow.

Both studies write current values back to `SnapshotStore.studies` for the active symbol via the existing `PluginContext::register_metric` path.

## Section 4 — Charting (uPlot)

Library choice: uPlot (~40 KB min+gzip, canvas-based, designed for high-frequency time-series). Alternatives considered: Chart.js (10× bigger, slower), D3/ECharts (50× bigger, far more than needed). uPlot draws 100k points in <10ms.

Three charts in the ucOrderBook middle column:

| Chart | Series | Update trigger | Window |
|---|---|---|---|
| Cumulative Bids (left half, top row) | Step line, cumulative size vs price (best bid → deep) | depth push | snapshot at call time |
| Cumulative Asks (right half, top row) | Step line, cumulative size vs price (best ask → deep) | depth push | snapshot at call time |
| Real-time Price (middle row, full width) | Bid (green), Ask (red), Mid (gray) lines + trade dots colored by direction | depth push + trade push | last 60s rolling |
| Spread (bottom row, full width) | Spread (line) over time | depth push | last 60s rolling |

Backend support: `ChartSeriesBuffer` in `rushhft-app/src/state.rs` — per-symbol ring buffer of last N points (default 600 = 1min at 10Hz). `OrderBookHub` updates feed it on every depth push. `get_chart_series(symbol, kind, points)` reads from this buffer; `subscribe_chart_series` pushes via Tauri Channel.

uPlot setup (`ui/src/lib/charts/uPlotSetup.ts`):
- Theme: dark, grid `#30363d`, axes `#8b949e`, accent `#58a6ff`.
- Cursor: crosshair + snap-to-point.
- Scales: time x-axis (sec), linear y-axis with auto-fit.
- All charts `readOnly: true` (no zoom/pan for MVP).

Update strategy: each chart component holds its own uPlot instance in a Svelte action; on store update, call `chart.setData(newData)` (uPlot handles incremental redraw efficiently). For channel-pushed updates, batch ~5 per second to avoid thrash.

## Section 5 — Error handling, edge cases, testing

Error handling:
- IPC commands already return `Result<T, String>` — keep pattern. Surface errors via toast/notification in the modal that issued the call.
- LongPort subscribe failures (quota, invalid symbol) bubble up to UI as a notification via existing `NotificationHub`, not a modal blocking the whole app.
- Chart series buffer overruns: ring buffer drops oldest silently; no error path.
- Disconnected provider: snapshot's `provider_status = Disconnected` already — charts show stale data with gray "stale" badge + timestamp.

Edge cases:
- Empty depth (pre-market, symbol just subscribed): render ladder with placeholder rows + "awaiting data". No NaN propagation to charts (chart series buffer just stays empty until first push).
- Symbol switch: invalidate all per-symbol state — clear chart buffers, reset studies' rolling windows for that symbol, fetch fresh snapshot. ~100ms blank state acceptable.
- Multi-venue modal with single venue: show LongPort only, with a "no other venues configured" hint. No mock data.
- Runtime symbol add/remove: add to `user_symbols` list in `AppState` (separate from `Settings.default_symbols`). Persist this list to settings on save so it survives restart.
- Stale study values: studies mark `is_stale = true` if no input for > 5s (matches VisualHFT). UI shows gray text.
- Plugin start failure (e.g. credentials wrong): keep current `PluginStatus::StoppedFailed`; UI tile shows red.

Testing strategy:

| Layer | Tests |
|---|---|
| `rushhft-core` (P² quantile, rolling window, chart buffer) | unit tests — quantile convergence on uniform/normal sample, ring buffer drop-oldest, chart buffer cap |
| `rushhft-studies` (OTT, MR) | unit tests — OTT formula on synthetic depth/trade stream; MR recovery-time on canned shock/recovery scenario |
| `rushhft-connector-longport` (subscribe/unsubscribe) | integration test against the real LongPort endpoint with proxy env vars set |
| `rushhft-app` commands | extend `mod tests` in `commands.rs` with new commands (add_symbol, get_chart_series) using mock AppState |
| UI components | no test framework for MVP — manual smoke test per phase |
| End-to-end | `#[test]` that boots a minimal `AppState` with a mock connector producing canned depth, calls `get_snapshot` + `get_chart_series`, asserts non-empty |

No regressions: existing tests in `commands.rs` (snapshot_dto, save_settings, save_trigger) must keep passing. New IPC commands are additive; existing ones unchanged.

Performance budget: snapshot poll at 500ms × 416 symbols = bounded by single-symbol lookup (ArcSwap is O(1)). Chart buffer at 600 points × 4 charts × 1 symbol = trivial. Studies at S1 aggregation = 1 calc/sec. All well under the 16ms frame budget for 60fps UI.

## Out of scope (deferred to later sub-projects)

- Additional market connectors (Binance, Coinbase, etc.) — sub-project 9.
- `ExchangeExecutionSimulator` (paper trading) — sub-project 7.
- `AnalyticReports` / `demoTradingCore` — sub-project 8.
- Positions pane real data — needs broker plugin (sub-project 6).
- Per-study time-series chart (`ChartStudy` view) — sub-project 8.
- Plugin DLL discovery (RushHFT uses compile-time plugin list) — not needed for parity.
- log4net → tracing-subscriber migration — already done.
- Object pools for hot paths — RushHFT uses ArcSwap + lock-free patterns; equivalent performance without explicit pools.
