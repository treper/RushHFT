mod commands;
mod context;
mod dto;
mod notification;
mod state;

use commands::AppState;
use context::PluginContextImpl;
use rushhft_connector_longport::{ConnectorSettings, LongPortConnector};
use rushhft_core::plugin::Plugin;
use rushhft_core::{OrderBookHub, ProviderHub, Settings, TradeHub, TriggerEngine};
use rushhft_studies::{LobImbalanceSettings, LobImbalanceStudy, VpinSettings, VpinStudy};
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing_subscriber::EnvFilter;

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::from_default_env())
        .init();

    let loaded = Settings::load().unwrap_or_default();
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
    ))) as Arc<dyn Plugin>;

    let vpin = Arc::new(VpinStudy::new(VpinSettings {
        bucket_volume_size: rust_decimal::Decimal::ONE,
        number_of_buckets: 50,
        symbol: first_symbol.clone(),
        provider_id: 1,
        aggregation_level: settings_snapshot.aggregation_level,
    })) as Arc<dyn Plugin>;

    let lob = Arc::new(LobImbalanceStudy::new(LobImbalanceSettings {
        symbol: first_symbol,
        provider_id: 1,
        levels: 5,
        aggregation_level: settings_snapshot.aggregation_level,
    })) as Arc<dyn Plugin>;

    let plugins: Vec<Arc<dyn Plugin>> = vec![connector.clone(), vpin.clone(), lob.clone()];

    let app_state = AppState {
        snapshot_store,
        plugins: plugins.clone(),
        settings: settings.clone(),
        plugin_context: plugin_context.clone(),
        trigger_engine: trigger_engine.clone(),
        notification_hub: notification_hub.clone(),
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
            commands::start_plugin,
            commands::stop_plugin,
            commands::get_settings,
            commands::save_settings,
            commands::get_triggers,
            commands::save_trigger,
            commands::delete_trigger,
            commands::test_trigger_rest,
            commands::subscribe_notifications,
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
