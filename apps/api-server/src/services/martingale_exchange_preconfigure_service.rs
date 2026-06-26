use chrono::Utc;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use shared_binance::{BinanceClient, CredentialCipher};
use shared_db::{MartingalePortfolioRecord, SharedDb, SharedDbError};
use std::collections::BTreeMap;

const BINANCE_EXCHANGE: &str = "binance";

#[derive(Debug, Clone, Deserialize)]
pub struct ExchangePreconfigureRequest {
    pub confirm_account_level_hedge_mode_change: bool,
    #[serde(default)]
    pub confirm_account_level_multi_assets_mode_change: bool,
    pub confirm_no_auto_orders: bool,
    pub confirm_symbol_margin_leverage_change: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExchangePreconfigureResponse {
    pub status: String,
    pub hedge_mode: HedgeModeCheck,
    pub multi_assets_mode: MultiAssetsModeCheck,
    pub blocked_symbols: Vec<String>,
    pub open_order_count: usize,
    pub nonzero_position_count: usize,
    pub symbols: Vec<SymbolExchangeCheck>,
    pub warnings: Vec<String>,
    pub checked_at: String,
    pub applied: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HedgeModeCheck {
    pub target: bool,
    pub current: Option<bool>,
    pub status: String,
    pub message: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MultiAssetsModeCheck {
    pub target: bool,
    pub current: Option<bool>,
    pub status: String,
    pub message: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SymbolExchangeCheck {
    pub symbol: String,
    pub target_margin_mode: String,
    pub current_margin_mode: Option<String>,
    pub target_leverage: u32,
    pub current_leverage: Option<u32>,
    pub status: String,
    pub message: String,
}

#[derive(Debug, Clone)]
pub struct TargetExchangeSettings {
    pub requires_hedge_mode: bool,
    pub requires_single_asset_mode: bool,
    pub symbols: BTreeMap<String, TargetSymbolSettings>,
}

#[derive(Debug, Clone)]
pub struct TargetSymbolSettings {
    pub margin_mode: String,
    pub leverage: u32,
}

pub fn validate_preconfigure_confirmations(
    portfolio: &MartingalePortfolioRecord,
    request: &ExchangePreconfigureRequest,
) -> Result<(), SharedDbError> {
    validate_preconfigurable_status(portfolio)?;
    let target = target_exchange_settings_from_portfolio(portfolio)?;
    if target.requires_hedge_mode && !request.confirm_account_level_hedge_mode_change {
        return Err(SharedDbError::new(
            "account-level Hedge Mode confirmation is required",
        ));
    }
    if target.requires_single_asset_mode && !request.confirm_account_level_multi_assets_mode_change
    {
        return Err(SharedDbError::new(
            "account-level Multi-Assets mode confirmation is required",
        ));
    }
    if !request.confirm_no_auto_orders {
        return Err(SharedDbError::new(
            "no-auto-orders confirmation is required",
        ));
    }
    if !target.symbols.is_empty() && !request.confirm_symbol_margin_leverage_change {
        return Err(SharedDbError::new(
            "symbol margin/leverage confirmation is required",
        ));
    }
    Ok(())
}

pub fn validate_preconfigurable_status(
    portfolio: &MartingalePortfolioRecord,
) -> Result<(), SharedDbError> {
    if matches!(portfolio.status.as_str(), "pending_confirmation" | "paused") {
        Ok(())
    } else {
        Err(SharedDbError::new(format!(
            "portfolio status {} cannot be exchange-preconfigured; pause it or use pending_confirmation",
            portfolio.status
        )))
    }
}

pub fn target_exchange_settings_from_portfolio(
    portfolio: &MartingalePortfolioRecord,
) -> Result<TargetExchangeSettings, SharedDbError> {
    let strategies = portfolio
        .config
        .get("portfolio_config")
        .and_then(|config| config.get("strategies"))
        .and_then(Value::as_array)
        .ok_or_else(|| SharedDbError::new("portfolio_config.strategies is required"))?;
    let mut symbols = BTreeMap::<String, TargetSymbolSettings>::new();
    let mut has_long = false;
    let mut has_short = false;
    let mut requires_single_asset_mode = false;
    for strategy in strategies {
        if strategy.get("market").and_then(Value::as_str) != Some("usd_m_futures") {
            continue;
        }
        let symbol = strategy
            .get("symbol")
            .and_then(Value::as_str)
            .ok_or_else(|| SharedDbError::new("strategy symbol is required"))?
            .trim()
            .to_uppercase();
        let direction = strategy
            .get("direction")
            .and_then(Value::as_str)
            .unwrap_or("");
        has_long |= direction == "long";
        has_short |= direction == "short";
        let margin_mode = strategy
            .get("margin_mode")
            .and_then(Value::as_str)
            .unwrap_or("isolated")
            .to_ascii_lowercase();
        if !matches!(margin_mode.as_str(), "isolated" | "cross" | "crossed") {
            return Err(SharedDbError::new(format!(
                "{symbol} margin mode must be isolated, cross, or crossed"
            )));
        }
        let normalized_margin_mode = normalize_margin_mode(&margin_mode);
        requires_single_asset_mode |= normalized_margin_mode == "isolated";
        let leverage = strategy
            .get("leverage")
            .and_then(Value::as_u64)
            .ok_or_else(|| SharedDbError::new(format!("{symbol} leverage is required")))?
            as u32;
        if !(1..=125).contains(&leverage) {
            return Err(SharedDbError::new(format!(
                "{symbol} leverage must be between 1 and 125"
            )));
        }
        if let Some(existing) = symbols.get(&symbol) {
            if normalize_margin_mode(&existing.margin_mode) != normalized_margin_mode {
                return Err(SharedDbError::new(format!("{symbol} margin mode conflict")));
            }
            if existing.leverage < leverage {
                symbols.insert(
                    symbol,
                    TargetSymbolSettings {
                        margin_mode: normalized_margin_mode,
                        leverage,
                    },
                );
            }
        } else {
            symbols.insert(
                symbol,
                TargetSymbolSettings {
                    margin_mode: normalized_margin_mode,
                    leverage,
                },
            );
        }
    }
    Ok(TargetExchangeSettings {
        requires_hedge_mode: portfolio.direction == "long_short" || has_long && has_short,
        requires_single_asset_mode,
        symbols,
    })
}

pub fn response_from_target_without_exchange_readback(
    target: TargetExchangeSettings,
    status: &str,
    message: &str,
) -> ExchangePreconfigureResponse {
    let blocked: Vec<String> = target.symbols.keys().cloned().collect();
    let symbol_checks: Vec<SymbolExchangeCheck> = target
        .symbols
        .into_iter()
        .map(|(symbol, settings)| SymbolExchangeCheck {
            symbol,
            target_margin_mode: settings.margin_mode,
            current_margin_mode: None,
            target_leverage: settings.leverage,
            current_leverage: None,
            status: "unknown".to_owned(),
            message: message.to_owned(),
        })
        .collect();
    ExchangePreconfigureResponse {
        status: status.to_owned(),
        hedge_mode: HedgeModeCheck {
            target: target.requires_hedge_mode,
            current: None,
            status: "unknown".to_owned(),
            message: message.to_owned(),
        },
        multi_assets_mode: MultiAssetsModeCheck {
            target: !target.requires_single_asset_mode,
            current: None,
            status: "unknown".to_owned(),
            message: message.to_owned(),
        },
        blocked_symbols: blocked,
        open_order_count: 0,
        nonzero_position_count: 0,
        symbols: symbol_checks,
        warnings: vec![
            "exchange readback is required before reporting Binance settings as ready".to_owned(),
            message.to_owned(),
        ],
        checked_at: Utc::now().to_rfc3339(),
        applied: false,
    }
}

pub fn preflight_exchange_settings(
    db: &SharedDb,
    owner: &str,
    portfolio: &MartingalePortfolioRecord,
) -> Result<ExchangePreconfigureResponse, SharedDbError> {
    validate_preconfigurable_status(portfolio)?;
    let target = target_exchange_settings_from_portfolio(portfolio)?;
    let client = binance_client_for_owner(db, owner)?;
    let blockers = check_live_state_blockers(&client, &target)?;
    if !blockers.is_empty() {
        let blocked_response = build_blocked_response(target.clone(), &blockers)?;
        persist_exchange_preconfigure_summary(db, portfolio, &blocked_response)?;
        return Ok(blocked_response);
    }
    let response = readback_response(&client, target, false)?;
    persist_exchange_preconfigure_summary(db, portfolio, &response)?;
    Ok(response)
}

pub fn apply_exchange_preconfigure(
    db: &SharedDb,
    owner: &str,
    portfolio: &MartingalePortfolioRecord,
    request: &ExchangePreconfigureRequest,
) -> Result<ExchangePreconfigureResponse, SharedDbError> {
    validate_preconfigure_confirmations(portfolio, request)?;
    let target = target_exchange_settings_from_portfolio(portfolio)?;
    let client = binance_client_for_owner(db, owner)?;
    // Block if any target symbol has open orders or non-zero positions.
    let blockers = check_live_state_blockers(&client, &target)?;
    if !blockers.is_empty() {
        let blocked_response = build_blocked_response(target.clone(), &blockers)?;
        persist_exchange_preconfigure_summary(db, portfolio, &blocked_response)?;
        return Ok(blocked_response);
    }
    let before = readback_response(&client, target.clone(), false)?;
    let requires_hedge = target.requires_hedge_mode;
    if requires_hedge && before.hedge_mode.current != Some(true) {
        client
            .set_usdm_position_mode(true)
            .map_err(|error| SharedDbError::new(error.to_string()))?;
    }
    if target.requires_single_asset_mode && before.multi_assets_mode.current != Some(false) {
        client
            .set_usdm_multi_assets_mode(false)
            .map_err(|error| SharedDbError::new(error.to_string()))?;
    }
    for (symbol, settings) in &target.symbols {
        client
            .set_usdm_margin_type(symbol, &settings.margin_mode)
            .map_err(|error| SharedDbError::new(error.to_string()))?;
        client
            .set_usdm_leverage(symbol, settings.leverage)
            .map_err(|error| SharedDbError::new(error.to_string()))?;
    }
    let response = readback_response(&client, target, true)?;
    persist_exchange_preconfigure_summary(db, portfolio, &response)?;
    Ok(response)
}

/// Checks for open orders and non-zero positions on all target symbols.
/// Returns the set of blocked symbols with reasons.
pub(crate) fn check_live_state_blockers(
    client: &BinanceClient,
    target: &TargetExchangeSettings,
) -> Result<Vec<String>, SharedDbError> {
    let mut blocked: Vec<String> = Vec::new();

    for (symbol, _settings) in &target.symbols {
        let orders = client
            .open_orders_for_symbol("usdm", symbol)
            .map_err(|error| SharedDbError::new(error.to_string()))?;
        if !orders.is_empty() {
            blocked.push(format!(
                "{symbol}: {} open order(s) — cancel or wait for fills before changing settings",
                orders.len()
            ));
        }
    }

    let account = client
        .read_usdm_account_v3()
        .map_err(|error| SharedDbError::new(error.to_string()))?;
    for pos in &account.positions {
        let symbol_upper = pos.symbol.to_uppercase();
        if !target.symbols.contains_key(&symbol_upper) {
            continue;
        }
        let amount: f64 = pos.position_amount.parse().unwrap_or(0.0);
        if amount != 0.0 {
            blocked.push(format!(
                "{symbol_upper}: non-zero position ({amount}) — close or reduce positions before changing settings",
            ));
        }
    }

    Ok(blocked)
}

fn build_blocked_response(
    target: TargetExchangeSettings,
    blocked_symbols: &[String],
) -> Result<ExchangePreconfigureResponse, SharedDbError> {
    let open_order_count = blocked_symbols
        .iter()
        .filter(|reason| reason.contains("open order"))
        .count();
    let nonzero_position_count = blocked_symbols
        .iter()
        .filter(|reason| reason.contains("non-zero position"))
        .count();
    let num_blocked = blocked_symbols.len();
    let response = response_from_target_without_exchange_readback(
        target,
        "blocked",
        &format!(
            "{} symbol(s) blocked: {}",
            num_blocked,
            blocked_symbols.join("; ")
        ),
    );
    Ok(ExchangePreconfigureResponse {
        blocked_symbols: blocked_symbols.to_vec(),
        open_order_count,
        nonzero_position_count,
        ..response
    })
}

#[cfg(test)]
fn preconfigure_exchange_with_client(
    portfolio: &MartingalePortfolioRecord,
    request: &ExchangePreconfigureRequest,
    exchange: &mut dyn TestExchangeSettingsClient,
) -> Result<ExchangePreconfigureResponse, SharedDbError> {
    validate_preconfigure_confirmations(portfolio, request)?;
    let target = target_exchange_settings_from_portfolio(portfolio)?;

    // Block if any target symbol has open orders or non-zero positions.
    let mut blocked: Vec<String> = Vec::new();
    for (symbol, _settings) in &target.symbols {
        let orders = exchange.open_orders_for_symbol(symbol)?;
        if !orders.is_empty() {
            blocked.push(format!(
                "{symbol}: {} open order(s) — cancel or wait for fills before changing settings",
                orders.len()
            ));
        }
    }
    for pos in exchange.read_positions()? {
        let symbol_upper = pos.symbol.to_uppercase();
        if !target.symbols.contains_key(&symbol_upper) {
            continue;
        }
        let amount: f64 = pos.position_amount.parse().unwrap_or(0.0);
        if amount != 0.0 {
            blocked.push(format!(
                "{symbol_upper}: non-zero position ({amount}) — close or reduce positions before changing settings",
            ));
        }
    }
    if !blocked.is_empty() {
        let blocked_detail = blocked.join("; ");
        let blocked_msg = format!(
            "{} symbol(s) have open orders or non-zero positions: {}",
            blocked.len(),
            blocked_detail
        );
        let response =
            response_from_target_without_exchange_readback(target.clone(), "blocked", &blocked_msg);
        return Ok(ExchangePreconfigureResponse {
            blocked_symbols: blocked,
            ..response
        });
    }

    let before_hedge = exchange.read_usdm_hedge_mode()?;
    let before_multi_assets = exchange.read_usdm_multi_assets_mode()?;
    let requires_hedge = target.requires_hedge_mode;
    if requires_hedge && !before_hedge {
        exchange.set_usdm_position_mode(true)?;
    }
    if target.requires_single_asset_mode && before_multi_assets {
        exchange.set_usdm_multi_assets_mode(false)?;
    }
    for (symbol, settings) in &target.symbols {
        exchange.set_usdm_margin_type(symbol, &settings.margin_mode)?;
        exchange.set_usdm_leverage(symbol, settings.leverage)?;
    }
    let current_hedge = exchange.read_usdm_hedge_mode()?;
    let current_multi_assets = exchange.read_usdm_multi_assets_mode()?;
    let multi_assets_target = if target.requires_single_asset_mode {
        false
    } else {
        current_multi_assets
    };
    let multi_assets_status = if current_multi_assets == multi_assets_target {
        "ready"
    } else {
        "mismatch"
    };
    let mut symbols = Vec::with_capacity(target.symbols.len());
    for (symbol, settings) in target.symbols {
        let current = exchange.read_usdm_symbol_settings(&symbol)?;
        symbols.push(SymbolExchangeCheck {
            symbol: symbol.clone(),
            target_margin_mode: settings.margin_mode.clone(),
            current_margin_mode: Some(current.margin_mode),
            target_leverage: settings.leverage,
            current_leverage: Some(current.leverage),
            status: "ready".to_owned(),
            message: "symbol margin mode and leverage match target".to_owned(),
        });
    }
    Ok(ExchangePreconfigureResponse {
        status: "ready".to_owned(),
        hedge_mode: HedgeModeCheck {
            target: true,
            current: Some(current_hedge),
            status: "ready".to_owned(),
            message: "account position mode matches target".to_owned(),
        },
        multi_assets_mode: MultiAssetsModeCheck {
            target: multi_assets_target,
            current: Some(current_multi_assets),
            status: multi_assets_status.to_owned(),
            message: if multi_assets_status == "ready" {
                "account Multi-Assets mode is compatible with target margin settings".to_owned()
            } else {
                "account Multi-Assets mode must be disabled before isolated margin can be set"
                    .to_owned()
            },
        },
        symbols,
        warnings: vec![],
        checked_at: Utc::now().to_rfc3339(),
        applied: true,
        blocked_symbols: vec![],
        open_order_count: 0,
        nonzero_position_count: 0,
    })
}

#[cfg(test)]
#[derive(Debug, Clone)]
struct TestCurrentSymbolSettings {
    margin_mode: String,
    leverage: u32,
}

#[cfg(test)]
trait TestExchangeSettingsClient {
    fn read_usdm_hedge_mode(&mut self) -> Result<bool, SharedDbError>;
    fn set_usdm_position_mode(&mut self, dual_side_position: bool) -> Result<(), SharedDbError>;
    fn read_usdm_multi_assets_mode(&mut self) -> Result<bool, SharedDbError>;
    fn set_usdm_multi_assets_mode(
        &mut self,
        multi_assets_margin: bool,
    ) -> Result<(), SharedDbError>;
    fn set_usdm_margin_type(
        &mut self,
        symbol: &str,
        margin_mode: &str,
    ) -> Result<(), SharedDbError>;
    fn set_usdm_leverage(&mut self, symbol: &str, leverage: u32) -> Result<(), SharedDbError>;
    fn read_usdm_symbol_settings(
        &mut self,
        symbol: &str,
    ) -> Result<TestCurrentSymbolSettings, SharedDbError>;
    fn open_orders_for_symbol(&mut self, symbol: &str) -> Result<Vec<String>, SharedDbError>;
    fn read_positions(&mut self) -> Result<Vec<TestPosition>, SharedDbError>;
}

#[cfg(test)]
#[derive(Debug, Clone)]
struct TestPosition {
    pub symbol: String,
    pub position_amount: String,
}

fn readback_response(
    client: &BinanceClient,
    target: TargetExchangeSettings,
    applied: bool,
) -> Result<ExchangePreconfigureResponse, SharedDbError> {
    let current_hedge = client
        .read_usdm_position_mode()
        .map_err(|error| SharedDbError::new(error.to_string()))?;
    let current_multi_assets = client
        .read_usdm_multi_assets_mode()
        .map_err(|error| SharedDbError::new(error.to_string()))?;
    let hedge_status = if current_hedge == target.requires_hedge_mode {
        "ready"
    } else {
        "mismatch"
    };
    let multi_assets_target = if target.requires_single_asset_mode {
        false
    } else {
        current_multi_assets
    };
    let multi_assets_status = if current_multi_assets == multi_assets_target {
        "ready"
    } else {
        "mismatch"
    };
    let mut symbols = Vec::with_capacity(target.symbols.len());
    for (symbol, settings) in target.symbols {
        let current = client
            .read_usdm_symbol_settings(&symbol)
            .map_err(|error| SharedDbError::new(error.to_string()))?;
        let current_margin = current
            .margin_type
            .map(|value| normalize_margin_mode(&value));
        let margin_ok = current_margin.as_deref() == Some(settings.margin_mode.as_str());
        let leverage_ok = current.leverage == Some(settings.leverage);
        let status = if margin_ok && leverage_ok {
            "ready"
        } else {
            "mismatch"
        };
        symbols.push(SymbolExchangeCheck {
            symbol: symbol.clone(),
            target_margin_mode: settings.margin_mode.clone(),
            current_margin_mode: current_margin,
            target_leverage: settings.leverage,
            current_leverage: current.leverage,
            status: status.to_owned(),
            message: if status == "ready" {
                "symbol margin mode and leverage match target".to_owned()
            } else {
                "symbol margin mode or leverage does not match target".to_owned()
            },
        });
    }
    let ready = hedge_status == "ready"
        && multi_assets_status == "ready"
        && symbols.iter().all(|symbol| symbol.status == "ready");
    let mut warnings = vec![
        "Only Binance Futures settings are checked/applied; this endpoint never places orders, cancels orders, or closes positions.".to_owned(),
    ];
    if target.requires_hedge_mode {
        warnings.push("Hedge Mode is account-level and may affect all USDT-M Futures strategies on the Binance account.".to_owned());
    }
    if target.requires_single_asset_mode {
        warnings.push("Multi-Assets mode is account-level; isolated margin requires disabling it and this may affect all USDT-M Futures strategies on the Binance account.".to_owned());
    }
    Ok(ExchangePreconfigureResponse {
        status: if ready { "ready" } else { "mismatch" }.to_owned(),
        hedge_mode: HedgeModeCheck {
            target: target.requires_hedge_mode,
            current: Some(current_hedge),
            status: hedge_status.to_owned(),
            message: if hedge_status == "ready" {
                "account position mode matches target".to_owned()
            } else {
                "account position mode does not match target".to_owned()
            },
        },
        multi_assets_mode: MultiAssetsModeCheck {
            target: multi_assets_target,
            current: Some(current_multi_assets),
            status: multi_assets_status.to_owned(),
            message: if multi_assets_status == "ready" {
                "account Multi-Assets mode is compatible with target margin settings".to_owned()
            } else {
                "account Multi-Assets mode must be disabled before isolated margin can be set"
                    .to_owned()
            },
        },
        symbols,
        warnings,
        checked_at: Utc::now().to_rfc3339(),
        blocked_symbols: vec![],
        open_order_count: 0,
        nonzero_position_count: 0,
        applied,
    })
}

fn persist_exchange_preconfigure_summary(
    db: &SharedDb,
    portfolio: &MartingalePortfolioRecord,
    response: &ExchangePreconfigureResponse,
) -> Result<(), SharedDbError> {
    let mut risk_summary = portfolio.risk_summary.clone();
    if !risk_summary.is_object() {
        risk_summary = json!({});
    }
    if let Value::Object(map) = &mut risk_summary {
        map.insert(
            "exchange_preconfigure".to_owned(),
            serde_json::to_value(response)
                .map_err(|error| SharedDbError::new(error.to_string()))?,
        );
    }
    db.backtest_repo()
        .update_martingale_portfolio_risk_summary(
            &portfolio.owner,
            &portfolio.portfolio_id,
            risk_summary,
        )?
        .ok_or_else(|| SharedDbError::new("portfolio not found"))?;
    Ok(())
}

pub(crate) fn binance_client_for_owner(
    db: &SharedDb,
    owner: &str,
) -> Result<BinanceClient, SharedDbError> {
    let credentials = db
        .find_exchange_credentials(owner, BINANCE_EXCHANGE)?
        .ok_or_else(|| SharedDbError::new("Binance credentials are required"))?;
    let (api_key, api_secret) = CredentialCipher::from_env("EXCHANGE_CREDENTIALS_MASTER_KEY")
        .map_err(|error| SharedDbError::new(error.to_string()))?
        .decrypt(&credentials.encrypted_secret)
        .map_err(|error| SharedDbError::new(error.to_string()))?;
    Ok(BinanceClient::new(api_key, api_secret))
}

fn normalize_margin_mode(value: &str) -> String {
    match value.trim().to_ascii_lowercase().as_str() {
        "cross" | "crossed" => "crossed".to_owned(),
        _ => "isolated".to_owned(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Utc;
    use shared_db::MartingalePortfolioRecord;
    use shared_domain::strategy::Decimal;

    fn confirmed_request() -> ExchangePreconfigureRequest {
        ExchangePreconfigureRequest {
            confirm_account_level_hedge_mode_change: true,
            confirm_account_level_multi_assets_mode_change: true,
            confirm_no_auto_orders: true,
            confirm_symbol_margin_leverage_change: true,
        }
    }

    #[test]
    fn missing_confirmations_reject_preconfigure() {
        let portfolio = portfolio_fixture(
            "long_short",
            vec![
                strategy_fixture("BTCUSDT", "long", 6),
                strategy_fixture("ETHUSDT", "short", 4),
            ],
        );
        let request = ExchangePreconfigureRequest {
            confirm_account_level_hedge_mode_change: false,
            confirm_account_level_multi_assets_mode_change: true,
            confirm_no_auto_orders: true,
            confirm_symbol_margin_leverage_change: true,
        };

        let error = validate_preconfigure_confirmations(&portfolio, &request).unwrap_err();

        assert!(error
            .to_string()
            .contains("account-level Hedge Mode confirmation"));
    }

    #[test]
    fn target_settings_group_symbols_and_keep_leverage() {
        let portfolio = portfolio_fixture(
            "long_short",
            vec![
                strategy_fixture("BTCUSDT", "long", 6),
                strategy_fixture("BTCUSDT", "short", 6),
                strategy_fixture("ETHUSDT", "short", 4),
            ],
        );

        let target = target_exchange_settings_from_portfolio(&portfolio).expect("target settings");

        assert!(target.requires_hedge_mode);
        assert!(target.requires_single_asset_mode);
        assert_eq!(target.symbols.len(), 2);
        assert_eq!(target.symbols["BTCUSDT"].leverage, 6);
        assert_eq!(target.symbols["BTCUSDT"].margin_mode, "isolated");
        assert_eq!(target.symbols["ETHUSDT"].leverage, 4);
    }

    #[test]
    fn same_symbol_leverage_uses_highest_target() {
        let portfolio = portfolio_fixture(
            "long_short",
            vec![
                strategy_fixture("BTCUSDT", "long", 6),
                strategy_fixture("BTCUSDT", "short", 8),
            ],
        );

        let target = target_exchange_settings_from_portfolio(&portfolio).expect("target settings");

        assert_eq!(target.symbols["BTCUSDT"].leverage, 8);
    }

    #[test]
    fn running_portfolio_is_not_preconfigurable() {
        let mut portfolio = portfolio_fixture("long", vec![strategy_fixture("BTCUSDT", "long", 6)]);
        portfolio.status = "running".to_owned();

        let error =
            validate_preconfigure_confirmations(&portfolio, &confirmed_request()).unwrap_err();

        assert!(error
            .to_string()
            .contains("cannot be exchange-preconfigured"));
    }

    #[test]
    fn preconfigure_runs_hedge_then_margin_then_leverage_then_readback() {
        let portfolio = portfolio_fixture(
            "long_short",
            vec![
                strategy_fixture("BTCUSDT", "long", 6),
                strategy_fixture("ETHUSDT", "short", 4),
            ],
        );
        let mut exchange = FakeExchangeSettingsClient {
            hedge_mode: false,
            multi_assets_mode: true,
            calls: Vec::new(),
            open_orders: std::collections::HashMap::new(),
            positions: Vec::new(),
        };

        let response =
            preconfigure_exchange_with_client(&portfolio, &confirmed_request(), &mut exchange)
                .expect("preconfigure");

        assert_eq!(response.status, "ready");
        assert_eq!(
            exchange.calls,
            vec![
                "open_orders:BTCUSDT",
                "open_orders:ETHUSDT",
                "read_positions",
                "read_hedge",
                "read_multi_assets",
                "set_hedge:true",
                "set_multi_assets:false",
                "set_margin_type:BTCUSDT:isolated",
                "set_leverage:BTCUSDT:6",
                "set_margin_type:ETHUSDT:isolated",
                "set_leverage:ETHUSDT:4",
                "read_hedge",
                "read_multi_assets",
                "read_symbol:BTCUSDT",
                "read_symbol:ETHUSDT",
            ]
        );
    }

    #[test]
    fn target_only_scaffold_response_requires_readback() {
        let portfolio = portfolio_fixture("long", vec![strategy_fixture("BTCUSDT", "long", 6)]);
        let target = target_exchange_settings_from_portfolio(&portfolio).expect("target settings");

        let response = response_from_target_without_exchange_readback(
            target,
            "readback_required",
            "exchange readback is added in Task 3",
        );

        assert_eq!(response.status, "readback_required");
        assert_eq!(response.hedge_mode.current, None);
        assert_eq!(response.multi_assets_mode.current, None);
        assert_eq!(response.symbols[0].symbol, "BTCUSDT");
        assert_eq!(response.symbols[0].current_margin_mode, None);
        assert_eq!(response.symbols[0].current_leverage, None);
        assert!(response.warnings[0].contains("exchange readback is required"));
    }

    fn portfolio_fixture(
        direction: &str,
        strategies: Vec<serde_json::Value>,
    ) -> MartingalePortfolioRecord {
        let now = Utc::now();
        MartingalePortfolioRecord {
            portfolio_id: "mp_test".to_owned(),
            owner: "user@example.com".to_owned(),
            name: "BTC basket".to_owned(),
            status: "pending_confirmation".to_owned(),
            source_task_id: "task_test".to_owned(),
            market: "futures".to_owned(),
            direction: direction.to_owned(),
            risk_profile: "balanced".to_owned(),
            total_weight_pct: Decimal::new(100, 0),
            config: json!({ "portfolio_config": { "strategies": strategies } }),
            risk_summary: json!({}),
            created_at: now,
            updated_at: now,
            items: vec![],
        }
    }

    fn strategy_fixture(symbol: &str, direction: &str, leverage: u32) -> serde_json::Value {
        json!({
            "strategy_id": format!("{symbol}-{direction}"),
            "symbol": symbol,
            "market": "usd_m_futures",
            "direction": direction,
            "margin_mode": "isolated",
            "leverage": leverage,
        })
    }

    struct FakeExchangeSettingsClient {
        hedge_mode: bool,
        multi_assets_mode: bool,
        calls: Vec<String>,
        open_orders: std::collections::HashMap<String, Vec<String>>,
        positions: Vec<TestPosition>,
    }

    impl TestExchangeSettingsClient for FakeExchangeSettingsClient {
        fn read_usdm_hedge_mode(&mut self) -> Result<bool, SharedDbError> {
            self.calls.push("read_hedge".to_owned());
            Ok(self.hedge_mode)
        }

        fn set_usdm_position_mode(
            &mut self,
            dual_side_position: bool,
        ) -> Result<(), SharedDbError> {
            self.calls.push(format!("set_hedge:{dual_side_position}"));
            self.hedge_mode = dual_side_position;
            Ok(())
        }

        fn read_usdm_multi_assets_mode(&mut self) -> Result<bool, SharedDbError> {
            self.calls.push("read_multi_assets".to_owned());
            Ok(self.multi_assets_mode)
        }

        fn set_usdm_multi_assets_mode(
            &mut self,
            multi_assets_margin: bool,
        ) -> Result<(), SharedDbError> {
            self.calls
                .push(format!("set_multi_assets:{multi_assets_margin}"));
            self.multi_assets_mode = multi_assets_margin;
            Ok(())
        }

        fn set_usdm_margin_type(
            &mut self,
            symbol: &str,
            margin_mode: &str,
        ) -> Result<(), SharedDbError> {
            self.calls
                .push(format!("set_margin_type:{symbol}:{margin_mode}"));
            Ok(())
        }

        fn set_usdm_leverage(&mut self, symbol: &str, leverage: u32) -> Result<(), SharedDbError> {
            self.calls.push(format!("set_leverage:{symbol}:{leverage}"));
            Ok(())
        }

        fn read_usdm_symbol_settings(
            &mut self,
            symbol: &str,
        ) -> Result<TestCurrentSymbolSettings, SharedDbError> {
            self.calls.push(format!("read_symbol:{symbol}"));
            Ok(TestCurrentSymbolSettings {
                margin_mode: "isolated".to_owned(),
                leverage: if symbol == "BTCUSDT" { 6 } else { 4 },
            })
        }

        fn open_orders_for_symbol(&mut self, symbol: &str) -> Result<Vec<String>, SharedDbError> {
            self.calls.push(format!("open_orders:{symbol}"));
            Ok(self.open_orders.get(symbol).cloned().unwrap_or_default())
        }

        fn read_positions(&mut self) -> Result<Vec<TestPosition>, SharedDbError> {
            self.calls.push("read_positions".to_owned());
            Ok(self.positions.clone())
        }
    }

    #[test]
    fn open_order_blocks_preconfigure() {
        let portfolio = portfolio_fixture("long", vec![strategy_fixture("BTCUSDT", "long", 6)]);
        let mut open_orders = std::collections::HashMap::new();
        open_orders.insert("BTCUSDT".to_owned(), vec!["order-1".to_owned()]);
        let mut exchange = FakeExchangeSettingsClient {
            hedge_mode: false,
            multi_assets_mode: false,
            calls: Vec::new(),
            open_orders,
            positions: Vec::new(),
        };

        let response =
            preconfigure_exchange_with_client(&portfolio, &confirmed_request(), &mut exchange)
                .expect("preconfigure");

        assert_eq!(response.status, "blocked");
        assert!(!response.blocked_symbols.is_empty());
        assert!(response
            .warnings
            .iter()
            .any(|warning| warning.contains("open orders")));
    }

    #[test]
    fn nonzero_position_blocks_preconfigure() {
        let portfolio = portfolio_fixture("long", vec![strategy_fixture("BTCUSDT", "long", 6)]);
        let mut exchange = FakeExchangeSettingsClient {
            hedge_mode: false,
            multi_assets_mode: false,
            calls: Vec::new(),
            open_orders: std::collections::HashMap::new(),
            positions: vec![TestPosition {
                symbol: "BTCUSDT".to_owned(),
                position_amount: "0.01".to_owned(),
            }],
        };

        let response =
            preconfigure_exchange_with_client(&portfolio, &confirmed_request(), &mut exchange)
                .expect("preconfigure");

        assert_eq!(response.status, "blocked");
        assert!(response
            .warnings
            .iter()
            .any(|warning| warning.contains("non-zero position")));
    }
}
