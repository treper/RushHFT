# RushHFT

VisualHFT（C# WPF 桌面端实时市场微观结构分析工具）的 Rust + Tauri 2 重写版本。

4 个 crate 组成的工作空间：

| Crate | 职责 |
|---|---|
| `rushhft-core` | 领域模型（订单簿、成交流、报价）、插件上下文、触发引擎、配置 |
| `rushhft-connector-longport` | LongPort 券商接入（WebSocket 行情 + REST 交易） |
| `rushhft-studies` | 实时研究指标：VPIN、LOB Imbalance |
| `rushhft-app` | Tauri 2 二进制 crate —— 桌面应用 + Svelte 5 前端 |

## 仓库结构

```
Cargo.toml                       # workspace（resolver="3"，edition 2024）
rust-toolchain.toml
rushhft-core/                    # 共享领域 + 插件 SDK
rushhft-connector-longport/       # LongPort 集成
rushhft-studies/                  # VPIN + LOB Imbalance
rushhft-app/                     # Tauri 二进制 crate
├── Cargo.toml
├── tauri.conf.json              # Tauri 配置（扁平结构，位于 crate 根目录）
├── build.rs
├── icons/icon.png
├── src/                         # Rust：main.rs、commands、state、dto、context、notification
└── ui/                          # SvelteKit + Vite 前端
    ├── package.json
    ├── vite.config.ts
    ├── svelte.config.js
    └── src/routes/+page.svelte  # 主仪表盘（深度梯 / 成交 / 研究指标）
docs/
└── superpowers/
    ├── specs/                   # 设计文档
    └── plans/                   # TDD 实现计划（core / connector / studies / app）
```

## 前置条件

- **Rust 工具链**（stable，edition 2024）—— `rustup show`
- **Node.js** + **pnpm** —— 用于 Svelte 前端
- **Tauri CLI**（可选，仅 `cargo tauri dev`/`cargo tauri build` 需要）：
  ```bash
  cargo install tauri-cli --version "^2" --locked
  ```
  如果网络访问 crates.io 太慢，先设置 HTTP 代理：
  ```bash
  export http_proxy=http://127.0.0.1:10808 https_proxy=http://127.0.0.1:10808 \
         HTTP_PROXY=http://127.0.0.1:10808 HTTPS_PROXY=http://127.0.0.1:10808 \
         ALL_PROXY=socks5://127.0.0.1:10808
  ```
- **LongPort 凭证**（仅获取实盘行情需要，没有也能启动）：
  - `app_key`、`app_secret`、`access_token`，从 LongPort OpenAPI 控制台获取

## 构建

### 1. 构建前端

```bash
cd rushhft-app/ui
pnpm install
pnpm build         # 输出静态站点到 rushhft-app/ui/build/
```

### 2. 构建 Rust workspace

在 workspace 根目录执行：

```bash
# 全部 crate
cargo build --release

# 或只构建 app（使用 ../ui/build 作为 frontendDist）
cargo build -p rushhft-app --release
```

产物：`target/release/rushhft-app`（约 24 MB，已优化）。

## 运行

### 方式 A —— Release 二进制（最简单，不需要 dev server）

```bash
./target/release/rushhft-app
```

桌面窗口以 1400×900 打开，前端来自 `rushhft-app/ui/build/`。

### 方式 B —— Dev 模式（Vite 热重载）

```bash
cd rushhft-app
cargo tauri dev
```

Tauri CLI 会执行 `pnpm dev`（`beforeDevCommand`）启动 Vite 到 `http://localhost:5173`，然后启动指向该 dev URL 的 debug 二进制。修改 `ui/src/routes/+page.svelte` 后窗口会热重载。

### 方式 C —— 打包 macOS `.app`

```bash
cd rushhft-app
cargo tauri build
```

产物：`target/release/bundle/macos/RushHFT.app`（以及 `.dmg`）。

## 配置

配置从平台配置目录下的 TOML 文件加载：

| 系统 | 路径 |
|---|---|
| macOS | `~/Library/Application Support/RushHFT/config.toml` |
| Linux | `~/.config/RushHft/config.toml` |
| Windows | `%APPDATA%\RushHFT\config.toml` |

如果文件不存在，使用 `Settings::default()`（凭证为空，默认标的 `700.HK`，深度 10 档）。

### 包含 LongPort 凭证的最小配置

```toml
app_key = "aa1645c32ef6e1adf55eab4bf0d6498c"
app_secret = "04d17b87631693c42dea56f93d144c68ea55db70d9d47fc7ec0cc3a5094499c2"
access_token = "hk_m_eyJhbGciOiJSUzI1NiIs..."
default_symbols = ["700.HK"]
depth_levels = 10
aggregation_level = "S1"
log_level = "info"
```

也可以从前端调用 `save_settings` IPC 命令 —— 它会通过 `Settings::save()` 写同一个文件。

### 自动启动行为

启动时，setup hook 会检查 `app_key`、`app_secret`、`access_token` 是否都非空。如果都有，会自动启动：

- **LongPortConnector** —— 订阅每个 `default_symbol` 的深度 + 成交
- **VpinStudy** —— 消费订单簿 + 成交事件，输出 VPIN 指标
- **LobImbalanceStudy** —— 消费订单簿事件，输出 imbalance 指标

如果凭证缺失，窗口依然会打开 —— 只是深度梯 / 成交 / 研究指标面板会是空的，直到你提供凭证并通过 `save_settings` IPC 调用 `start_plugin`。

## IPC 命令

在 `rushhft-app/src/commands.rs` 中注册：

| 命令 | 用途 |
|---|---|
| `get_snapshot` | 某个标的的当前深度 + 成交 + 研究指标 |
| `get_providers` | Connector 状态列表 |
| `get_symbols` | 配置的标的列表 |
| `get_studies` | 研究指标描述符 + 状态 |
| `start_plugin` / `stop_plugin` | 按名字控制插件 |
| `get_settings` / `save_settings` | 读写配置 TOML |
| `get_triggers` / `save_trigger` / `delete_trigger` | 管理触发规则 |
| `test_trigger_rest` | 针对最近指标试跑一个触发器 |
| `subscribe_notifications` | 打开 `Channel<NotificationPayload>` 接收触发通知 |

前端（`+page.svelte`）每 500ms 通过 `@tauri-apps/api/core` 的 `invoke` 轮询 `get_snapshot` / `get_providers` / `get_studies`。

## 测试

```bash
# Workspace 全部测试
cargo test --workspace

# Clippy
cargo clippy --workspace --all-targets -- -D warnings

# 格式检查
cargo fmt --all -- --check

# 前端类型检查
cd rushhft-app/ui && pnpm check
```

每个 crate 遵循 TDD，详见 `docs/superpowers/plans/` 下的计划文档。

## 架构要点

- **无锁快照读取**：`SnapshotStore` 用 `DashMap<String, ArcSwap<SymbolSnapshot>>` —— 读不会阻塞写。
- **插件上下文**：`PluginContextImpl` 封装订单簿 hub、成交 hub 和触发引擎。插件在 `start()` 时收到一个 `Arc<dyn PluginContext>`。
- **通知扇出**：`NotificationHub` 用 `Mutex<Vec<Channel<NotificationPayload>>>` —— 多个前端可同时订阅。
- **Decimal 精度**：`rust_decimal::Decimal` 用字符串序列化（`serde-with-str` feature），避免 IPC 传输时浮点精度丢失。
- **时间处理**：`OffsetDateTime::unix_timestamp_nanos()` 返回 `i128`，需要 `(value / 1_000_000) as i64` 转成 i64 毫秒。

## License

Apache-2.0.
