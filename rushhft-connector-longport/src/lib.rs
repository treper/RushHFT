//! LongPort connector for RushHFT.
//!
//! Thin wrapper around the `longport` SDK crate that implements
//! `rushhft_core::Plugin` and maps `PushEvent` pushes to normalized
//! `rushhft_core` domain models.

#[derive(Debug, Clone)]
pub struct ConnectorSettings {
    pub app_key: String,
    pub app_secret: String,
    pub access_token: String,
    pub symbols: Vec<String>,
    pub depth_levels: usize,
    pub price_decimal_places: u8,
    pub size_decimal_places: u8,
    pub provider_id: i32,
    pub sub_flags: longport::quote::SubFlags,
}

impl Default for ConnectorSettings {
    fn default() -> Self {
        Self {
            app_key: String::new(),
            app_secret: String::new(),
            access_token: String::new(),
            symbols: vec!["700.HK".to_string()],
            depth_levels: 10,
            price_decimal_places: 2,
            size_decimal_places: 0,
            provider_id: 1,
            sub_flags: longport::quote::SubFlags::DEPTH
                | longport::quote::SubFlags::BROKER
                | longport::quote::SubFlags::TRADE
                | longport::quote::SubFlags::QUOTE,
        }
    }
}

impl ConnectorSettings {
    pub fn from_settings(s: &rushhft_core::Settings) -> Self {
        Self {
            app_key: s.app_key.clone(),
            app_secret: s.app_secret.clone(),
            access_token: s.access_token.clone(),
            symbols: s.default_symbols.clone(),
            depth_levels: s.depth_levels,
            price_decimal_places: 2,
            size_decimal_places: 0,
            provider_id: 1,
            sub_flags: longport::quote::SubFlags::DEPTH
                | longport::quote::SubFlags::BROKER
                | longport::quote::SubFlags::TRADE
                | longport::quote::SubFlags::QUOTE,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_settings_have_longport_sub_flags() {
        let s = ConnectorSettings::default();
        assert!(s.sub_flags.contains(longport::quote::SubFlags::DEPTH));
        assert!(s.sub_flags.contains(longport::quote::SubFlags::BROKER));
        assert!(s.sub_flags.contains(longport::quote::SubFlags::TRADE));
        assert!(s.sub_flags.contains(longport::quote::SubFlags::QUOTE));
        assert_eq!(s.depth_levels, 10);
        assert_eq!(s.provider_id, 1);
    }

    #[test]
    fn from_settings_maps_core_fields() {
        let mut core = rushhft_core::Settings::default();
        core.app_key = "key".into();
        core.app_secret = "secret".into();
        core.access_token = "tok".into();
        core.default_symbols = vec!["700.HK".into(), "AAPL.US".into()];
        core.depth_levels = 20;
        let cs = ConnectorSettings::from_settings(&core);
        assert_eq!(cs.app_key, "key");
        assert_eq!(cs.access_token, "tok");
        assert_eq!(cs.symbols, vec!["700.HK", "AAPL.US"]);
        assert_eq!(cs.depth_levels, 20);
    }
}
