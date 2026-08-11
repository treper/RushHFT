mod commands;
mod context;
mod dto;
mod notification;
mod state;
mod ui_state;

use commands::AppState;
use context::PluginContextImpl;
use rushhft_connector_longport::{ConnectorSettings, LongPortConnector};
use rushhft_core::plugin::Plugin;
use rushhft_core::{OrderBookHub, ProviderHub, Settings, TradeHub, TriggerEngine};
use rushhft_studies::{
    LobImbalanceSettings, LobImbalanceStudy, MarketResilienceSettings, MarketResilienceStudy,
    OttRatioSettings, OttRatioStudy, VpinSettings, VpinStudy,
};
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing_subscriber::EnvFilter;

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::from_default_env())
        .init();

    let loaded = Settings::load().unwrap_or_default();
    // The LongPort Rust SDK geo-probes `https://geotest.lbkrs.com` and, on a
    // 200 response, switches to the `*.longport.cn` endpoints. Those are
    // unreachable from networks that *can* reach the probe host but *cannot*
    // reach `openapi.longport.cn` (e.g. this machine). Pin the region from
    // Settings so the SDK skips the probe and uses the configured endpoint.
    // `LONGPORT_REGION` is read by the SDK at the first HTTP/WS request, so
    // setting it here — before any connector code runs — is sufficient.
    if std::env::var("LONGPORT_REGION").is_err() {
        // SAFETY: this runs once at process startup before any other thread
        // could be reading LONGPORT_REGION (no connector or worker task has
        // been spawned yet). The LongPort SDK reads this env var lazily on
        // the first HTTP/WS request.
        unsafe { std::env::set_var("LONGPORT_REGION", &loaded.region) };
    }

    // On macOS the user may have a system-level HTTP/HTTPS/SOCKS proxy
    // configured in System Preferences but no HTTP_PROXY/HTTPS_PROXY env
    // vars exported in the shell. reqwest (and the LongPort SDK's HTTP
    // client) only read env vars, not the macOS system proxy. Read the
    // system proxy via `scutil --proxy` and inject the env vars so the
    // HTTP calls (get_otp, REST quotes) and the wsclient's new CONNECT
    // tunnel path both route through the proxy.
    //
    // Only runs on macOS; on other platforms this is a no-op.
    #[cfg(target_os = "macos")]
    {
        if let Err(e) = inject_macos_system_proxy() {
            tracing::warn!(error = %e, "failed to read macOS system proxy");
        }
    }

    let settings = Arc::new(RwLock::new(loaded));

    let ob_hub = Arc::new(OrderBookHub::new());
    let t_hub = Arc::new(TradeHub::new());
    let p_hub = Arc::new(ProviderHub::new());
    let snapshot_store = Arc::new(state::SnapshotStore::new());
    let trigger_engine = Arc::new(TriggerEngine::new());
    let notification_hub = Arc::new(notification::NotificationHub::new());

    // Forward metric events from PluginContextImpl into TriggerEngine.
    let (metric_tx, mut metric_rx) =
        tokio::sync::mpsc::unbounded_channel::<rushhft_core::MetricEvent>();
    {
        let te = trigger_engine.clone();
        tokio::spawn(async move {
            while let Some(event) = metric_rx.recv().await {
                te.register_metric(event);
            }
        });
    }

    let plugin_context: Arc<dyn rushhft_core::plugin::PluginContext> =
        Arc::new(PluginContextImpl::new(
            ob_hub.clone(),
            t_hub.clone(),
            p_hub.clone(),
            snapshot_store.clone(),
            metric_tx,
        ));

    let settings_snapshot = settings.read().await.clone();
    let first_symbol = settings_snapshot
        .default_symbols
        .first()
        .cloned()
        .unwrap_or_else(|| "700.HK".to_string());

    let connector = Arc::new(LongPortConnector::new(ConnectorSettings::from_settings(
        &settings_snapshot,
    )));

    let vpin = Arc::new(VpinStudy::new(VpinSettings {
        bucket_volume_size: rust_decimal::Decimal::ONE,
        number_of_buckets: 50,
        symbol: first_symbol.clone(),
        provider_id: 1,
        aggregation_level: settings_snapshot.aggregation_level,
    })) as Arc<dyn Plugin>;

    let lob = Arc::new(LobImbalanceStudy::new(LobImbalanceSettings {
        symbol: first_symbol.clone(),
        provider_id: 1,
        levels: 5,
        aggregation_level: settings_snapshot.aggregation_level,
    })) as Arc<dyn Plugin>;

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

    let plugins: Vec<Arc<dyn Plugin>> = vec![
        connector.clone() as Arc<dyn Plugin>,
        vpin.clone(),
        lob.clone(),
        mr,
        ott,
    ];

    let app_state = AppState {
        snapshot_store,
        plugins: plugins.clone(),
        settings: settings.clone(),
        plugin_context: plugin_context.clone(),
        trigger_engine: trigger_engine.clone(),
        notification_hub: notification_hub.clone(),
        user_symbols: Arc::new(ui_state::UserSymbols::new()),
        connector: Some(connector.clone()),
    };

    // Spawn the TriggerEngine consumer.
    let te = trigger_engine.clone();
    tokio::spawn(async move { te.start().await });

    let plugins_for_setup = plugins.clone();
    let ctx_for_setup = plugin_context.clone();
    let settings_for_setup = settings.clone();
    tauri::Builder::default()
        .plugin(tauri_plugin_shell::init())
        .manage(app_state)
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
            commands::add_symbol,
            commands::remove_symbol,
        ])
        .setup(move |_app| {
            let plugins_inner = plugins_for_setup.clone();
            let ctx_inner = ctx_for_setup.clone();
            let settings_inner = settings_for_setup.clone();
            tokio::spawn(async move {
                let s = settings_inner.read().await;
                let has_credentials =
                    !s.app_key.is_empty() && !s.app_secret.is_empty() && !s.access_token.is_empty();
                drop(s);
                if has_credentials {
                    for p in &plugins_inner {
                        let _ = p.start(ctx_inner.clone()).await;
                    }
                }
            });
            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("error while running RushHFT");
}

/// Parse `scutil --proxy` output and inject `HTTP_PROXY`/`HTTPS_PROXY` env
/// vars from the macOS system proxy configuration, but only if the user has
/// not already exported them. Returns Ok(()) if scutil could be run (even if
/// no proxy is configured); Err only on spawn/read failure.
///
/// SAFETY: env::set_var is called before any connector or worker task is
/// spawned, so no other thread is reading these vars concurrently.
#[cfg(target_os = "macos")]
fn inject_macos_system_proxy() -> Result<(), Box<dyn std::error::Error>> {
    let output = std::process::Command::new("scutil")
        .arg("--proxy")
        .output()?;
    if !output.status.success() {
        return Err(format!("scutil --proxy exited {}", output.status).into());
    }
    let stdout = String::from_utf8_lossy(&output.stdout);
    let http_proxy = extract_scutil_field(&stdout, "HTTPEnable", "HTTPProxy", "HTTPPort");
    let https_proxy = extract_scutil_field(&stdout, "HTTPSEnable", "HTTPSProxy", "HTTPSPort");

    // SAFETY: runs once at process startup before any other thread could be
    // reading these env vars. The LongPort SDK's HTTP client (reqwest with
    // system-proxy feature) reads these lazily on the first HTTP request,
    // and the wsclient reads HTTPS_PROXY inside do_connect per connection.
    unsafe {
        if std::env::var("HTTP_PROXY").is_err() {
            if let Some(p) = http_proxy {
                std::env::set_var("HTTP_PROXY", p);
            }
        }
        if std::env::var("HTTPS_PROXY").is_err() {
            if let Some(p) = https_proxy {
                std::env::set_var("HTTPS_PROXY", p);
            }
        }
    }
    Ok(())
}

/// Parse scutil output: returns `http://<host>:<port>` if `<enable_key> : 1`
/// and both `<host_key>` and `<port_key>` are present.
#[cfg(target_os = "macos")]
fn extract_scutil_field(output: &str, enable_key: &str, host_key: &str, port_key: &str) -> Option<String> {
    let enabled = output
        .lines()
        .find_map(|l| {
            let l = l.trim();
            let mut parts = l.split(':');
            let k = parts.next()?.trim();
            let v = parts.next()?.trim();
            if k == enable_key {
                Some(v == "1")
            } else {
                None
            }
        })?;
    if !enabled {
        return None;
    }
    let host = output.lines().find_map(|l| {
        let l = l.trim();
        let mut parts = l.split(':');
        let k = parts.next()?.trim();
        let v = parts.next()?.trim();
        if k == host_key { Some(v.to_string()) } else { None }
    })?;
    let port = output.lines().find_map(|l| {
        let l = l.trim();
        let mut parts = l.split(':');
        let k = parts.next()?.trim();
        let v = parts.next()?.trim();
        if k == port_key { Some(v.to_string()) } else { None }
    })?;
    Some(format!("http://{}:{}", host, port))
}
