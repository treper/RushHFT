//! Tauri IPC commands. AppState is the managed Tauri state.
#![allow(dead_code)]

use crate::dto::{
    AggregationLevelDto, PluginStatusDto, PluginTypeDto, ProviderDto, SessionStatusDto,
    SettingsDto, SnapshotDto, StudyDescriptorDto,
};
use crate::state::{SnapshotStore, SymbolSnapshot};
use rushhft_core::Settings;
use rushhft_core::plugin::Plugin;
use rust_decimal::Decimal;
use std::sync::Arc;
use tokio::sync::RwLock;

pub struct AppState {
    pub snapshot_store: Arc<SnapshotStore>,
    pub plugins: Vec<Arc<dyn Plugin>>,
    pub settings: Arc<RwLock<Settings>>,
    pub plugin_context: Arc<dyn rushhft_core::plugin::PluginContext>,
    pub trigger_engine: Arc<rushhft_core::TriggerEngine>,
    pub notification_hub: Arc<crate::notification::NotificationHub>,
}

impl AppState {
    pub fn descriptor_for(&self, plugin: &Arc<dyn Plugin>) -> StudyDescriptorDto {
        StudyDescriptorDto {
            plugin_id: plugin.plugin_id().to_string(),
            name: plugin.name().to_string(),
            version: plugin.version().to_string(),
            description: plugin.description().to_string(),
            plugin_type: map_plugin_type(plugin.plugin_type()),
            status: map_plugin_status(plugin.status()),
            emits_metric: plugin.emits_metric(),
        }
    }

    pub fn snapshot_dto(&self, symbol: &str) -> SnapshotDto {
        match self.snapshot_store.snapshot(symbol) {
            Some(s) => snapshot_to_dto(s),
            None => SnapshotDto {
                symbol: symbol.to_string(),
                bids: vec![],
                asks: vec![],
                spread: Decimal::ZERO,
                mid_price: Decimal::ZERO,
                last_updated: 0,
                sequence: 0,
                provider_status: SessionStatusDto::Disconnected,
                studies: vec![],
                recent_trades: vec![],
                quote_stats: None,
            },
        }
    }

    pub fn providers_dto(&self) -> Vec<ProviderDto> {
        self.snapshot_store.providers()
    }

    pub fn symbols_dto(&self) -> Vec<String> {
        self.snapshot_store.symbols()
    }

    pub fn studies_dto(&self) -> Vec<StudyDescriptorDto> {
        self.plugins
            .iter()
            .map(|p| self.descriptor_for(p))
            .collect()
    }
}

fn map_plugin_type(t: rushhft_core::model::enums::PluginType) -> PluginTypeDto {
    use rushhft_core::model::enums::PluginType::*;
    match t {
        Unknown => PluginTypeDto::Unknown,
        Study => PluginTypeDto::Study,
        MultiStudy => PluginTypeDto::MultiStudy,
        MarketConnector => PluginTypeDto::MarketConnector,
    }
}

fn map_plugin_status(s: rushhft_core::model::enums::PluginStatus) -> PluginStatusDto {
    use rushhft_core::model::enums::PluginStatus::*;
    match s {
        Loaded => PluginStatusDto::Loaded,
        Starting => PluginStatusDto::Starting,
        Started => PluginStatusDto::Started,
        Stopping => PluginStatusDto::Stopping,
        Stopped => PluginStatusDto::Stopped,
        StoppedFailed => PluginStatusDto::StoppedFailed,
    }
}

fn snapshot_to_dto(snap: SymbolSnapshot) -> SnapshotDto {
    SnapshotDto {
        symbol: snap.symbol,
        bids: snap.bids,
        asks: snap.asks,
        spread: snap.spread,
        mid_price: snap.mid_price,
        last_updated: snap.last_updated,
        sequence: snap.sequence,
        provider_status: snap.provider_status,
        studies: snap.studies,
        recent_trades: snap.recent_trades,
        quote_stats: snap.quote_stats,
    }
}

#[tauri::command]
pub async fn get_snapshot(
    state: tauri::State<'_, AppState>,
    symbol: String,
) -> Result<SnapshotDto, String> {
    Ok(state.snapshot_dto(&symbol))
}

#[tauri::command]
pub async fn get_providers(state: tauri::State<'_, AppState>) -> Result<Vec<ProviderDto>, String> {
    Ok(state.providers_dto())
}

#[tauri::command]
pub async fn get_symbols(state: tauri::State<'_, AppState>) -> Result<Vec<String>, String> {
    Ok(state.symbols_dto())
}

#[tauri::command]
pub async fn get_studies(
    state: tauri::State<'_, AppState>,
) -> Result<Vec<StudyDescriptorDto>, String> {
    Ok(state.studies_dto())
}

pub async fn start_plugin_inner(state: &AppState, plugin_id: &str) -> Result<(), String> {
    let plugin = state
        .plugins
        .iter()
        .find(|p| p.plugin_id() == plugin_id)
        .ok_or_else(|| format!("plugin not found: {}", plugin_id))?
        .clone();
    plugin
        .start(state.plugin_context.clone())
        .await
        .map_err(|e| e.to_string())
}

pub async fn stop_plugin_inner(state: &AppState, plugin_id: &str) -> Result<(), String> {
    let plugin = state
        .plugins
        .iter()
        .find(|p| p.plugin_id() == plugin_id)
        .ok_or_else(|| format!("plugin not found: {}", plugin_id))?
        .clone();
    plugin.stop().await.map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn start_plugin(
    state: tauri::State<'_, AppState>,
    plugin_id: String,
) -> Result<(), String> {
    start_plugin_inner(&state, &plugin_id).await
}

#[tauri::command]
pub async fn stop_plugin(
    state: tauri::State<'_, AppState>,
    plugin_id: String,
) -> Result<(), String> {
    stop_plugin_inner(&state, &plugin_id).await
}

fn mask_secret(s: &str) -> String {
    if s.is_empty() {
        return String::new();
    }
    "••••••".to_string()
}

fn map_aggregation(a: rushhft_core::model::enums::AggregationLevel) -> AggregationLevelDto {
    use rushhft_core::model::enums::AggregationLevel::*;
    match a {
        None => AggregationLevelDto::None,
        Ms1 => AggregationLevelDto::Ms1,
        Ms10 => AggregationLevelDto::Ms10,
        Ms100 => AggregationLevelDto::Ms100,
        Ms500 => AggregationLevelDto::Ms500,
        S1 => AggregationLevelDto::S1,
        S3 => AggregationLevelDto::S3,
        S5 => AggregationLevelDto::S5,
        D1 => AggregationLevelDto::D1,
    }
}

fn aggregation_from_dto(a: AggregationLevelDto) -> rushhft_core::model::enums::AggregationLevel {
    use rushhft_core::model::enums::AggregationLevel::*;
    match a {
        AggregationLevelDto::None => None,
        AggregationLevelDto::Ms1 => Ms1,
        AggregationLevelDto::Ms10 => Ms10,
        AggregationLevelDto::Ms100 => Ms100,
        AggregationLevelDto::Ms500 => Ms500,
        AggregationLevelDto::S1 => S1,
        AggregationLevelDto::S3 => S3,
        AggregationLevelDto::S5 => S5,
        AggregationLevelDto::D1 => D1,
    }
}

pub async fn get_settings_inner(state: &AppState) -> SettingsDto {
    let s = state.settings.read().await;
    SettingsDto {
        app_key: s.app_key.clone(),
        app_secret_masked: mask_secret(&s.app_secret),
        access_token_masked: mask_secret(&s.access_token),
        default_symbols: s.default_symbols.clone(),
        depth_levels: s.depth_levels,
        aggregation_level: map_aggregation(s.aggregation_level),
        log_level: s.log_level.clone(),
    }
}

pub async fn save_settings_inner(state: &AppState, dto: SettingsDto) -> Result<(), String> {
    let mut s = state.settings.write().await;
    s.app_key = dto.app_key;
    // Masked fields in the DTO mean "unchanged" if the value is the mask; otherwise
    // the frontend sent the new value. We treat "••••••" as "keep existing".
    if dto.app_secret_masked != "••••••" {
        s.app_secret = dto.app_secret_masked;
    }
    if dto.access_token_masked != "••••••" {
        s.access_token = dto.access_token_masked;
    }
    s.default_symbols = dto.default_symbols;
    s.depth_levels = dto.depth_levels;
    s.aggregation_level = aggregation_from_dto(dto.aggregation_level);
    s.log_level = dto.log_level;
    Ok(())
}

#[tauri::command]
pub async fn get_settings(state: tauri::State<'_, AppState>) -> Result<SettingsDto, String> {
    Ok(get_settings_inner(&state).await)
}

#[tauri::command]
pub async fn save_settings(
    state: tauri::State<'_, AppState>,
    settings: SettingsDto,
) -> Result<(), String> {
    save_settings_inner(&state, settings).await?;
    let s = state.settings.read().await;
    s.save().map_err(|e| e.to_string())?;
    Ok(())
}

pub async fn get_triggers_inner(state: &AppState) -> Vec<rushhft_core::TriggerRule> {
    state.trigger_engine.get_rules().await
}

pub async fn save_trigger_inner(
    state: &AppState,
    rule: rushhft_core::TriggerRule,
) -> Result<(), String> {
    state.trigger_engine.add_or_update_rule(rule).await;
    Ok(())
}

pub async fn delete_trigger_inner(state: &AppState, rule_id: i64) -> Result<(), String> {
    state.trigger_engine.remove_rule(rule_id).await;
    Ok(())
}

pub async fn test_trigger_rest_inner(state: &AppState, rule_id: i64) -> Result<String, String> {
    let rules = state.trigger_engine.get_rules().await;
    let rule = rules
        .into_iter()
        .find(|r| r.rule_id == rule_id)
        .ok_or_else(|| format!("rule {} not found", rule_id))?;
    let action = rule
        .actions
        .first()
        .ok_or_else(|| format!("rule {} has no actions", rule_id))?;
    let rest = action
        .rest_api
        .as_ref()
        .ok_or_else(|| format!("rule {} action has no REST config", rule_id))?;
    // Fire a one-shot HTTP request — this is the manual "test" path.
    let client = reqwest::Client::new();
    let mut req = match rest.method.as_str() {
        "POST" => client.post(&rest.url),
        "PUT" => client.put(&rest.url),
        "GET" => client.get(&rest.url),
        _ => client.post(&rest.url),
    };
    for (k, v) in &rest.headers {
        req = req.header(k, v);
    }
    if !rest.body.is_empty() {
        req = req.body(rest.body.clone());
    }
    let resp = req.send().await.map_err(|e| e.to_string())?;
    Ok(format!("{} {}", resp.status().as_u16(), rest.url))
}

#[tauri::command]
pub async fn get_triggers(
    state: tauri::State<'_, AppState>,
) -> Result<Vec<rushhft_core::TriggerRule>, String> {
    Ok(get_triggers_inner(&state).await)
}

#[tauri::command]
pub async fn save_trigger(
    state: tauri::State<'_, AppState>,
    rule: rushhft_core::TriggerRule,
) -> Result<(), String> {
    save_trigger_inner(&state, rule).await
}

#[tauri::command]
pub async fn delete_trigger(state: tauri::State<'_, AppState>, rule_id: i64) -> Result<(), String> {
    delete_trigger_inner(&state, rule_id).await
}

#[tauri::command]
pub async fn test_trigger_rest(
    state: tauri::State<'_, AppState>,
    rule_id: i64,
) -> Result<String, String> {
    test_trigger_rest_inner(&state, rule_id).await
}

#[tauri::command]
pub async fn subscribe_notifications(
    state: tauri::State<'_, AppState>,
    channel: tauri::ipc::Channel<crate::dto::NotificationPayload>,
) -> Result<(), String> {
    state.notification_hub.register(channel).await;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

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
        }
    }

    #[tokio::test]
    async fn snapshot_dto_returns_empty_for_unknown_symbol() {
        let state = make_state(vec![]);
        let dto = state.snapshot_dto("NOPE.HK");
        assert_eq!(dto.symbol, "NOPE.HK");
        assert!(dto.bids.is_empty());
        assert_eq!(dto.provider_status, SessionStatusDto::Disconnected);
    }

    #[tokio::test]
    async fn providers_dto_empty_initially() {
        let state = make_state(vec![]);
        assert!(state.providers_dto().is_empty());
    }

    #[tokio::test]
    async fn symbols_dto_empty_initially() {
        let state = make_state(vec![]);
        assert!(state.symbols_dto().is_empty());
    }

    #[tokio::test]
    async fn studies_dto_lists_all_plugins() {
        use rushhft_studies::{VpinSettings, VpinStudy};
        let vpin = Arc::new(VpinStudy::new(VpinSettings::default()));
        let state = make_state(vec![vpin]);
        let studies = state.studies_dto();
        assert_eq!(studies.len(), 1);
        assert_eq!(studies[0].name, "VPIN Study");
        assert!(studies[0].emits_metric);
    }

    #[tokio::test]
    async fn start_plugin_by_id_invokes_start() {
        use rushhft_studies::{VpinSettings, VpinStudy};
        let vpin = Arc::new(VpinStudy::new(VpinSettings::default()));
        let id = vpin.plugin_id().to_string();
        let state = make_state(vec![vpin.clone()]);
        // Before: Loaded
        assert_eq!(state.studies_dto()[0].status, PluginStatusDto::Loaded);
        start_plugin_inner(&state, &id).await.unwrap();
        assert_eq!(state.studies_dto()[0].status, PluginStatusDto::Started);
        stop_plugin_inner(&state, &id).await.unwrap();
        assert_eq!(state.studies_dto()[0].status, PluginStatusDto::Stopped);
    }

    #[tokio::test]
    async fn start_plugin_unknown_id_returns_error() {
        let state = make_state(vec![]);
        let result = start_plugin_inner(&state, "nope").await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn get_settings_returns_masked_secrets() {
        let state = make_state(vec![]);
        {
            let mut s = state.settings.write().await;
            s.app_key = "real_key".into();
            s.app_secret = "real_secret_value".into();
            s.access_token = "real_token_value".into();
        }
        let dto = get_settings_inner(&state).await;
        assert_eq!(dto.app_key, "real_key");
        assert_eq!(dto.app_secret_masked, "••••••");
        assert_eq!(dto.access_token_masked, "••••••");
    }

    #[tokio::test]
    async fn save_settings_persists_to_memory() {
        let state = make_state(vec![]);
        let dto = crate::dto::SettingsDto {
            app_key: "new_key".into(),
            app_secret_masked: "new_secret".into(),
            access_token_masked: "new_token".into(),
            default_symbols: vec!["700.HK".into()],
            depth_levels: 10,
            aggregation_level: crate::dto::AggregationLevelDto::S1,
            log_level: "info".into(),
        };
        save_settings_inner(&state, dto.clone()).await.unwrap();
        let loaded = state.settings.read().await;
        assert_eq!(loaded.app_key, "new_key");
        assert_eq!(loaded.app_secret, "new_secret");
    }

    use rushhft_core::{
        ActionType, ConditionOperator, RestApiConfig, TimeWindow, TimeWindowUnit, TriggerAction,
        TriggerCondition, TriggerRule,
    };
    use rust_decimal_macros::dec;

    fn sample_rule(id: i64) -> TriggerRule {
        TriggerRule {
            rule_id: id,
            name: format!("rule-{}", id),
            is_enabled: true,
            conditions: vec![TriggerCondition {
                condition_id: 1,
                plugin: "VPIN Study".into(),
                metric: "VPIN".into(),
                exchange: "LongPort".into(),
                symbol: "700.HK".into(),
                operator: ConditionOperator::GreaterThan,
                threshold: dec!(0.5),
                window: Some(TimeWindow {
                    value: 1,
                    unit: TimeWindowUnit::Seconds,
                }),
            }],
            actions: vec![TriggerAction {
                action_type: ActionType::RestApi,
                cooldown_duration: 10,
                cooldown_unit: TimeWindowUnit::Seconds,
                rest_api: Some(RestApiConfig {
                    url: "https://example.com/hook".into(),
                    method: "POST".into(),
                    headers: std::collections::HashMap::new(),
                    body: "{}".into(),
                }),
            }],
        }
    }

    #[tokio::test]
    async fn save_trigger_persists_and_lists() {
        let state = make_state(vec![]);
        save_trigger_inner(&state, sample_rule(1)).await.unwrap();
        let rules = get_triggers_inner(&state).await;
        assert_eq!(rules.len(), 1);
        assert_eq!(rules[0].rule_id, 1);
    }

    #[tokio::test]
    async fn delete_trigger_removes_rule() {
        let state = make_state(vec![]);
        save_trigger_inner(&state, sample_rule(1)).await.unwrap();
        save_trigger_inner(&state, sample_rule(2)).await.unwrap();
        delete_trigger_inner(&state, 1).await.unwrap();
        let rules = get_triggers_inner(&state).await;
        assert_eq!(rules.len(), 1);
        assert_eq!(rules[0].rule_id, 2);
    }
}
