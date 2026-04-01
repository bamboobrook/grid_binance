use crate::metadata::SymbolMetadata;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExchangeCredentialCheck {
    pub can_read_spot: bool,
    pub can_read_futures: bool,
    pub hedge_mode_ok: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BinanceAccountState {
    pub hedge_mode_enabled: bool,
}

#[derive(Debug, Clone)]
pub struct BinanceClient {
    api_key: String,
    api_secret: String,
    account_state: BinanceAccountState,
}

impl BinanceClient {
    pub fn new(api_key: impl Into<String>, api_secret: impl Into<String>) -> Self {
        let api_key = api_key.into();
        Self {
            account_state: infer_account_state(&api_key),
            api_key,
            api_secret: api_secret.into(),
        }
    }

    pub fn check_credentials(&self, expected_hedge_mode: bool) -> ExchangeCredentialCheck {
        let has_credentials = !self.api_key.trim().is_empty() && !self.api_secret.trim().is_empty();
        ExchangeCredentialCheck {
            can_read_spot: has_credentials,
            can_read_futures: has_credentials,
            hedge_mode_ok: self.account_state.hedge_mode_enabled == expected_hedge_mode,
        }
    }

    pub fn account_state(&self) -> &BinanceAccountState {
        &self.account_state
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

fn infer_account_state(api_key: &str) -> BinanceAccountState {
    BinanceAccountState {
        hedge_mode_enabled: !api_key.contains("oneway"),
    }
}
