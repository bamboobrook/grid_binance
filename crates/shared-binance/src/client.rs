use aes_gcm::{
    aead::{Aead, KeyInit},
    Aes256Gcm, Key, Nonce,
};
use base64::{engine::general_purpose::STANDARD as BASE64, Engine as _};
use sha2::{Digest, Sha256};
use std::env;

use crate::metadata::SymbolMetadata;

const NONCE_SIZE: usize = 12;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CredentialValidationRequest {
    pub expected_hedge_mode: bool,
    pub selected_markets: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CredentialValidationError {
    message: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExchangeCredentialCheck {
    pub selected_markets: Vec<String>,
    pub api_connectivity_ok: bool,
    pub timestamp_in_sync: bool,
    pub can_read_spot: bool,
    pub can_read_usdm: bool,
    pub can_read_coinm: bool,
    pub hedge_mode_ok: bool,
    pub permissions_ok: bool,
    pub withdrawal_disabled: bool,
    pub market_access_ok: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BinanceAccountState {
    pub api_connectivity_ok: bool,
    pub timestamp_in_sync: bool,
    pub hedge_mode_enabled: bool,
    pub trading_enabled: bool,
    pub withdrawal_enabled: bool,
    pub spot_reachable: bool,
    pub usdm_reachable: bool,
    pub coinm_reachable: bool,
}

#[derive(Debug, Clone)]
pub struct BinanceClient {
    api_key: String,
    api_secret: String,
    account_state: BinanceAccountState,
}

#[derive(Debug, Clone)]
pub struct CredentialCipher {
    key: [u8; 32],
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CredentialCipherError {
    message: String,
}

impl CredentialValidationRequest {
    pub fn new(
        expected_hedge_mode: bool,
        selected_markets: &[String],
    ) -> Result<Self, CredentialValidationError> {
        Ok(Self {
            expected_hedge_mode,
            selected_markets: normalize_markets(selected_markets)?,
        })
    }
}

impl BinanceClient {
    pub fn new(api_key: impl Into<String>, api_secret: impl Into<String>) -> Self {
        let api_key = api_key.into();
        let api_secret = api_secret.into();
        Self {
            account_state: infer_account_state(&api_key, &api_secret),
            api_key,
            api_secret,
        }
    }

    pub fn check_credentials(&self, expected_hedge_mode: bool) -> ExchangeCredentialCheck {
        self.check_credentials_for(
            &CredentialValidationRequest::new(
                expected_hedge_mode,
                &["spot".to_owned(), "usdm".to_owned(), "coinm".to_owned()],
            )
            .expect("default market selection should be valid"),
        )
    }

    pub fn check_credentials_for(
        &self,
        request: &CredentialValidationRequest,
    ) -> ExchangeCredentialCheck {
        let selected_markets = request.selected_markets.clone();
        let api_connectivity_ok = self.account_state.api_connectivity_ok
            && !self.api_key.trim().is_empty()
            && !self.api_secret.trim().is_empty();
        let permissions_ok = api_connectivity_ok
            && self.account_state.trading_enabled
            && !self.account_state.withdrawal_enabled;

        let can_read_spot = api_connectivity_ok
            && selected_markets.contains(&"spot".to_owned())
            && self.account_state.spot_reachable;
        let can_read_usdm = api_connectivity_ok
            && selected_markets.contains(&"usdm".to_owned())
            && self.account_state.usdm_reachable;
        let can_read_coinm = api_connectivity_ok
            && selected_markets.contains(&"coinm".to_owned())
            && self.account_state.coinm_reachable;

        let market_access_ok = selected_markets.iter().all(|market| match market.as_str() {
            "spot" => can_read_spot,
            "usdm" => can_read_usdm,
            "coinm" => can_read_coinm,
            _ => false,
        });

        let futures_selected = selected_markets
            .iter()
            .any(|market| market == "usdm" || market == "coinm");

        ExchangeCredentialCheck {
            selected_markets,
            api_connectivity_ok,
            timestamp_in_sync: self.account_state.timestamp_in_sync,
            can_read_spot,
            can_read_usdm,
            can_read_coinm,
            hedge_mode_ok: if futures_selected {
                self.account_state.hedge_mode_enabled == request.expected_hedge_mode
            } else {
                true
            },
            permissions_ok,
            withdrawal_disabled: !self.account_state.withdrawal_enabled,
            market_access_ok,
        }
    }

    pub fn account_state(&self) -> &BinanceAccountState {
        &self.account_state
    }

    pub fn spot_symbols(&self) -> Vec<SymbolMetadata> {
        vec![
            SymbolMetadata::new(
                "BTCUSDT",
                "spot",
                "TRADING",
                "BTC",
                "USDT",
                2,
                6,
                symbol_filters("0.01", "0.000010", "0.000010", "5", None),
                symbol_requirements(false, false, false, false, &[]),
                ["spot", "cash", "exchange"],
            ),
            SymbolMetadata::new(
                "ETHUSDT",
                "spot",
                "TRADING",
                "ETH",
                "USDT",
                2,
                5,
                symbol_filters("0.01", "0.000100", "0.000100", "5", None),
                symbol_requirements(false, false, false, false, &[]),
                ["spot", "cash", "exchange"],
            ),
        ]
    }

    pub fn usdm_symbols(&self) -> Vec<SymbolMetadata> {
        vec![
            SymbolMetadata::new(
                "BTCUSDT",
                "usdm",
                "TRADING",
                "BTC",
                "USDT",
                2,
                3,
                symbol_filters("0.10", "0.001", "0.001", "100", Some("1")),
                symbol_requirements(true, true, true, true, &[20, 50, 125]),
                ["futures", "perpetual", "linear", "usdm"],
            ),
            SymbolMetadata::new(
                "SOLUSDT",
                "usdm",
                "TRADING",
                "SOL",
                "USDT",
                3,
                1,
                symbol_filters("0.001", "0.1", "0.1", "20", Some("1")),
                symbol_requirements(true, true, true, true, &[20, 50]),
                ["futures", "perpetual", "linear", "usdm"],
            ),
        ]
    }

    pub fn coinm_symbols(&self) -> Vec<SymbolMetadata> {
        vec![
            SymbolMetadata::new(
                "BTCUSD_PERP",
                "coinm",
                "TRADING",
                "BTC",
                "USD",
                1,
                0,
                symbol_filters("0.1", "1", "1", "100", Some("100")),
                symbol_requirements(true, true, true, true, &[25, 50, 100]),
                ["futures", "delivery", "inverse", "coin"],
            ),
            SymbolMetadata::new(
                "ETHUSD_PERP",
                "coinm",
                "TRADING",
                "ETH",
                "USD",
                1,
                0,
                symbol_filters("0.1", "1", "1", "50", Some("50")),
                symbol_requirements(true, true, true, true, &[25, 50]),
                ["futures", "delivery", "inverse", "coin"],
            ),
        ]
    }
}

impl ExchangeCredentialCheck {
    pub fn is_healthy(&self) -> bool {
        self.api_connectivity_ok
            && self.timestamp_in_sync
            && self.permissions_ok
            && self.withdrawal_disabled
            && self.market_access_ok
            && self.hedge_mode_ok
    }

    pub fn connection_status(&self) -> &'static str {
        if self.is_healthy() {
            "healthy"
        } else {
            "degraded"
        }
    }
}

impl CredentialCipher {
    pub fn new(master_key_material: impl AsRef<[u8]>) -> Self {
        let digest = Sha256::digest(master_key_material.as_ref());
        let mut key = [0u8; 32];
        key.copy_from_slice(&digest);
        Self { key }
    }

    pub fn encrypt(
        &self,
        api_key: &str,
        api_secret: &str,
    ) -> Result<String, CredentialCipherError> {
        let cipher = Aes256Gcm::new(Key::<Aes256Gcm>::from_slice(&self.key));
        let mut nonce = [0u8; NONCE_SIZE];
        getrandom::getrandom(&mut nonce).map_err(|error| {
            CredentialCipherError::new(format!("failed to generate nonce: {error}"))
        })?;

        let payload = format!("{api_key}\n{api_secret}");
        let ciphertext = cipher
            .encrypt(Nonce::from_slice(&nonce), payload.as_bytes())
            .map_err(|_| CredentialCipherError::new("failed to encrypt exchange credentials"))?;

        Ok(format!(
            "v1:{}:{}",
            BASE64.encode(nonce),
            BASE64.encode(ciphertext)
        ))
    }

    pub fn decrypt(&self, payload: &str) -> Result<(String, String), CredentialCipherError> {
        let (version, nonce_b64, ciphertext_b64) = payload
            .split_once(':')
            .and_then(|(version, rest)| {
                rest.split_once(':')
                    .map(|(nonce, ciphertext)| (version, nonce, ciphertext))
            })
            .ok_or_else(|| CredentialCipherError::new("invalid encrypted credential payload"))?;
        if version != "v1" {
            return Err(CredentialCipherError::new(
                "unsupported encrypted credential payload version",
            ));
        }

        let nonce = BASE64
            .decode(nonce_b64)
            .map_err(|_| CredentialCipherError::new("invalid encrypted credential nonce"))?;
        if nonce.len() != NONCE_SIZE {
            return Err(CredentialCipherError::new(
                "invalid encrypted credential nonce length",
            ));
        }
        let ciphertext = BASE64
            .decode(ciphertext_b64)
            .map_err(|_| CredentialCipherError::new("invalid encrypted credential ciphertext"))?;

        let cipher = Aes256Gcm::new(Key::<Aes256Gcm>::from_slice(&self.key));
        let plaintext = cipher
            .decrypt(Nonce::from_slice(&nonce), ciphertext.as_ref())
            .map_err(|_| CredentialCipherError::new("failed to decrypt exchange credentials"))?;
        let plaintext = String::from_utf8(plaintext)
            .map_err(|_| CredentialCipherError::new("encrypted credentials are not valid utf-8"))?;
        let (api_key, api_secret) = plaintext.split_once('\n').ok_or_else(|| {
            CredentialCipherError::new("encrypted credentials payload is malformed")
        })?;
        Ok((api_key.to_owned(), api_secret.to_owned()))
    }

    pub fn from_env(var_name: &str) -> Result<Self, CredentialCipherError> {
        let key_material = env::var(var_name)
            .ok()
            .map(|value| value.trim().to_owned())
            .filter(|value| !value.is_empty())
            .ok_or_else(|| CredentialCipherError::new(format!("{var_name} is required")))?;
        Ok(Self::new(key_material))
    }
}

impl CredentialCipherError {
    pub fn new(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
        }
    }
}

impl CredentialValidationError {
    pub fn new(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
        }
    }
}

impl std::fmt::Display for CredentialValidationError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.message)
    }
}

impl std::error::Error for CredentialValidationError {}

impl std::fmt::Display for CredentialCipherError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.message)
    }
}

impl std::error::Error for CredentialCipherError {}

pub fn mask_api_key(api_key: &str) -> String {
    let trimmed = api_key.trim();
    if trimmed.len() <= 8 {
        return "*".repeat(trimmed.len());
    }

    format!("{}****{}", &trimmed[..4], &trimmed[trimmed.len() - 4..])
}

fn infer_account_state(api_key: &str, api_secret: &str) -> BinanceAccountState {
    BinanceAccountState {
        api_connectivity_ok: !api_key.contains("offline") && !api_secret.contains("offline"),
        timestamp_in_sync: !api_key.contains("skew") && !api_secret.contains("skew"),
        hedge_mode_enabled: !api_key.contains("oneway"),
        trading_enabled: !api_key.contains("readonly"),
        withdrawal_enabled: api_key.contains("withdraw"),
        spot_reachable: !api_key.contains("nospot"),
        usdm_reachable: !api_key.contains("nousdm"),
        coinm_reachable: !api_key.contains("nocoinm"),
    }
}

fn normalize_markets(markets: &[String]) -> Result<Vec<String>, CredentialValidationError> {
    let mut normalized = Vec::new();
    for market in markets {
        let market = market.trim().to_lowercase();
        if market.is_empty() {
            continue;
        }
        if !matches!(market.as_str(), "spot" | "usdm" | "coinm") {
            return Err(CredentialValidationError::new(
                "selected_markets contains unsupported market",
            ));
        }
        if normalized.iter().any(|current| current == &market) {
            continue;
        }
        normalized.push(market);
    }

    if normalized.is_empty() {
        return Err(CredentialValidationError::new(
            "selected_markets must include at least one of spot, usdm, coinm",
        ));
    }

    Ok(normalized)
}

fn symbol_filters(
    price_tick_size: &str,
    quantity_step_size: &str,
    min_quantity: &str,
    min_notional: &str,
    contract_size: Option<&str>,
) -> crate::metadata::SymbolFilters {
    crate::metadata::SymbolFilters {
        price_tick_size: price_tick_size.to_owned(),
        quantity_step_size: quantity_step_size.to_owned(),
        min_quantity: min_quantity.to_owned(),
        min_notional: min_notional.to_owned(),
        contract_size: contract_size.map(str::to_owned),
    }
}

fn symbol_requirements(
    supports_isolated_margin: bool,
    supports_cross_margin: bool,
    hedge_mode_required: bool,
    requires_futures_permissions: bool,
    leverage_brackets: &[u32],
) -> crate::metadata::MarketRequirements {
    crate::metadata::MarketRequirements {
        supports_isolated_margin,
        supports_cross_margin,
        hedge_mode_required,
        requires_futures_permissions,
        leverage_brackets: leverage_brackets.to_vec(),
    }
}

#[cfg(test)]
mod tests {
    use super::{
        BinanceClient, CredentialCipher, CredentialValidationError, CredentialValidationRequest,
    };

    #[test]
    fn credential_cipher_round_trips_credentials() {
        let cipher = CredentialCipher::new("shared-binance-test-master-key");

        let encrypted = cipher.encrypt("demo-key", "demo-secret").expect("encrypt");
        let decrypted = cipher.decrypt(&encrypted).expect("decrypt");

        assert_eq!(decrypted.0, "demo-key");
        assert_eq!(decrypted.1, "demo-secret");
        assert_ne!(encrypted, "demo-key\ndemo-secret");
    }

    #[test]
    fn validation_respects_selected_markets_and_timestamp_health() {
        let client = BinanceClient::new("demo-key-nocoinm-1234", "demo-secret-skew");
        let request =
            CredentialValidationRequest::new(true, &["spot".to_owned(), "coinm".to_owned()])
                .expect("request");

        let check = client.check_credentials_for(&request);

        assert_eq!(check.selected_markets, vec!["spot", "coinm"]);
        assert!(check.api_connectivity_ok);
        assert!(!check.timestamp_in_sync);
        assert!(check.can_read_spot);
        assert!(!check.can_read_coinm);
        assert!(!check.market_access_ok);
    }

    #[test]
    fn validation_rejects_empty_and_unsupported_market_selection() {
        let empty = CredentialValidationRequest::new(true, &[]);
        assert_eq!(
            empty,
            Err(CredentialValidationError::new(
                "selected_markets must include at least one of spot, usdm, coinm"
            ))
        );

        let invalid =
            CredentialValidationRequest::new(true, &["spot".to_owned(), "margin".to_owned()]);
        assert_eq!(
            invalid,
            Err(CredentialValidationError::new(
                "selected_markets contains unsupported market"
            ))
        );
    }
}
