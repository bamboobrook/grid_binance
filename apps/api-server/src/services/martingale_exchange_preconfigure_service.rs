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
    pub confirm_no_auto_orders: bool,
    pub confirm_symbol_margin_leverage_change: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExchangePreconfigureResponse {
    pub status: String,
    pub hedge_mode: HedgeModeCheck,
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
    if !request.confirm_no_auto_orders {
        return Err(SharedDbError::new("no-auto-orders confirmation is required"));
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
        let direction = strategy.get("direction").and_then(Value::as_str).unwrap_or("");
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
            if normalize_margin_mode(&existing.margin_mode) != normalize_margin_mode(&margin_mode) {
                return Err(SharedDbError::new(format!("{symbol} margin mode conflict")));
            }
            if existing.leverage != leverage {
                return Err(SharedDbError::new(format!("{symbol} leverage conflict")));
            }
        } else {
            symbols.insert(
                symbol,
                TargetSymbolSettings {
                    margin_mode: normalize_margin_mode(&margin_mode),
                    leverage,
                },
            );
        }
    }
    Ok(TargetExchangeSettings {
        requires_hedge_mode: portfolio.direction == "long_short" || has_long && has_short,
        symbols,
    })
}

pub fn response_from_target_without_exchange_readback(
    target: TargetExchangeSettings,
    status: &str,
    message: &str,
) -> ExchangePreconfigureResponse {
    ExchangePreconfigureResponse {
        status: status.to_owned(),
        hedge_mode: HedgeModeCheck {
            target: target.requires_hedge_mode,
            current: None,
            status: "unknown".to_owned(),
            message: message.to_owned(),
        },
        symbols: target
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
            .collect(),
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
    let before = readback_response(&client, target.clone(), false)?;
    if target.requires_hedge_mode && before.hedge_mode.current != Some(true) {
        client
            .set_usdm_position_mode(true)
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

fn readback_response(
    client: &BinanceClient,
    target: TargetExchangeSettings,
    applied: bool,
) -> Result<ExchangePreconfigureResponse, SharedDbError> {
    let current_hedge = client
        .read_usdm_position_mode()
        .map_err(|error| SharedDbError::new(error.to_string()))?;
    let hedge_status = if current_hedge == target.requires_hedge_mode {
        "ready"
    } else {
        "mismatch"
    };
    let mut symbols = Vec::with_capacity(target.symbols.len());
    for (symbol, settings) in target.symbols {
        let current = client
            .read_usdm_symbol_settings(&symbol)
            .map_err(|error| SharedDbError::new(error.to_string()))?;
        let current_margin = current.margin_type.map(|value| normalize_margin_mode(&value));
        let margin_ok = current_margin.as_deref() == Some(settings.margin_mode.as_str());
        let leverage_ok = current.leverage == Some(settings.leverage);
        let status = if margin_ok && leverage_ok { "ready" } else { "mismatch" };
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
    let ready = hedge_status == "ready" && symbols.iter().all(|symbol| symbol.status == "ready");
    let mut warnings = vec![
        "Only Binance Futures settings are checked/applied; this endpoint never places orders, cancels orders, or closes positions.".to_owned(),
    ];
    if target.requires_hedge_mode {
        warnings.push("Hedge Mode is account-level and may affect all USDT-M Futures strategies on the Binance account.".to_owned());
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
        symbols,
        warnings,
        checked_at: Utc::now().to_rfc3339(),
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
            serde_json::to_value(response).map_err(|error| SharedDbError::new(error.to_string()))?,
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

fn binance_client_for_owner(db: &SharedDb, owner: &str) -> Result<BinanceClient, SharedDbError> {
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
        assert_eq!(target.symbols.len(), 2);
        assert_eq!(target.symbols["BTCUSDT"].leverage, 6);
        assert_eq!(target.symbols["BTCUSDT"].margin_mode, "isolated");
        assert_eq!(target.symbols["ETHUSDT"].leverage, 4);
    }

    #[test]
    fn conflicting_same_symbol_leverage_is_rejected() {
        let portfolio = portfolio_fixture(
            "long_short",
            vec![
                strategy_fixture("BTCUSDT", "long", 6),
                strategy_fixture("BTCUSDT", "short", 8),
            ],
        );

        let error = target_exchange_settings_from_portfolio(&portfolio).unwrap_err();

        assert!(error.to_string().contains("BTCUSDT leverage conflict"));
    }

    #[test]
    fn running_portfolio_is_not_preconfigurable() {
        let mut portfolio = portfolio_fixture("long", vec![strategy_fixture("BTCUSDT", "long", 6)]);
        portfolio.status = "running".to_owned();

        let error = validate_preconfigure_confirmations(&portfolio, &confirmed_request()).unwrap_err();

        assert!(error.to_string().contains("cannot be exchange-preconfigured"));
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
}
