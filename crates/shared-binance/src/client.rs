use aes_gcm::{
    aead::{Aead, KeyInit},
    Aes256Gcm, Key, Nonce,
};
use base64::{engine::general_purpose::STANDARD as BASE64, Engine as _};
use hmac::{Hmac, Mac};
use reqwest::blocking::Client as HttpClient;
use rust_decimal::Decimal;
use serde::{de::DeserializeOwned, Deserialize};
use sha2::{Digest, Sha256};
use std::{
    collections::BTreeMap,
    env,
    time::{Duration, SystemTime, UNIX_EPOCH},
};

use crate::metadata::SymbolMetadata;

const NONCE_SIZE: usize = 12;
const DEFAULT_HTTP_TIMEOUT_MS: u64 = 5_000;
const DEFAULT_RECV_WINDOW_MS: u64 = 5_000;
const MAX_TIMESTAMP_SKEW_MS: i64 = 5_000;
const LIVE_MODE_ENV: &str = "BINANCE_LIVE_MODE";
const SPOT_REST_BASE_URL_ENV: &str = "BINANCE_SPOT_REST_BASE_URL";
const USDM_REST_BASE_URL_ENV: &str = "BINANCE_USDM_REST_BASE_URL";
const COINM_REST_BASE_URL_ENV: &str = "BINANCE_COINM_REST_BASE_URL";
const SPOT_WS_BASE_URL_ENV: &str = "BINANCE_SPOT_WS_BASE_URL";
const USDM_WS_BASE_URL_ENV: &str = "BINANCE_USDM_WS_BASE_URL";
const COINM_WS_BASE_URL_ENV: &str = "BINANCE_COINM_WS_BASE_URL";
const SPOT_REST_BASE_URL: &str = "https://api.binance.com";
const USDM_REST_BASE_URL: &str = "https://fapi.binance.com";
const COINM_REST_BASE_URL: &str = "https://dapi.binance.com";
const SPOT_WS_BASE_URL: &str = "wss://stream.binance.com:9443/ws";
const USDM_WS_BASE_URL: &str = "wss://fstream.binance.com/ws";
const COINM_WS_BASE_URL: &str = "wss://dstream.binance.com/ws";

type HmacSha256 = Hmac<Sha256>;

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
pub struct BinanceOrderRequest {
    pub market: String,
    pub symbol: String,
    pub side: String,
    pub order_type: String,
    pub quantity: String,
    pub price: Option<String>,
    pub time_in_force: Option<String>,
    pub reduce_only: Option<bool>,
    pub position_side: Option<String>,
    pub client_order_id: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BinanceOrderResponse {
    pub market: String,
    pub symbol: String,
    pub order_id: String,
    pub client_order_id: Option<String>,
    pub status: String,
    pub side: Option<String>,
    pub order_type: Option<String>,
    pub price: Option<String>,
    pub quantity: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BinanceUserDataStream {
    pub market: String,
    pub listen_key: String,
    pub websocket_url: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BinanceExecutionUpdate {
    pub market: String,
    pub symbol: String,
    pub order_id: String,
    pub client_order_id: Option<String>,
    pub side: Option<String>,
    pub order_type: Option<String>,
    pub status: String,
    pub execution_type: Option<String>,
    pub order_price: Option<String>,
    pub last_fill_price: Option<String>,
    pub last_fill_quantity: Option<String>,
    pub cumulative_fill_quantity: Option<String>,
    pub fee_amount: Option<String>,
    pub fee_asset: Option<String>,
    pub position_side: Option<String>,
    pub trade_id: Option<String>,
    pub realized_profit: Option<String>,
    pub event_time_ms: i64,
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
    live_config: BinanceLiveConfig,
}

#[derive(Debug, Clone)]
pub struct CredentialCipher {
    key: [u8; 32],
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CredentialCipherError {
    message: String,
}

#[derive(Debug, Clone)]
struct BinanceLiveConfig {
    enabled: bool,
    spot_rest_base_url: String,
    usdm_rest_base_url: String,
    coinm_rest_base_url: String,
    spot_ws_base_url: String,
    usdm_ws_base_url: String,
    coinm_ws_base_url: String,
    timeout: Duration,
    recv_window_ms: u64,
}

#[derive(Debug, Clone, Copy)]
enum BinanceMarket {
    Spot,
    Usdm,
    Coinm,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ServerTimeResponse {
    server_time: i64,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct SpotAccountResponse {
    #[serde(default)]
    can_trade: bool,
    #[serde(default)]
    can_withdraw: bool,
    #[serde(default)]
    permissions: Vec<String>,
    #[serde(default)]
    balances: Vec<SpotAccountBalance>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct SpotAccountBalance {
    asset: String,
    free: FlexibleValue,
    locked: FlexibleValue,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct FuturesAccountResponse {
    #[serde(default)]
    can_trade: bool,
    total_wallet_balance: Option<FlexibleValue>,
    total_unrealized_profit: Option<FlexibleValue>,
    #[serde(default)]
    assets: Vec<FuturesAssetBalance>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct FuturesAssetBalance {
    asset: String,
    wallet_balance: FlexibleValue,
    unrealized_profit: Option<FlexibleValue>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct PositionSideModeResponse {
    dual_side_position: bool,
}

#[derive(Debug, Deserialize)]
struct ExchangeInfoResponse {
    #[serde(default)]
    symbols: Vec<ExchangeInfoSymbol>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ExchangeInfoSymbol {
    symbol: String,
    status: String,
    base_asset: String,
    quote_asset: String,
    quote_precision: Option<u32>,
    base_asset_precision: Option<u32>,
    price_precision: Option<u32>,
    quantity_precision: Option<u32>,
    contract_size: Option<FlexibleValue>,
    contract_type: Option<String>,
    #[serde(default)]
    filters: Vec<ExchangeFilter>,
    #[serde(default)]
    permissions: Vec<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ExchangeFilter {
    filter_type: String,
    tick_size: Option<FlexibleValue>,
    step_size: Option<FlexibleValue>,
    min_qty: Option<FlexibleValue>,
    min_notional: Option<FlexibleValue>,
    notional: Option<FlexibleValue>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(untagged)]
enum FlexibleIdentifier {
    Text(String),
    Integer(i64),
    Unsigned(u64),
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct OrderResponsePayload {
    symbol: String,
    order_id: FlexibleIdentifier,
    client_order_id: Option<String>,
    status: String,
    side: Option<String>,
    #[serde(rename = "type")]
    order_type: Option<String>,
    price: Option<FlexibleValue>,
    orig_qty: Option<FlexibleValue>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct UserTradePayload {
    id: FlexibleIdentifier,
    #[serde(default)]
    order_id: Option<FlexibleIdentifier>,
    symbol: String,
    price: FlexibleValue,
    qty: FlexibleValue,
    commission: Option<FlexibleValue>,
    commission_asset: Option<String>,
    time: i64,
    #[serde(default)]
    is_buyer: bool,
    #[serde(default, alias = "rp", alias = "realizedPnl", alias = "realizedProfit")]
    realized_profit: Option<FlexibleValue>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ListenKeyResponse {
    listen_key: String,
}

#[derive(Debug, Deserialize)]
struct SpotExecutionReportPayload {
    #[serde(rename = "e")]
    event_type: String,
    #[serde(rename = "E")]
    event_time_ms: i64,
    #[serde(rename = "s")]
    symbol: String,
    #[serde(rename = "c")]
    client_order_id: String,
    #[serde(rename = "S")]
    side: String,
    #[serde(rename = "o")]
    order_type: String,
    #[serde(rename = "x")]
    execution_type: String,
    #[serde(rename = "X")]
    status: String,
    #[serde(rename = "i")]
    order_id: FlexibleIdentifier,
    #[serde(rename = "p")]
    order_price: FlexibleValue,
    #[serde(rename = "L")]
    last_fill_price: Option<FlexibleValue>,
    #[serde(rename = "l")]
    last_fill_quantity: Option<FlexibleValue>,
    #[serde(rename = "z")]
    cumulative_fill_quantity: Option<FlexibleValue>,
    #[serde(rename = "n")]
    fee_amount: Option<FlexibleValue>,
    #[serde(rename = "N")]
    fee_asset: Option<String>,
    #[serde(rename = "t")]
    trade_id: Option<FlexibleIdentifier>,
}

#[derive(Debug, Deserialize)]
struct FuturesOrderTradeUpdateEnvelope {
    #[serde(rename = "e")]
    event_type: String,
    #[serde(rename = "E")]
    event_time_ms: i64,
    #[serde(rename = "o")]
    order: FuturesOrderTradePayload,
}

#[derive(Debug, Deserialize)]
struct FuturesOrderTradePayload {
    #[serde(rename = "s")]
    symbol: String,
    #[serde(rename = "c")]
    client_order_id: String,
    #[serde(rename = "S")]
    side: String,
    #[serde(rename = "o")]
    order_type: String,
    #[serde(rename = "x")]
    execution_type: String,
    #[serde(rename = "X")]
    status: String,
    #[serde(rename = "i")]
    order_id: FlexibleIdentifier,
    #[serde(rename = "p")]
    order_price: FlexibleValue,
    #[serde(rename = "L")]
    last_fill_price: Option<FlexibleValue>,
    #[serde(rename = "l")]
    last_fill_quantity: Option<FlexibleValue>,
    #[serde(rename = "z")]
    cumulative_fill_quantity: Option<FlexibleValue>,
    #[serde(rename = "n")]
    fee_amount: Option<FlexibleValue>,
    #[serde(rename = "N")]
    fee_asset: Option<String>,
    #[serde(rename = "ps")]
    position_side: Option<String>,
    #[serde(rename = "t")]
    trade_id: Option<FlexibleIdentifier>,
    #[serde(rename = "rp")]
    realized_profit: Option<FlexibleValue>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BinanceUserTrade {
    pub market: String,
    pub trade_id: String,
    pub order_id: Option<String>,
    pub symbol: String,
    pub side: String,
    pub price: String,
    pub quantity: String,
    pub fee_amount: Option<String>,
    pub fee_asset: Option<String>,
    pub realized_profit: Option<String>,
    pub traded_at_ms: i64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BinanceSnapshotBundle {
    pub account_snapshots: Vec<BinanceAccountSnapshot>,
    pub wallet_snapshots: Vec<BinanceWalletSnapshot>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BinanceAccountSnapshot {
    pub exchange: String,
    pub realized_pnl: String,
    pub unrealized_pnl: String,
    pub fees: String,
    pub funding: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BinanceWalletSnapshot {
    pub exchange: String,
    pub wallet_type: String,
    pub balances: BTreeMap<String, String>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(untagged)]
enum FlexibleValue {
    Text(String),
    Integer(i64),
    Float(f64),
}

#[derive(Debug, Default)]
struct LiveCheckState {
    api_connectivity_ok: bool,
    saw_timestamp: bool,
    timestamp_in_sync: bool,
    can_read_spot: bool,
    can_read_usdm: bool,
    can_read_coinm: bool,
    hedge_mode_ok: bool,
    saw_futures_permissions: bool,
    permissions_ok: bool,
    withdrawal_disabled: bool,
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
            live_config: BinanceLiveConfig::from_env(),
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
        if !self.live_config.enabled {
            return self.offline_credential_check(request);
        }

        self.live_credential_check(request)
            .unwrap_or_else(|_| failed_live_check(request))
    }

    pub fn place_order(
        &self,
        request: BinanceOrderRequest,
    ) -> Result<BinanceOrderResponse, CredentialValidationError> {
        if !self.live_config.enabled {
            return Err(CredentialValidationError::new(
                "binance live mode is disabled",
            ));
        }
        let market = BinanceMarket::from_scope(&request.market)?;
        let http = self.live_http_client()?;
        let server_time = self.fetch_server_time(&http, market)?;
        let mut params = vec![
            ("symbol".to_string(), request.symbol),
            ("side".to_string(), request.side),
            ("type".to_string(), request.order_type),
            ("quantity".to_string(), request.quantity),
        ];
        if let Some(price) = request.price {
            params.push(("price".to_string(), price));
        }
        if let Some(time_in_force) = request.time_in_force {
            params.push(("timeInForce".to_string(), time_in_force));
        }
        if let Some(reduce_only) = request.reduce_only {
            params.push((
                "reduceOnly".to_string(),
                if reduce_only { "true" } else { "false" }.to_string(),
            ));
        }
        if let Some(position_side) = request.position_side {
            params.push(("positionSide".to_string(), position_side));
        }
        if let Some(client_order_id) = request.client_order_id {
            params.push(("newClientOrderId".to_string(), client_order_id));
        }
        let payload: OrderResponsePayload = self.signed_request(
            &http,
            "POST",
            self.live_config.base_url(market),
            market.order_path(),
            server_time,
            &params,
        )?;
        Ok(BinanceOrderResponse::from_payload(market, payload))
    }

    pub fn cancel_order(
        &self,
        market: &str,
        symbol: &str,
        order_id: Option<&str>,
        client_order_id: Option<&str>,
    ) -> Result<BinanceOrderResponse, CredentialValidationError> {
        if !self.live_config.enabled {
            return Err(CredentialValidationError::new(
                "binance live mode is disabled",
            ));
        }
        if order_id.is_none() && client_order_id.is_none() {
            return Err(CredentialValidationError::new(
                "cancel order requires order_id or client_order_id",
            ));
        }
        let market = BinanceMarket::from_scope(market)?;
        let http = self.live_http_client()?;
        let server_time = self.fetch_server_time(&http, market)?;
        let mut params = vec![("symbol".to_string(), symbol.to_string())];
        if let Some(order_id) = order_id {
            params.push(("orderId".to_string(), order_id.to_string()));
        }
        if let Some(client_order_id) = client_order_id {
            params.push(("origClientOrderId".to_string(), client_order_id.to_string()));
        }
        let payload: OrderResponsePayload = self.signed_request(
            &http,
            "DELETE",
            self.live_config.base_url(market),
            market.order_path(),
            server_time,
            &params,
        )?;
        Ok(BinanceOrderResponse::from_payload(market, payload))
    }

    pub fn keepalive_user_data_stream(
        &self,
        market: &str,
        listen_key: &str,
    ) -> Result<(), CredentialValidationError> {
        if !self.live_config.enabled {
            return Err(CredentialValidationError::new(
                "binance live mode is disabled",
            ));
        }
        let market = BinanceMarket::from_scope(market)?;
        let http = self.live_http_client()?;
        self.api_key_request::<serde_json::Value>(
            &http,
            "PUT",
            self.live_config.base_url(market),
            market.user_data_stream_path(),
            &[("listenKey".to_string(), listen_key.to_string())],
        )?;
        Ok(())
    }

    pub fn start_user_data_stream(
        &self,
        market: &str,
    ) -> Result<BinanceUserDataStream, CredentialValidationError> {
        if !self.live_config.enabled {
            return Err(CredentialValidationError::new(
                "binance live mode is disabled",
            ));
        }
        let market = BinanceMarket::from_scope(market)?;
        let http = self.live_http_client()?;
        let response: ListenKeyResponse = self.api_key_request(
            &http,
            "POST",
            self.live_config.base_url(market),
            market.user_data_stream_path(),
            &[],
        )?;
        Ok(BinanceUserDataStream {
            market: market.as_str().to_string(),
            websocket_url: format!(
                "{}/{}",
                self.live_config.ws_base_url(market),
                response.listen_key
            ),
            listen_key: response.listen_key,
        })
    }

    pub fn get_order(
        &self,
        market: &str,
        symbol: &str,
        order_id: Option<&str>,
        client_order_id: Option<&str>,
    ) -> Result<BinanceOrderResponse, CredentialValidationError> {
        if !self.live_config.enabled {
            return Err(CredentialValidationError::new(
                "binance live mode is disabled",
            ));
        }
        if order_id.is_none() && client_order_id.is_none() {
            return Err(CredentialValidationError::new(
                "get order requires order_id or client_order_id",
            ));
        }
        let market = BinanceMarket::from_scope(market)?;
        let http = self.live_http_client()?;
        let server_time = self.fetch_server_time(&http, market)?;
        let mut params = vec![("symbol".to_string(), symbol.to_string())];
        if let Some(order_id) = order_id {
            params.push(("orderId".to_string(), order_id.to_string()));
        }
        if let Some(client_order_id) = client_order_id {
            params.push(("origClientOrderId".to_string(), client_order_id.to_string()));
        }
        let payload: OrderResponsePayload = self.signed_request(
            &http,
            "GET",
            self.live_config.base_url(market),
            market.order_path(),
            server_time,
            &params,
        )?;
        Ok(BinanceOrderResponse::from_payload(market, payload))
    }

    pub fn user_trades(
        &self,
        market: &str,
        symbol: &str,
        limit: usize,
    ) -> Result<Vec<BinanceUserTrade>, CredentialValidationError> {
        if !self.live_config.enabled {
            return Err(CredentialValidationError::new(
                "binance live mode is disabled",
            ));
        }
        let market = BinanceMarket::from_scope(market)?;
        let http = self.live_http_client()?;
        let server_time = self.fetch_server_time(&http, market)?;
        let payload: Vec<UserTradePayload> = self.signed_request(
            &http,
            "GET",
            self.live_config.base_url(market),
            market.user_trades_path(),
            server_time,
            &[
                ("symbol".to_string(), symbol.to_string()),
                ("limit".to_string(), limit.to_string()),
            ],
        )?;
        Ok(payload
            .into_iter()
            .map(|trade| BinanceUserTrade::from_payload(market, trade))
            .collect())
    }

    pub fn snapshot_bundle(
        &self,
        selected_markets: &[String],
    ) -> Result<BinanceSnapshotBundle, CredentialValidationError> {
        if !self.live_config.enabled {
            return Err(CredentialValidationError::new(
                "binance live mode is disabled",
            ));
        }
        let markets = normalize_markets(selected_markets)?;
        let http = self.live_http_client()?;
        let now = current_timestamp_ms();
        let mut bundle = BinanceSnapshotBundle {
            account_snapshots: Vec::new(),
            wallet_snapshots: Vec::new(),
        };
        for market in markets {
            let market = BinanceMarket::from_scope(&market)?;
            match market {
                BinanceMarket::Spot => {
                    let account: SpotAccountResponse = self.signed_request(
                        &http,
                        "GET",
                        self.live_config.base_url(market),
                        market.account_path(),
                        now,
                        &[],
                    )?;
                    bundle.account_snapshots.push(BinanceAccountSnapshot {
                        exchange: "binance".to_string(),
                        realized_pnl: "0".to_string(),
                        unrealized_pnl: "0".to_string(),
                        fees: "0".to_string(),
                        funding: None,
                    });
                    bundle.wallet_snapshots.push(BinanceWalletSnapshot {
                        exchange: "binance".to_string(),
                        wallet_type: "spot".to_string(),
                        balances: spot_balances(&account.balances),
                    });
                }
                BinanceMarket::Usdm | BinanceMarket::Coinm => {
                    let account: FuturesAccountResponse = self.signed_request(
                        &http,
                        "GET",
                        self.live_config.base_url(market),
                        market.account_path(),
                        now,
                        &[],
                    )?;
                    let exchange = format!("binance-{}", market.as_str());
                    bundle.account_snapshots.push(BinanceAccountSnapshot {
                        exchange: exchange.clone(),
                        realized_pnl: "0".to_string(),
                        unrealized_pnl: account
                            .total_unrealized_profit
                            .as_ref()
                            .map(flexible_value_ref_to_string)
                            .unwrap_or_else(|| "0".to_string()),
                        fees: "0".to_string(),
                        funding: None,
                    });
                    bundle.wallet_snapshots.push(BinanceWalletSnapshot {
                        exchange,
                        wallet_type: market.as_str().to_string(),
                        balances: futures_balances(&account.assets),
                    });
                }
            }
        }
        Ok(bundle)
    }

    pub fn account_state(&self) -> &BinanceAccountState {
        &self.account_state
    }

    pub fn spot_symbols(&self) -> Vec<SymbolMetadata> {
        self.spot_symbols_strict()
            .unwrap_or_else(|_| offline_spot_symbols())
    }

    pub fn spot_symbols_strict(&self) -> Result<Vec<SymbolMetadata>, CredentialValidationError> {
        if !self.live_config.enabled {
            return Ok(offline_spot_symbols());
        }

        self.live_symbols(BinanceMarket::Spot)
    }

    pub fn usdm_symbols(&self) -> Vec<SymbolMetadata> {
        self.usdm_symbols_strict()
            .unwrap_or_else(|_| offline_usdm_symbols())
    }

    pub fn usdm_symbols_strict(&self) -> Result<Vec<SymbolMetadata>, CredentialValidationError> {
        if !self.live_config.enabled {
            return Ok(offline_usdm_symbols());
        }

        self.live_symbols(BinanceMarket::Usdm)
    }

    pub fn coinm_symbols(&self) -> Vec<SymbolMetadata> {
        self.coinm_symbols_strict()
            .unwrap_or_else(|_| offline_coinm_symbols())
    }

    pub fn coinm_symbols_strict(&self) -> Result<Vec<SymbolMetadata>, CredentialValidationError> {
        if !self.live_config.enabled {
            return Ok(offline_coinm_symbols());
        }

        self.live_symbols(BinanceMarket::Coinm)
    }

    fn offline_credential_check(
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

    fn live_credential_check(
        &self,
        request: &CredentialValidationRequest,
    ) -> Result<ExchangeCredentialCheck, CredentialValidationError> {
        let http = self.live_http_client()?;
        let mut state = LiveCheckState {
            timestamp_in_sync: true,
            hedge_mode_ok: true,
            permissions_ok: true,
            withdrawal_disabled: true,
            ..LiveCheckState::default()
        };

        for market in &request.selected_markets {
            match market.as_str() {
                "spot" => {
                    if self.check_live_spot_market(&http, &mut state).is_err() {
                        state.withdrawal_disabled = false;
                    }
                }
                "usdm" => {
                    let _ = self.check_live_futures_market(
                        &http,
                        &mut state,
                        BinanceMarket::Usdm,
                        request.expected_hedge_mode,
                    );
                }
                "coinm" => {
                    let _ = self.check_live_futures_market(
                        &http,
                        &mut state,
                        BinanceMarket::Coinm,
                        request.expected_hedge_mode,
                    );
                }
                _ => {}
            }
        }

        if !state.saw_timestamp {
            state.timestamp_in_sync = false;
        }
        if !state.saw_futures_permissions
            && !request
                .selected_markets
                .iter()
                .any(|market| market == "spot")
        {
            state.permissions_ok = false;
        }

        let market_access_ok =
            request
                .selected_markets
                .iter()
                .all(|market| match market.as_str() {
                    "spot" => state.can_read_spot,
                    "usdm" => state.can_read_usdm,
                    "coinm" => state.can_read_coinm,
                    _ => false,
                });
        let futures_selected = request
            .selected_markets
            .iter()
            .any(|market| market == "usdm" || market == "coinm");

        Ok(ExchangeCredentialCheck {
            selected_markets: request.selected_markets.clone(),
            api_connectivity_ok: state.api_connectivity_ok,
            timestamp_in_sync: state.timestamp_in_sync,
            can_read_spot: state.can_read_spot,
            can_read_usdm: state.can_read_usdm,
            can_read_coinm: state.can_read_coinm,
            hedge_mode_ok: if futures_selected {
                state.hedge_mode_ok
            } else {
                true
            },
            permissions_ok: state.api_connectivity_ok && state.permissions_ok,
            withdrawal_disabled: state.withdrawal_disabled,
            market_access_ok,
        })
    }

    fn check_live_spot_market(
        &self,
        http: &HttpClient,
        state: &mut LiveCheckState,
    ) -> Result<(), CredentialValidationError> {
        let server_time = self.fetch_server_time(http, BinanceMarket::Spot)?;
        state.saw_timestamp = true;
        state.timestamp_in_sync &= timestamp_is_in_sync(server_time);
        let account = self.fetch_spot_account(http, server_time)?;
        state.api_connectivity_ok = true;
        state.permissions_ok &= account.can_trade;
        state.withdrawal_disabled &= !account.can_withdraw;
        state.can_read_spot = account.permissions.is_empty()
            || account
                .permissions
                .iter()
                .any(|permission| permission.eq_ignore_ascii_case("SPOT"));
        Ok(())
    }

    fn check_live_futures_market(
        &self,
        http: &HttpClient,
        state: &mut LiveCheckState,
        market: BinanceMarket,
        expected_hedge_mode: bool,
    ) -> Result<(), CredentialValidationError> {
        let server_time = self.fetch_server_time(http, market)?;
        state.saw_timestamp = true;
        state.timestamp_in_sync &= timestamp_is_in_sync(server_time);
        let account = self.fetch_futures_account(http, market, server_time)?;
        let position_mode = self.fetch_position_mode(http, market, server_time)?;

        state.api_connectivity_ok = true;
        state.saw_futures_permissions = true;
        state.permissions_ok &= account.can_trade;
        state.hedge_mode_ok &= position_mode.dual_side_position == expected_hedge_mode;
        match market {
            BinanceMarket::Usdm => state.can_read_usdm = true,
            BinanceMarket::Coinm => state.can_read_coinm = true,
            BinanceMarket::Spot => {}
        }
        Ok(())
    }

    fn live_symbols(
        &self,
        market: BinanceMarket,
    ) -> Result<Vec<SymbolMetadata>, CredentialValidationError> {
        let http = self.live_http_client()?;
        let exchange_info: ExchangeInfoResponse = self.public_get(
            &http,
            self.live_config.base_url(market),
            market.exchange_info_path(),
        )?;

        Ok(exchange_info
            .symbols
            .into_iter()
            .map(|symbol| map_exchange_info_symbol(market, symbol))
            .collect())
    }

    fn live_http_client(&self) -> Result<HttpClient, CredentialValidationError> {
        HttpClient::builder()
            .timeout(self.live_config.timeout)
            .build()
            .map_err(|error| {
                CredentialValidationError::new(format!(
                    "failed to build live binance client: {error}"
                ))
            })
    }

    fn fetch_server_time(
        &self,
        http: &HttpClient,
        market: BinanceMarket,
    ) -> Result<i64, CredentialValidationError> {
        let response: ServerTimeResponse =
            self.public_get(http, self.live_config.base_url(market), market.time_path())?;
        Ok(response.server_time)
    }

    fn fetch_spot_account(
        &self,
        http: &HttpClient,
        server_time: i64,
    ) -> Result<SpotAccountResponse, CredentialValidationError> {
        self.signed_get(
            http,
            self.live_config.base_url(BinanceMarket::Spot),
            BinanceMarket::Spot.account_path(),
            server_time,
        )
    }

    fn fetch_futures_account(
        &self,
        http: &HttpClient,
        market: BinanceMarket,
        server_time: i64,
    ) -> Result<FuturesAccountResponse, CredentialValidationError> {
        self.signed_get(
            http,
            self.live_config.base_url(market),
            market.account_path(),
            server_time,
        )
    }

    fn fetch_position_mode(
        &self,
        http: &HttpClient,
        market: BinanceMarket,
        server_time: i64,
    ) -> Result<PositionSideModeResponse, CredentialValidationError> {
        self.signed_get(
            http,
            self.live_config.base_url(market),
            market.position_side_path(),
            server_time,
        )
    }

    fn public_get<T: DeserializeOwned>(
        &self,
        http: &HttpClient,
        base_url: &str,
        path: &str,
    ) -> Result<T, CredentialValidationError> {
        let url = format!("{base_url}{path}");
        let response = http.get(url).send().map_err(|error| {
            CredentialValidationError::new(format!("binance request failed: {error}"))
        })?;
        parse_json_response(response)
    }

    fn api_key_request<T: DeserializeOwned>(
        &self,
        http: &HttpClient,
        method: &str,
        base_url: &str,
        path: &str,
        params: &[(String, String)],
    ) -> Result<T, CredentialValidationError> {
        let method = reqwest::Method::from_bytes(method.as_bytes()).map_err(|error| {
            CredentialValidationError::new(format!("invalid http method: {error}"))
        })?;
        let mut request = http
            .request(method, format!("{base_url}{path}"))
            .header("X-MBX-APIKEY", &self.api_key);
        if !params.is_empty() {
            request = request.form(params);
        }
        let response = request.send().map_err(|error| {
            CredentialValidationError::new(format!("binance api-key request failed: {error}"))
        })?;
        parse_json_response(response)
    }

    fn signed_get<T: DeserializeOwned>(
        &self,
        http: &HttpClient,
        base_url: &str,
        path: &str,
        server_time: i64,
    ) -> Result<T, CredentialValidationError> {
        self.signed_request(http, "GET", base_url, path, server_time, &[])
    }

    fn signed_request<T: DeserializeOwned>(
        &self,
        http: &HttpClient,
        method: &str,
        base_url: &str,
        path: &str,
        server_time: i64,
        params: &[(String, String)],
    ) -> Result<T, CredentialValidationError> {
        let mut query = params
            .iter()
            .map(|(key, value)| format!("{key}={value}"))
            .collect::<Vec<_>>();
        query.push(format!("timestamp={server_time}"));
        query.push(format!("recvWindow={}", self.live_config.recv_window_ms));
        let query = query.join("&");
        let signature = sign_query(&self.api_secret, &query)?;
        let url = format!("{base_url}{path}?{query}&signature={signature}");
        let method = reqwest::Method::from_bytes(method.as_bytes()).map_err(|error| {
            CredentialValidationError::new(format!("invalid http method: {error}"))
        })?;
        let response = http
            .request(method, url)
            .header("X-MBX-APIKEY", &self.api_key)
            .send()
            .map_err(|error| {
                CredentialValidationError::new(format!("binance signed request failed: {error}"))
            })?;
        parse_json_response(response)
    }
}

impl BinanceUserTrade {
    fn from_payload(market: BinanceMarket, payload: UserTradePayload) -> Self {
        Self {
            market: market.as_str().to_string(),
            trade_id: flexible_identifier_to_string(payload.id),
            order_id: payload.order_id.map(flexible_identifier_to_string),
            symbol: payload.symbol,
            side: if payload.is_buyer {
                "BUY".to_string()
            } else {
                "SELL".to_string()
            },
            price: flexible_scalar_to_string(payload.price),
            quantity: flexible_scalar_to_string(payload.qty),
            fee_amount: payload.commission.map(flexible_scalar_to_string),
            fee_asset: payload.commission_asset,
            realized_profit: payload.realized_profit.map(flexible_scalar_to_string),
            traded_at_ms: payload.time,
        }
    }
}

impl BinanceOrderResponse {
    fn from_payload(market: BinanceMarket, payload: OrderResponsePayload) -> Self {
        Self {
            market: market.as_str().to_string(),
            symbol: payload.symbol,
            order_id: flexible_identifier_to_string(payload.order_id),
            client_order_id: payload.client_order_id,
            status: payload.status,
            side: payload.side,
            order_type: payload.order_type,
            price: payload.price.map(flexible_scalar_to_string),
            quantity: payload.orig_qty.map(flexible_scalar_to_string),
        }
    }
}

impl BinanceMarket {
    fn from_scope(scope: &str) -> Result<Self, CredentialValidationError> {
        match scope.trim().to_ascii_lowercase().as_str() {
            "spot" => Ok(Self::Spot),
            "usdm" => Ok(Self::Usdm),
            "coinm" => Ok(Self::Coinm),
            _ => Err(CredentialValidationError::new("unsupported order market")),
        }
    }

    fn as_str(self) -> &'static str {
        match self {
            Self::Spot => "spot",
            Self::Usdm => "usdm",
            Self::Coinm => "coinm",
        }
    }

    fn exchange_info_path(self) -> &'static str {
        match self {
            Self::Spot => "/api/v3/exchangeInfo",
            Self::Usdm => "/fapi/v1/exchangeInfo",
            Self::Coinm => "/dapi/v1/exchangeInfo",
        }
    }

    fn time_path(self) -> &'static str {
        match self {
            Self::Spot => "/api/v3/time",
            Self::Usdm => "/fapi/v1/time",
            Self::Coinm => "/dapi/v1/time",
        }
    }

    fn account_path(self) -> &'static str {
        match self {
            Self::Spot => "/api/v3/account",
            Self::Usdm => "/fapi/v2/account",
            Self::Coinm => "/dapi/v1/account",
        }
    }

    fn order_path(self) -> &'static str {
        match self {
            Self::Spot => "/api/v3/order",
            Self::Usdm => "/fapi/v1/order",
            Self::Coinm => "/dapi/v1/order",
        }
    }

    fn user_trades_path(self) -> &'static str {
        match self {
            Self::Spot => "/api/v3/myTrades",
            Self::Usdm => "/fapi/v1/userTrades",
            Self::Coinm => "/dapi/v1/userTrades",
        }
    }

    fn user_data_stream_path(self) -> &'static str {
        match self {
            Self::Spot => "/api/v3/userDataStream",
            Self::Usdm => "/fapi/v1/listenKey",
            Self::Coinm => "/dapi/v1/listenKey",
        }
    }

    fn position_side_path(self) -> &'static str {
        match self {
            Self::Spot => "/api/v3/account",
            Self::Usdm => "/fapi/v1/positionSide/dual",
            Self::Coinm => "/dapi/v1/positionSide/dual",
        }
    }
}

impl BinanceLiveConfig {
    fn from_env() -> Self {
        Self {
            enabled: env_flag(LIVE_MODE_ENV),
            spot_rest_base_url: env_url(SPOT_REST_BASE_URL_ENV, SPOT_REST_BASE_URL),
            usdm_rest_base_url: env_url(USDM_REST_BASE_URL_ENV, USDM_REST_BASE_URL),
            coinm_rest_base_url: env_url(COINM_REST_BASE_URL_ENV, COINM_REST_BASE_URL),
            spot_ws_base_url: env_url(SPOT_WS_BASE_URL_ENV, SPOT_WS_BASE_URL),
            usdm_ws_base_url: env_url(USDM_WS_BASE_URL_ENV, USDM_WS_BASE_URL),
            coinm_ws_base_url: env_url(COINM_WS_BASE_URL_ENV, COINM_WS_BASE_URL),
            timeout: Duration::from_millis(DEFAULT_HTTP_TIMEOUT_MS),
            recv_window_ms: DEFAULT_RECV_WINDOW_MS,
        }
    }

    fn base_url(&self, market: BinanceMarket) -> &str {
        match market {
            BinanceMarket::Spot => &self.spot_rest_base_url,
            BinanceMarket::Usdm => &self.usdm_rest_base_url,
            BinanceMarket::Coinm => &self.coinm_rest_base_url,
        }
    }

    fn ws_base_url(&self, market: BinanceMarket) -> &str {
        match market {
            BinanceMarket::Spot => &self.spot_ws_base_url,
            BinanceMarket::Usdm => &self.usdm_ws_base_url,
            BinanceMarket::Coinm => &self.coinm_ws_base_url,
        }
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

fn failed_live_check(request: &CredentialValidationRequest) -> ExchangeCredentialCheck {
    ExchangeCredentialCheck {
        selected_markets: request.selected_markets.clone(),
        api_connectivity_ok: false,
        timestamp_in_sync: false,
        can_read_spot: false,
        can_read_usdm: false,
        can_read_coinm: false,
        hedge_mode_ok: !request
            .selected_markets
            .iter()
            .any(|market| market == "usdm" || market == "coinm"),
        permissions_ok: false,
        withdrawal_disabled: false,
        market_access_ok: false,
    }
}

fn spot_balance_locked_total(balances: &[SpotAccountBalance]) -> String {
    balances
        .iter()
        .filter_map(|balance| flexible_value_ref_to_decimal(&balance.locked))
        .fold(Decimal::ZERO, |acc, value| acc + value)
        .normalize()
        .to_string()
}

fn futures_balance_locked_total(assets: &[FuturesAssetBalance]) -> String {
    assets
        .iter()
        .filter_map(|asset| asset.unrealized_profit.as_ref())
        .filter_map(flexible_value_ref_to_decimal)
        .map(|value| value.abs())
        .fold(Decimal::ZERO, |acc, value| acc + value)
        .normalize()
        .to_string()
}

fn flexible_value_ref_to_decimal(value: &FlexibleValue) -> Option<Decimal> {
    match value {
        FlexibleValue::Text(text) => text.parse::<Decimal>().ok(),
        FlexibleValue::Integer(number) => Some(Decimal::from(*number)),
        FlexibleValue::Float(number) => Decimal::from_str_exact(&number.to_string()).ok(),
    }
}

fn spot_balances(balances: &[SpotAccountBalance]) -> BTreeMap<String, String> {
    balances
        .iter()
        .filter_map(|balance| {
            let total = add_decimal_strings(
                &flexible_value_ref_to_string(&balance.free),
                &flexible_value_ref_to_string(&balance.locked),
            )
            .ok()?;
            (total != Decimal::ZERO).then(|| (balance.asset.clone(), total.normalize().to_string()))
        })
        .collect()
}

fn futures_balances(balances: &[FuturesAssetBalance]) -> BTreeMap<String, String> {
    balances
        .iter()
        .filter_map(|balance| {
            let value =
                parse_decimal_str(&flexible_value_ref_to_string(&balance.wallet_balance)).ok()?;
            (value != Decimal::ZERO).then(|| (balance.asset.clone(), value.normalize().to_string()))
        })
        .collect()
}

fn add_decimal_strings(left: &str, right: &str) -> Result<Decimal, CredentialValidationError> {
    Ok(parse_decimal_str(left)? + parse_decimal_str(right)?)
}

fn parse_decimal_str(value: &str) -> Result<Decimal, CredentialValidationError> {
    value.parse::<Decimal>().map_err(|error| {
        CredentialValidationError::new(format!("invalid decimal payload: {error}"))
    })
}

fn current_timestamp_ms() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as i64
}

fn parse_json_response<T: DeserializeOwned>(
    response: reqwest::blocking::Response,
) -> Result<T, CredentialValidationError> {
    response
        .error_for_status()
        .map_err(|error| {
            CredentialValidationError::new(format!("binance request failed: {error}"))
        })?
        .json::<T>()
        .map_err(|error| {
            CredentialValidationError::new(format!("binance response decode failed: {error}"))
        })
}

fn flexible_value_ref_to_string(value: &FlexibleValue) -> String {
    match value {
        FlexibleValue::Text(value) => value.clone(),
        FlexibleValue::Integer(value) => value.to_string(),
        FlexibleValue::Float(value) => {
            let mut text = value.to_string();
            if text.contains('.') {
                while text.ends_with('0') {
                    text.pop();
                }
                if text.ends_with('.') {
                    text.pop();
                }
            }
            text
        }
    }
}

pub fn parse_user_data_message(
    default_market: &str,
    payload: &str,
) -> Option<BinanceExecutionUpdate> {
    if let Ok(spot) = serde_json::from_str::<SpotExecutionReportPayload>(payload) {
        if spot.event_type == "executionReport" {
            return Some(BinanceExecutionUpdate {
                market: default_market.to_string(),
                symbol: spot.symbol,
                order_id: flexible_identifier_to_string(spot.order_id),
                client_order_id: Some(spot.client_order_id),
                side: Some(spot.side),
                order_type: Some(spot.order_type),
                status: spot.status,
                execution_type: Some(spot.execution_type),
                order_price: Some(flexible_scalar_to_string(spot.order_price)),
                last_fill_price: spot.last_fill_price.map(flexible_scalar_to_string),
                last_fill_quantity: spot.last_fill_quantity.map(flexible_scalar_to_string),
                cumulative_fill_quantity: spot
                    .cumulative_fill_quantity
                    .map(flexible_scalar_to_string),
                fee_amount: spot.fee_amount.map(flexible_scalar_to_string),
                fee_asset: spot.fee_asset,
                position_side: None,
                trade_id: spot.trade_id.map(flexible_identifier_to_string),
                realized_profit: None,
                event_time_ms: spot.event_time_ms,
            });
        }
    }
    if let Ok(futures) = serde_json::from_str::<FuturesOrderTradeUpdateEnvelope>(payload) {
        if futures.event_type == "ORDER_TRADE_UPDATE" {
            return Some(BinanceExecutionUpdate {
                market: default_market.to_string(),
                symbol: futures.order.symbol,
                order_id: flexible_identifier_to_string(futures.order.order_id),
                client_order_id: Some(futures.order.client_order_id),
                side: Some(futures.order.side),
                order_type: Some(futures.order.order_type),
                status: futures.order.status,
                execution_type: Some(futures.order.execution_type),
                order_price: Some(flexible_scalar_to_string(futures.order.order_price)),
                last_fill_price: futures.order.last_fill_price.map(flexible_scalar_to_string),
                last_fill_quantity: futures
                    .order
                    .last_fill_quantity
                    .map(flexible_scalar_to_string),
                cumulative_fill_quantity: futures
                    .order
                    .cumulative_fill_quantity
                    .map(flexible_scalar_to_string),
                fee_amount: futures.order.fee_amount.map(flexible_scalar_to_string),
                fee_asset: futures.order.fee_asset,
                position_side: futures.order.position_side,
                trade_id: futures.order.trade_id.map(flexible_identifier_to_string),
                realized_profit: futures.order.realized_profit.map(flexible_scalar_to_string),
                event_time_ms: futures.event_time_ms,
            });
        }
    }
    None
}

fn flexible_identifier_to_string(value: FlexibleIdentifier) -> String {
    match value {
        FlexibleIdentifier::Text(value) => value,
        FlexibleIdentifier::Integer(value) => value.to_string(),
        FlexibleIdentifier::Unsigned(value) => value.to_string(),
    }
}

fn flexible_scalar_to_string(value: FlexibleValue) -> String {
    match value {
        FlexibleValue::Text(value) => value,
        FlexibleValue::Integer(value) => value.to_string(),
        FlexibleValue::Float(value) => {
            let mut text = value.to_string();
            if text.contains('.') {
                while text.ends_with('0') {
                    text.pop();
                }
                if text.ends_with('.') {
                    text.pop();
                }
            }
            text
        }
    }
}

fn sign_query(secret: &str, query: &str) -> Result<String, CredentialValidationError> {
    let mut mac = <HmacSha256 as Mac>::new_from_slice(secret.as_bytes())
        .map_err(|error| CredentialValidationError::new(format!("invalid api secret: {error}")))?;
    mac.update(query.as_bytes());
    Ok(hex::encode(mac.finalize().into_bytes()))
}

fn env_flag(name: &str) -> bool {
    env::var(name)
        .ok()
        .map(|value| value.trim().to_ascii_lowercase())
        .map(|value| matches!(value.as_str(), "1" | "true" | "yes" | "on" | "live"))
        .unwrap_or(false)
}

fn env_url(name: &str, default: &str) -> String {
    env::var(name)
        .ok()
        .map(|value| value.trim().trim_end_matches('/').to_owned())
        .filter(|value| !value.is_empty())
        .unwrap_or_else(|| default.to_owned())
}

fn timestamp_is_in_sync(server_time: i64) -> bool {
    let local_now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as i64;
    (local_now - server_time).abs() <= MAX_TIMESTAMP_SKEW_MS
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

fn map_exchange_info_symbol(market: BinanceMarket, symbol: ExchangeInfoSymbol) -> SymbolMetadata {
    let filters = filters_from_exchange_info(&symbol.filters, symbol.contract_size);
    let keywords =
        keywords_for_market(market, &symbol.permissions, symbol.contract_type.as_deref());
    let market_requirements = match market {
        BinanceMarket::Spot => symbol_requirements(false, false, false, false, &[]),
        BinanceMarket::Usdm | BinanceMarket::Coinm => {
            symbol_requirements(true, true, true, true, &[])
        }
    };

    SymbolMetadata::new(
        symbol.symbol,
        market.as_str(),
        symbol.status,
        symbol.base_asset,
        symbol.quote_asset,
        symbol
            .price_precision
            .or(symbol.quote_precision)
            .unwrap_or_default(),
        symbol
            .quantity_precision
            .or(symbol.base_asset_precision)
            .unwrap_or_default(),
        filters,
        market_requirements,
        keywords,
    )
}

fn filters_from_exchange_info(
    filters: &[ExchangeFilter],
    contract_size: Option<FlexibleValue>,
) -> crate::metadata::SymbolFilters {
    let mut price_tick_size = "0".to_owned();
    let mut quantity_step_size = "0".to_owned();
    let mut min_quantity = "0".to_owned();
    let mut min_notional = "0".to_owned();

    for filter in filters {
        match filter.filter_type.as_str() {
            "PRICE_FILTER" => {
                if let Some(value) = flexible_value_to_string(filter.tick_size.as_ref()) {
                    price_tick_size = value;
                }
            }
            "LOT_SIZE" => {
                if let Some(value) = flexible_value_to_string(filter.step_size.as_ref()) {
                    quantity_step_size = value;
                }
                if let Some(value) = flexible_value_to_string(filter.min_qty.as_ref()) {
                    min_quantity = value;
                }
            }
            "MIN_NOTIONAL" | "NOTIONAL" => {
                if let Some(value) = flexible_value_to_string(filter.min_notional.as_ref())
                    .or_else(|| flexible_value_to_string(filter.notional.as_ref()))
                {
                    min_notional = value;
                }
            }
            _ => {}
        }
    }

    crate::metadata::SymbolFilters {
        price_tick_size,
        quantity_step_size,
        min_quantity,
        min_notional,
        contract_size: flexible_value_to_string(contract_size.as_ref()),
    }
}

fn keywords_for_market(
    market: BinanceMarket,
    permissions: &[String],
    contract_type: Option<&str>,
) -> Vec<String> {
    let mut keywords = Vec::new();
    match market {
        BinanceMarket::Spot => keywords.extend(["spot", "cash", "exchange"].map(str::to_owned)),
        BinanceMarket::Usdm => keywords.extend(["futures", "linear", "usdm"].map(str::to_owned)),
        BinanceMarket::Coinm => keywords.extend(["futures", "inverse", "coinm"].map(str::to_owned)),
    }
    if let Some(contract_type) = contract_type {
        keywords.push(contract_type.to_ascii_lowercase());
    }
    keywords.extend(
        permissions
            .iter()
            .map(|permission| permission.to_ascii_lowercase()),
    );
    keywords.sort();
    keywords.dedup();
    keywords
}

fn flexible_value_to_string(value: Option<&FlexibleValue>) -> Option<String> {
    value.map(|value| match value {
        FlexibleValue::Text(text) => text.clone(),
        FlexibleValue::Integer(number) => number.to_string(),
        FlexibleValue::Float(number) => {
            let mut text = number.to_string();
            if text.contains('.') {
                while text.ends_with('0') {
                    text.pop();
                }
                if text.ends_with('.') {
                    text.push('0');
                }
            }
            text
        }
    })
}

fn offline_spot_symbols() -> Vec<SymbolMetadata> {
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

fn offline_usdm_symbols() -> Vec<SymbolMetadata> {
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

fn offline_coinm_symbols() -> Vec<SymbolMetadata> {
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
        parse_user_data_message, BinanceClient, BinanceOrderRequest, CredentialCipher,
        CredentialValidationError, CredentialValidationRequest,
    };
    use std::{
        collections::VecDeque,
        env,
        io::{Read, Write},
        net::TcpListener,
        sync::{Arc, Mutex, OnceLock},
        thread,
        time::{SystemTime, UNIX_EPOCH},
    };

    #[derive(Clone)]
    struct TestRoute {
        path_prefix: &'static str,
        status_line: &'static str,
        body: &'static str,
    }

    struct TestServer {
        base_url: String,
        join_handle: Option<thread::JoinHandle<()>>,
    }

    struct EnvGuard {
        key: &'static str,
        previous: Option<String>,
    }

    impl Drop for EnvGuard {
        fn drop(&mut self) {
            match &self.previous {
                Some(value) => env::set_var(self.key, value),
                None => env::remove_var(self.key),
            }
        }
    }

    impl Drop for TestServer {
        fn drop(&mut self) {
            if let Some(join_handle) = self.join_handle.take() {
                join_handle
                    .join()
                    .expect("test server thread should exit cleanly");
            }
        }
    }

    fn env_lock() -> &'static Mutex<()> {
        static LOCK: OnceLock<Mutex<()>> = OnceLock::new();
        LOCK.get_or_init(|| Mutex::new(()))
    }

    fn set_env(key: &'static str, value: impl Into<String>) -> EnvGuard {
        let previous = env::var(key).ok();
        env::set_var(key, value.into());
        EnvGuard { key, previous }
    }

    fn spawn_test_server(routes: Vec<TestRoute>) -> TestServer {
        let listener = TcpListener::bind("127.0.0.1:0").expect("bind local test server");
        let address = listener.local_addr().expect("test server address");
        let queue = Arc::new(Mutex::new(VecDeque::from(routes)));
        let queue_for_thread = queue.clone();
        let join_handle = thread::spawn(move || {
            while let Some(route) = queue_for_thread
                .lock()
                .expect("route queue poisoned")
                .pop_front()
            {
                let (mut stream, _) = listener.accept().expect("accept test request");
                let mut buffer = [0u8; 4096];
                let read = stream.read(&mut buffer).expect("read test request");
                let request = String::from_utf8_lossy(&buffer[..read]);
                let path = request
                    .lines()
                    .next()
                    .and_then(|line| line.split_whitespace().nth(1))
                    .expect("request path");
                assert!(
                    path.starts_with(route.path_prefix),
                    "expected path prefix {} but received {}",
                    route.path_prefix,
                    path
                );

                let response = format!(
                    "{}\r\ncontent-type: application/json\r\ncontent-length: {}\r\nconnection: close\r\n\r\n{}",
                    route.status_line,
                    route.body.len(),
                    route.body
                );
                stream
                    .write_all(response.as_bytes())
                    .expect("write test response");
            }
        });

        TestServer {
            base_url: format!("http://{}", address),
            join_handle: Some(join_handle),
        }
    }

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

    #[test]
    fn live_exchange_info_uses_rest_payloads_when_enabled() {
        let _guard = env_lock().lock().unwrap();
        let server = spawn_test_server(vec![
            TestRoute {
                path_prefix: "/api/v3/exchangeInfo",
                status_line: "HTTP/1.1 200 OK",
                body: r#"{
                    "symbols": [{
                        "symbol": "XRPUSDT",
                        "status": "TRADING",
                        "baseAsset": "XRP",
                        "quoteAsset": "USDT",
                        "quotePrecision": 8,
                        "baseAssetPrecision": 4,
                        "filters": [
                            {"filterType": "PRICE_FILTER", "tickSize": "0.0001"},
                            {"filterType": "LOT_SIZE", "minQty": "1", "stepSize": "1"},
                            {"filterType": "MIN_NOTIONAL", "minNotional": "5"}
                        ],
                        "permissions": ["SPOT"]
                    }]
                }"#,
            },
            TestRoute {
                path_prefix: "/fapi/v1/exchangeInfo",
                status_line: "HTTP/1.1 200 OK",
                body: r#"{
                    "symbols": [{
                        "symbol": "BTCUSDT",
                        "status": "TRADING",
                        "baseAsset": "BTC",
                        "quoteAsset": "USDT",
                        "pricePrecision": 2,
                        "quantityPrecision": 3,
                        "filters": [
                            {"filterType": "PRICE_FILTER", "tickSize": "0.10"},
                            {"filterType": "LOT_SIZE", "minQty": "0.001", "stepSize": "0.001"},
                            {"filterType": "MIN_NOTIONAL", "notional": "100"}
                        ],
                        "contractType": "PERPETUAL"
                    }]
                }"#,
            },
            TestRoute {
                path_prefix: "/dapi/v1/exchangeInfo",
                status_line: "HTTP/1.1 200 OK",
                body: r#"{
                    "symbols": [{
                        "symbol": "BTCUSD_PERP",
                        "status": "TRADING",
                        "baseAsset": "BTC",
                        "quoteAsset": "USD",
                        "pricePrecision": 1,
                        "quantityPrecision": 0,
                        "contractSize": "100",
                        "filters": [
                            {"filterType": "PRICE_FILTER", "tickSize": "0.1"},
                            {"filterType": "LOT_SIZE", "minQty": "1", "stepSize": "1"},
                            {"filterType": "MIN_NOTIONAL", "notional": "100"}
                        ],
                        "contractType": "PERPETUAL"
                    }]
                }"#,
            },
        ]);
        let _live_mode = set_env("BINANCE_LIVE_MODE", "1");
        let _spot_base = set_env("BINANCE_SPOT_REST_BASE_URL", &server.base_url);
        let _usdm_base = set_env("BINANCE_USDM_REST_BASE_URL", &server.base_url);
        let _coinm_base = set_env("BINANCE_COINM_REST_BASE_URL", &server.base_url);

        let client = BinanceClient::new("live-key", "live-secret");

        let spot = client.spot_symbols();
        let usdm = client.usdm_symbols();
        let coinm = client.coinm_symbols();

        assert_eq!(spot.len(), 1);
        assert_eq!(spot[0].symbol, "XRPUSDT");
        assert_eq!(spot[0].filters.price_tick_size, "0.0001");
        assert_eq!(spot[0].quantity_precision, 4);

        assert_eq!(usdm.len(), 1);
        assert_eq!(usdm[0].symbol, "BTCUSDT");
        assert_eq!(usdm[0].market, "usdm");
        assert_eq!(usdm[0].filters.min_notional, "100");

        assert_eq!(coinm.len(), 1);
        assert_eq!(coinm[0].symbol, "BTCUSD_PERP");
        assert_eq!(coinm[0].filters.contract_size.as_deref(), Some("100"));
    }

    #[test]
    fn live_symbol_fetch_strict_surfaces_exchange_info_failures() {
        let _guard = env_lock().lock().unwrap();
        let server = spawn_test_server(vec![TestRoute {
            path_prefix: "/api/v3/exchangeInfo",
            status_line: "HTTP/1.1 500 Internal Server Error",
            body: r#"{"code":-1,"msg":"boom"}"#,
        }]);
        let _live_mode = set_env("BINANCE_LIVE_MODE", "1");
        let _spot_base = set_env("BINANCE_SPOT_REST_BASE_URL", &server.base_url);

        let client = BinanceClient::new("live-key", "live-secret");

        assert!(client.spot_symbols_strict().is_err());
    }

    #[test]
    fn live_order_endpoints_submit_and_cancel_spot_orders() {
        let _guard = env_lock().lock().unwrap();
        let server = spawn_test_server(vec![
            TestRoute {
                path_prefix: "/api/v3/time",
                status_line: "HTTP/1.1 200 OK",
                body: r#"{"serverTime": 1710000000000}"#,
            },
            TestRoute {
                path_prefix: "/api/v3/order?",
                status_line: "HTTP/1.1 200 OK",
                body: r#"{"symbol":"BTCUSDT","orderId":98765,"clientOrderId":"grid-order-1","status":"NEW","price":"42000","origQty":"0.001","side":"BUY","type":"LIMIT"}"#,
            },
            TestRoute {
                path_prefix: "/api/v3/time",
                status_line: "HTTP/1.1 200 OK",
                body: r#"{"serverTime": 1710000000001}"#,
            },
            TestRoute {
                path_prefix: "/api/v3/order?",
                status_line: "HTTP/1.1 200 OK",
                body: r#"{"symbol":"BTCUSDT","orderId":98765,"clientOrderId":"grid-order-1","status":"CANCELED","price":"42000","origQty":"0.001","side":"BUY","type":"LIMIT"}"#,
            },
        ]);
        let _live_mode = set_env("BINANCE_LIVE_MODE", "1");
        let _spot_base = set_env("BINANCE_SPOT_REST_BASE_URL", &server.base_url);

        let client = BinanceClient::new("live-key", "live-secret");
        let created = client
            .place_order(BinanceOrderRequest {
                market: "spot".to_string(),
                symbol: "BTCUSDT".to_string(),
                side: "BUY".to_string(),
                order_type: "LIMIT".to_string(),
                quantity: "0.001".to_string(),
                price: Some("42000".to_string()),
                time_in_force: Some("GTC".to_string()),
                reduce_only: None,
                position_side: None,
                client_order_id: Some("grid-order-1".to_string()),
            })
            .expect("place order");
        assert_eq!(created.order_id, "98765");
        assert_eq!(created.status, "NEW");
        assert_eq!(created.client_order_id.as_deref(), Some("grid-order-1"));

        let canceled = client
            .cancel_order("spot", "BTCUSDT", Some("98765"), Some("grid-order-1"))
            .expect("cancel order");
        assert_eq!(canceled.order_id, "98765");
        assert_eq!(canceled.status, "CANCELED");
    }

    #[test]
    fn live_snapshot_bundle_reads_spot_and_usdm_accounts() {
        let _guard = env_lock().lock().unwrap();
        let server = spawn_test_server(vec![
            TestRoute {
                path_prefix: "/api/v3/account?",
                status_line: "HTTP/1.1 200 OK",
                body: r#"{"canTrade":true,"canWithdraw":false,"permissions":["SPOT"],"balances":[{"asset":"BTC","free":"0.01","locked":"0.00"},{"asset":"USDT","free":"120.5","locked":"0.5"}]}"#,
            },
            TestRoute {
                path_prefix: "/fapi/v2/account?",
                status_line: "HTTP/1.1 200 OK",
                body: r#"{"totalWalletBalance":"200.5","totalUnrealizedProfit":"3.25","assets":[{"asset":"USDT","walletBalance":"200.5","unrealizedProfit":"3.25"}]}"#,
            },
        ]);
        let _live_mode = set_env("BINANCE_LIVE_MODE", "1");
        let _spot_base = set_env("BINANCE_SPOT_REST_BASE_URL", &server.base_url);
        let _usdm_base = set_env("BINANCE_USDM_REST_BASE_URL", &server.base_url);

        let client = BinanceClient::new("live-key", "live-secret");
        let bundle = client
            .snapshot_bundle(&["spot".to_string(), "usdm".to_string()])
            .expect("snapshot bundle");

        assert_eq!(bundle.account_snapshots.len(), 2);
        assert_eq!(bundle.wallet_snapshots.len(), 2);
        assert_eq!(bundle.account_snapshots[0].exchange, "binance");
        assert_eq!(bundle.account_snapshots[0].realized_pnl, "0");
        assert_eq!(bundle.wallet_snapshots[0].wallet_type, "spot");
        assert_eq!(bundle.wallet_snapshots[0].balances["BTC"], "0.01");
        assert_eq!(bundle.account_snapshots[1].exchange, "binance-usdm");
        assert_eq!(bundle.account_snapshots[1].unrealized_pnl, "3.25");
        assert_eq!(bundle.wallet_snapshots[1].balances["USDT"], "200.5");
    }

    #[test]
    fn live_user_data_stream_returns_listen_key_and_ws_url() {
        let _guard = env_lock().lock().unwrap();
        let server = spawn_test_server(vec![TestRoute {
            path_prefix: "/api/v3/userDataStream",
            status_line: "HTTP/1.1 200 OK",
            body: r#"{"listenKey":"spot-key-123"}"#,
        }]);
        let _live_mode = set_env("BINANCE_LIVE_MODE", "1");
        let _spot_base = set_env("BINANCE_SPOT_REST_BASE_URL", &server.base_url);
        let _spot_ws = set_env(
            "BINANCE_SPOT_WS_BASE_URL",
            "wss://stream.binance.com:9443/ws",
        );

        let client = BinanceClient::new("live-key", "live-secret");
        let stream = client.start_user_data_stream("spot").expect("listen key");

        assert_eq!(stream.market, "spot");
        assert_eq!(stream.listen_key, "spot-key-123");
        assert_eq!(
            stream.websocket_url,
            "wss://stream.binance.com:9443/ws/spot-key-123"
        );
    }

    #[test]
    fn parse_user_data_message_supports_spot_and_futures_payloads() {
        let spot = parse_user_data_message(
            "spot",
            r#"{"e":"executionReport","E":1710000,"s":"BTCUSDT","c":"grid-order-1","S":"BUY","o":"LIMIT","x":"TRADE","X":"FILLED","i":555,"p":"42000","L":"42000","l":"0.001","z":"0.001","n":"0.05","N":"USDT","t":-1}"#,
        )
        .expect("spot execution report");
        assert_eq!(spot.market, "spot");
        assert_eq!(spot.order_id, "555");
        assert_eq!(spot.status, "FILLED");
        assert_eq!(spot.last_fill_quantity.as_deref(), Some("0.001"));
        assert_eq!(spot.trade_id.as_deref(), Some("-1"));

        let futures = parse_user_data_message(
            "usdm",
            r#"{"e":"ORDER_TRADE_UPDATE","E":1710001,"o":{"s":"BTCUSDT","c":"grid-order-2","S":"SELL","o":"LIMIT","x":"TRADE","X":"PARTIALLY_FILLED","i":777,"p":"43000","L":"43000","l":"0.002","z":"0.003","n":"0.06","N":"USDT","ps":"SHORT","t":0,"rp":"1.25"}}"#,
        )
        .expect("futures order trade update");
        assert_eq!(futures.market, "usdm");
        assert_eq!(futures.order_id, "777");
        assert_eq!(futures.position_side.as_deref(), Some("SHORT"));
        assert_eq!(futures.execution_type.as_deref(), Some("TRADE"));
        assert_eq!(futures.trade_id.as_deref(), Some("0"));
    }

    #[test]
    fn live_keepalive_user_data_stream_reuses_listen_key() {
        let _guard = env_lock().lock().unwrap();
        let server = spawn_test_server(vec![TestRoute {
            path_prefix: "/api/v3/userDataStream",
            status_line: "HTTP/1.1 200 OK",
            body: r#"{}"#,
        }]);
        let _live_mode = set_env("BINANCE_LIVE_MODE", "1");
        let _spot_base = set_env("BINANCE_SPOT_REST_BASE_URL", &server.base_url);

        let client = BinanceClient::new("live-key", "live-secret");
        client
            .keepalive_user_data_stream("spot", "spot-key-123")
            .expect("keepalive");
    }

    #[test]
    fn live_get_order_reads_spot_order_status() {
        let _guard = env_lock().lock().unwrap();
        let server = spawn_test_server(vec![
            TestRoute {
                path_prefix: "/api/v3/time",
                status_line: "HTTP/1.1 200 OK",
                body: r#"{"serverTime": 1710000000003}"#,
            },
            TestRoute {
                path_prefix: "/api/v3/order?",
                status_line: "HTTP/1.1 200 OK",
                body: r#"{"symbol":"BTCUSDT","orderId":555,"clientOrderId":"grid-order-1","status":"CANCELED","price":"42000","origQty":"0.001","side":"BUY","type":"LIMIT"}"#,
            },
        ]);
        let _live_mode = set_env("BINANCE_LIVE_MODE", "1");
        let _spot_base = set_env("BINANCE_SPOT_REST_BASE_URL", &server.base_url);

        let client = BinanceClient::new("live-key", "live-secret");
        let order = client
            .get_order("spot", "BTCUSDT", Some("555"), Some("grid-order-1"))
            .expect("get order");

        assert_eq!(order.order_id, "555");
        assert_eq!(order.status, "CANCELED");
    }

    #[test]
    fn live_trade_history_reads_spot_my_trades() {
        let _guard = env_lock().lock().unwrap();
        let server = spawn_test_server(vec![
            TestRoute {
                path_prefix: "/api/v3/time",
                status_line: "HTTP/1.1 200 OK",
                body: r#"{"serverTime": 1710000000002}"#,
            },
            TestRoute {
                path_prefix: "/api/v3/myTrades?",
                status_line: "HTTP/1.1 200 OK",
                body: r#"[{"id":1001,"orderId":98765,"symbol":"BTCUSDT","price":"42000","qty":"0.001","commission":"0.05","commissionAsset":"USDT","time":1710000000123,"isBuyer":true}]"#,
            },
        ]);
        let _live_mode = set_env("BINANCE_LIVE_MODE", "1");
        let _spot_base = set_env("BINANCE_SPOT_REST_BASE_URL", &server.base_url);

        let client = BinanceClient::new("live-key", "live-secret");
        let trades = client
            .user_trades("spot", "BTCUSDT", 10)
            .expect("load trades");

        assert_eq!(trades.len(), 1);
        assert_eq!(trades[0].trade_id, "1001");
        assert_eq!(trades[0].order_id.as_deref(), Some("98765"));
        assert_eq!(trades[0].side, "BUY");
        assert_eq!(trades[0].quantity, "0.001");
        assert_eq!(trades[0].fee_amount.as_deref(), Some("0.05"));
    }

    #[test]
    fn live_validation_checks_time_skew_hedge_mode_and_market_access() {
        let _guard = env_lock().lock().unwrap();
        let skewed_now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("time should be after epoch")
            .as_millis() as i64
            + 60_000;
        let skewed_now_payload =
            Box::leak(format!(r#"{{"serverTime": {skewed_now}}}"#).into_boxed_str());
        let server = spawn_test_server(vec![
            TestRoute {
                path_prefix: "/api/v3/time",
                status_line: "HTTP/1.1 200 OK",
                body: skewed_now_payload,
            },
            TestRoute {
                path_prefix: "/api/v3/account?",
                status_line: "HTTP/1.1 200 OK",
                body: r#"{"canTrade": true, "canWithdraw": false, "permissions": ["SPOT"]}"#,
            },
            TestRoute {
                path_prefix: "/fapi/v1/time",
                status_line: "HTTP/1.1 200 OK",
                body: skewed_now_payload,
            },
            TestRoute {
                path_prefix: "/fapi/v2/account?",
                status_line: "HTTP/1.1 200 OK",
                body: r#"{"canTrade": true}"#,
            },
            TestRoute {
                path_prefix: "/fapi/v1/positionSide/dual?",
                status_line: "HTTP/1.1 200 OK",
                body: r#"{"dualSidePosition": false}"#,
            },
            TestRoute {
                path_prefix: "/dapi/v1/time",
                status_line: "HTTP/1.1 200 OK",
                body: skewed_now_payload,
            },
            TestRoute {
                path_prefix: "/dapi/v1/account?",
                status_line: "HTTP/1.1 403 Forbidden",
                body: r#"{"code": -2015, "msg": "Invalid API-key, IP, or permissions for action."}"#,
            },
        ]);
        let _live_mode = set_env("BINANCE_LIVE_MODE", "1");
        let _spot_base = set_env("BINANCE_SPOT_REST_BASE_URL", &server.base_url);
        let _usdm_base = set_env("BINANCE_USDM_REST_BASE_URL", &server.base_url);
        let _coinm_base = set_env("BINANCE_COINM_REST_BASE_URL", &server.base_url);

        let client = BinanceClient::new("live-key", "live-secret");
        let request = CredentialValidationRequest::new(
            true,
            &["spot".to_owned(), "usdm".to_owned(), "coinm".to_owned()],
        )
        .expect("request");

        let check = client.check_credentials_for(&request);

        assert!(check.api_connectivity_ok);
        assert!(!check.timestamp_in_sync);
        assert!(check.can_read_spot);
        assert!(check.can_read_usdm);
        assert!(!check.can_read_coinm);
        assert!(!check.hedge_mode_ok);
        assert!(check.permissions_ok);
        assert!(!check.market_access_ok);
        assert_eq!(check.connection_status(), "degraded");
    }
}
