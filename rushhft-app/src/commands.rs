//! Tauri IPC commands. AppState is the managed Tauri state.
#![allow(dead_code)]

use crate::dto::{
    PluginStatusDto, PluginTypeDto, ProviderDto, SessionStatusDto, SnapshotDto,
    StudyDescriptorDto,
};
use crate::state::{SnapshotStore, SymbolSnapshot};
use rushhft_core::plugin::Plugin;
use rushhft_core::Settings;
use rust_decimal::Decimal;
use std::sync::Arc;
use tokio::sync::RwLock;

pub struct AppState {
    pub snapshot_store: Arc<SnapshotStore>,
    pub plugins: Vec<Arc<dyn Plugin>>,
    pub settings: Arc<RwLock<Settings>>,
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
        self.plugins.iter().map(|p| self.descriptor_for(p)).collect()
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

#[cfg(test)]
mod tests {
    use super::*;

    fn make_state(plugins: Vec<Arc<dyn Plugin>>) -> AppState {
        AppState {
            snapshot_store: Arc::new(SnapshotStore::new()),
            plugins,
            settings: Arc::new(RwLock::new(Settings::default())),
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
}
