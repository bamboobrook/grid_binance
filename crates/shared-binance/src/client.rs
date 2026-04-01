use crate::metadata::SymbolMetadata;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExchangeCredentialCheck {
    pub can_read_spot: bool,
    pub can_read_futures: bool,
    pub hedge_mode_ok: bool,
}

#[derive(Debug, Clone)]
pub struct BinanceClient {
    api_key: String,
    api_secret: String,
    hedge_mode_enabled: bool,
}

impl BinanceClient {
    pub fn new(
        api_key: impl Into<String>,
        api_secret: impl Into<String>,
        hedge_mode_enabled: bool,
    ) -> Self {
        Self {
            api_key: api_key.into(),
            api_secret: api_secret.into(),
            hedge_mode_enabled,
        }
    }

    pub fn check_credentials(&self) -> ExchangeCredentialCheck {
        let has_credentials = !self.api_key.trim().is_empty() && !self.api_secret.trim().is_empty();
        ExchangeCredentialCheck {
            can_read_spot: has_credentials,
            can_read_futures: has_credentials,
            hedge_mode_ok: self.hedge_mode_enabled,
        }
    }

    pub fn spot_symbols(&self) -> Vec<SymbolMetadata> {
        vec![
            SymbolMetadata::new("BTCUSDT", "spot", "TRADING"),
            SymbolMetadata::new("ETHUSDT", "spot", "TRADING"),
        ]
    }

    pub fn futures_symbols(&self) -> Vec<SymbolMetadata> {
        vec![
            SymbolMetadata::new("BTCUSDT", "futures", "TRADING"),
            SymbolMetadata::new("SOLUSDT", "futures", "TRADING"),
        ]
    }
}
