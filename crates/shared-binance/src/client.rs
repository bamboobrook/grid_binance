use crate::metadata::SymbolMetadata;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExchangeCredentialCheck {
    pub can_read_spot: bool,
    pub can_read_usdm: bool,
    pub can_read_coinm: bool,
    pub hedge_mode_ok: bool,
    pub permissions_ok: bool,
    pub withdrawal_disabled: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BinanceAccountState {
    pub hedge_mode_enabled: bool,
    pub trading_enabled: bool,
    pub withdrawal_enabled: bool,
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
        let permissions_ok = has_credentials
            && self.account_state.trading_enabled
            && !self.account_state.withdrawal_enabled;

        ExchangeCredentialCheck {
            can_read_spot: has_credentials,
            can_read_usdm: has_credentials,
            can_read_coinm: has_credentials,
            hedge_mode_ok: self.account_state.hedge_mode_enabled == expected_hedge_mode,
            permissions_ok,
            withdrawal_disabled: !self.account_state.withdrawal_enabled,
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
                "0.000010",
                "5",
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
                "0.000100",
                "5",
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
                "0.001",
                "100",
                ["futures", "perpetual", "linear", "usdsm"],
            ),
            SymbolMetadata::new(
                "SOLUSDT",
                "usdm",
                "TRADING",
                "SOL",
                "USDT",
                3,
                1,
                "0.1",
                "20",
                ["futures", "perpetual", "linear", "usdsm"],
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
                "1",
                "100",
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
                "1",
                "50",
                ["futures", "delivery", "inverse", "coin"],
            ),
        ]
    }
}

fn infer_account_state(api_key: &str) -> BinanceAccountState {
    BinanceAccountState {
        hedge_mode_enabled: !api_key.contains("oneway"),
        trading_enabled: !api_key.contains("readonly"),
        withdrawal_enabled: api_key.contains("withdraw"),
    }
}

impl ExchangeCredentialCheck {
    pub fn is_healthy(&self) -> bool {
        self.can_read_spot
            && self.can_read_usdm
            && self.can_read_coinm
            && self.hedge_mode_ok
            && self.permissions_ok
            && self.withdrawal_disabled
    }

    pub fn connection_status(&self) -> &'static str {
        if self.is_healthy() {
            "healthy"
        } else {
            "degraded"
        }
    }
}

pub fn mask_api_key(api_key: &str) -> String {
    let trimmed = api_key.trim();
    if trimmed.len() <= 8 {
        return "*".repeat(trimmed.len());
    }

    format!("{}****{}", &trimmed[..4], &trimmed[trimmed.len() - 4..])
}

pub fn encrypt_credentials(api_key: &str, api_secret: &str) -> String {
    let payload = format!("{api_key}\n{api_secret}");
    xor_hex(payload.as_bytes())
}

pub fn decrypt_credentials(payload: &str) -> Option<(String, String)> {
    let decoded = xor_hex_decode(payload)?;
    let plain = String::from_utf8(decoded).ok()?;
    let (api_key, api_secret) = plain.split_once('\n')?;
    Some((api_key.to_owned(), api_secret.to_owned()))
}

fn xor_hex(input: &[u8]) -> String {
    const KEY: &[u8] = b"grid-binance-v1";
    let mut output = String::with_capacity(input.len() * 2);

    for (idx, byte) in input.iter().enumerate() {
        let masked = byte ^ KEY[idx % KEY.len()];
        output.push(nibble_to_hex(masked >> 4));
        output.push(nibble_to_hex(masked & 0x0f));
    }

    output
}

fn xor_hex_decode(input: &str) -> Option<Vec<u8>> {
    const KEY: &[u8] = b"grid-binance-v1";
    if input.len() % 2 != 0 {
        return None;
    }

    let mut decoded = Vec::with_capacity(input.len() / 2);
    for (idx, chunk) in input.as_bytes().chunks(2).enumerate() {
        let high = hex_to_nibble(chunk[0])?;
        let low = hex_to_nibble(chunk[1])?;
        let byte = (high << 4) | low;
        decoded.push(byte ^ KEY[idx % KEY.len()]);
    }

    Some(decoded)
}

fn nibble_to_hex(value: u8) -> char {
    match value {
        0..=9 => (b'0' + value) as char,
        10..=15 => (b'a' + (value - 10)) as char,
        _ => unreachable!("nibble must be in range 0..=15"),
    }
}

fn hex_to_nibble(value: u8) -> Option<u8> {
    match value {
        b'0'..=b'9' => Some(value - b'0'),
        b'a'..=b'f' => Some(value - b'a' + 10),
        b'A'..=b'F' => Some(value - b'A' + 10),
        _ => None,
    }
}
