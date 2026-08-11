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
    /// LongPort SDK region override. Default `"global"` pins the international
    /// endpoint (`*.longportapp.com`) and bypasses the SDK's geo-probe. Set to
    /// `"cn"` to force the China mainland endpoint (`*.longport.cn`).
    pub region: String,
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
            region: "global".to_string(),
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
        let settings: Settings =
            toml::from_str(&content).map_err(|e| SettingsError::ParseFailed(e.to_string()))?;
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
            region: "global".into(),
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
