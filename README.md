# RushHFT

Real-time market microstructure analysis desktop app — Rust + Tauri 2 rewrite of VisualHFT (C# WPF).

Workspace of 4 crates:

| Crate | Role |
|---|---|
| `rushhft-core` | Domain model (order book, trades, quotes), plugin context, trigger engine, settings |
| `rushhft-connector-longport` | LongPort broker connector (WebSocket market data + REST) |
| `rushhft-studies` | Real-time studies: VPIN, LOB Imbalance |
| `rushhft-app` | Tauri 2 binary crate — desktop app + Svelte 5 frontend |

## Repository layout

```
Cargo.toml                       # workspace (resolver="3", edition 2024)
rust-toolchain.toml
rushhft-core/                    # shared domain + plugin SDK
rushhft-connector-longport/      # LongPort integration
rushhft-studies/                  # VPIN + LOB Imbalance
rushhft-app/                     # Tauri binary crate
├── Cargo.toml
├── tauri.conf.json              # Tauri config (flat layout, at crate root)
├── build.rs
├── icons/icon.png
├── src/                         # Rust: main.rs, commands, state, dto, context, notification
└── ui/                          # SvelteKit + Vite frontend
    ├── package.json
    ├── vite.config.ts
    ├── svelte.config.js
    └── src/routes/+page.svelte  # main dashboard (depth ladder / trades / studies)
docs/
└── superpowers/
    ├── specs/                   # design spec
    └── plans/                   # TDD implementation plans (core / connector / studies / app)
```

## Prerequisites

- **Rust toolchain** (stable, edition 2024) — `rustup show`
- **Node.js** + **pnpm** — for the Svelte frontend
- **Tauri CLI** (optional, only for `cargo tauri dev`/`cargo tauri build`):
  ```bash
  cargo install tauri-cli --version "^2" --locked
  ```
  If crates.io is slow from your network, set an HTTP proxy first:
  ```bash
  export http_proxy=http://127.0.0.1:10808 https_proxy=http://127.0.0.1:10808 \
         HTTP_PROXY=http://127.0.0.1:10808 HTTPS_PROXY=http://127.0.0.1:10808 \
         ALL_PROXY=socks5://127.0.0.1:10808
  ```
- **LongPort credentials** (only for live market data — app launches fine without them):
  - `app_key`, `app_secret`, `access_token` from the LongPort OpenAPI console

## Build

### 1. Build the frontend

```bash
cd rushhft-app/ui
pnpm install
pnpm build         # outputs static site to rushhft-app/ui/build/
```

### 2. Build the Rust workspace

From the workspace root:

```bash
# All crates
cargo build --release

# Or just the app (uses ../ui/build for frontendDist)
cargo build -p rushhft-app --release
```

Binary: `target/release/rushhft-app` (~24 MB, optimized).

## Run

### Option A — Release binary (simplest, no dev server needed)

```bash
./target/release/rushhft-app
```

The desktop window opens at 1400×900 with the prebuilt UI from `rushhft-app/ui/build/`.

### Option B — Dev mode (hot-reload UI via Vite)

```bash
cd rushhft-app
cargo tauri dev
```

Tauri CLI will run `pnpm dev` (`beforeDevCommand`) to start Vite on `http://localhost:5173`, then launch the debug binary pointed at that dev URL. Edit `ui/src/routes/+page.svelte` and the window hot-reloads.

### Option C — Bundle a macOS `.app`

```bash
cd rushhft-app
cargo tauri build
```

Output: `target/release/bundle/macos/RushHFT.app` (and a `.dmg`).

## Configuration

Settings are loaded from a TOML file under the platform config dir:

| OS | Path |
|---|---|
| macOS | `~/Library/Application Support/RushHFT/config.toml` |
| Linux | `~/.config/RushHFT/config.toml` |
| Windows | `%APPDATA%\RushHFT\config.toml` |

If the file is missing, `Settings::default()` is used (empty credentials, default symbol `700.HK`, depth 10).

### Minimal config with LongPort credentials

```toml
app_key = "aa1645c32ef6e1adf55eab4bf0d6498c"
app_secret = "04d17b87631693c42dea56f93d144c68ea55db70d9d47fc7ec0cc3a5094499c2"
access_token = "hk_m_eyJhbGciOiJSUzI1NiIs..."
default_symbols = ["700.HK"]
depth_levels = 10
aggregation_level = "S1"
log_level = "info"
```

Alternatively, call the `save_settings` IPC command from the frontend — it writes the same file via `Settings::save()`.

### Auto-start behavior

On launch, the setup hook checks whether `app_key`, `app_secret`, and `access_token` are all non-empty. If so, it auto-starts:

- **LongPortConnector** — subscribes to depth + trades for each `default_symbol`
- **VpinStudy** — consumes order book + trade events, emits VPIN metrics
- **LobImbalanceStudy** — consumes order book events, emits imbalance metrics

If credentials are missing, the window still opens — the depth ladder / trades / studies panels just show empty until you provide credentials and call `start_plugin` via the `save_settings` IPC.

## IPC commands

Registered in `rushhft-app/src/commands.rs`:

| Command | Purpose |
|---|---|
| `get_snapshot` | Current depth + trades + studies for a symbol |
| `get_providers` | Connector status list |
| `get_symbols` | Configured symbol list |
| `get_studies` | Study descriptors + status |
| `start_plugin` / `stop_plugin` | Control plugins by name |
| `get_settings` / `save_settings` | Read/write config TOML |
| `get_triggers` / `save_trigger` / `delete_trigger` | Manage trigger rules |
| `test_trigger_rest` | Dry-run a trigger against recent metrics |
| `subscribe_notifications` | Open a `Channel<NotificationPayload>` for trigger fires |

The frontend (`+page.svelte`) polls `get_snapshot` / `get_providers` / `get_studies` every 500 ms via `@tauri-apps/api/core` `invoke`.

## Tests

```bash
# Workspace tests
cargo test --workspace

# Clippy
cargo clippy --workspace --all-targets -- -D warnings

# Format check
cargo fmt --all -- --check

# Frontend type check
cd rushhft-app/ui && pnpm check
```

Each crate follows TDD — see the plan docs under `docs/superpowers/plans/`.

## Architecture notes

- **Lock-free snapshot reads**: `SnapshotStore` uses `DashMap<String, ArcSwap<SymbolSnapshot>>` — readers never block writers.
- **Plugin context**: `PluginContextImpl` wraps the order book hub, trade hub, and trigger engine. Plugins receive an `Arc<dyn PluginContext>` on `start()`.
- **Notification fan-out**: `NotificationHub` keeps a `Mutex<Vec<Channel<NotificationPayload>>>` — multiple frontends can subscribe at once.
- **Decimal precision**: `rust_decimal::Decimal` serialized as string (`serde-with-str` feature) to avoid float precision loss across IPC.
- **Time handling**: `OffsetDateTime::unix_timestamp_nanos()` returns `i128`; cast to i64 ms via `(value / 1_000_000) as i64`.

## License

Apache-2.0.
