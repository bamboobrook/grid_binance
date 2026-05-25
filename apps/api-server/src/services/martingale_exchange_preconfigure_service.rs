use serde::{Deserialize, Serialize};
use serde_json::Value;
use shared_db::{MartingalePortfolioRecord, SharedDbError};
use std::collections::BTreeMap;

#[derive(Debug, Clone, Deserialize)]
pub struct ExchangePreconfigureRequest {
    pub confirm_account_level_hedge_mode_change: bool,
    pub confirm_no_auto_orders: bool,
    pub confirm_symbol_margin_leverage_change: bool,
}

#[derive(Debug, Clone, Serialize)]
pub struct ExchangePreconfigureResponse {
    pub status: String,
    pub hedge_mode: HedgeModeCheck,
    pub symbols: Vec<SymbolExchangeCheck>,
    pub warnings: Vec<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct HedgeModeCheck {
    pub target: bool,
    pub current: Option<bool>,
    pub status: String,
    pub message: String,
}

#[derive(Debug, Clone, Serialize)]
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
    let target = target_exchange_settings_from_portfolio(portfolio)?;
    if target.requires_hedge_mode && !request.confirm_account_level_hedge_mode_change {
        return Err(SharedDbError::new(
            "account-level Hedge Mode confirmation is required",
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
            if existing.margin_mode != margin_mode {
                return Err(SharedDbError::new(format!("{symbol} margin mode conflict")));
            }
            if existing.leverage != leverage {
                return Err(SharedDbError::new(format!("{symbol} leverage conflict")));
            }
        } else {
            symbols.insert(
                symbol,
                TargetSymbolSettings {
                    margin_mode,
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
        warnings: vec!["exchange readback is required before live start".to_owned()],
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Utc;
    use serde_json::json;
    use shared_db::MartingalePortfolioRecord;
    use shared_domain::strategy::Decimal;

    #[test]
    fn missing_confirmations_reject_preconfigure() {
        let portfolio =
            portfolio_fixture("long_short", vec![strategy_fixture("BTCUSDT", "long", 6)]);
        let request = ExchangePreconfigureRequest {
            confirm_account_level_hedge_mode_change: false,
            confirm_no_auto_orders: true,
            confirm_symbol_margin_leverage_change: true,
        };

        let error = validate_preconfigure_confirmations(&portfolio, &request).unwrap_err();

        assert!(error.to_string().contains("account-level Hedge Mode"));
    }

    #[test]
    fn target_settings_group_symbols_and_keep_leverage() {
        let portfolio = portfolio_fixture(
            "long_short",
            vec![
                strategy_fixture("BTCUSDT", "long", 6),
                strategy_fixture("BTCUSDT", "short", 6),
                strategy_fixture("ETHUSDT", "long", 4),
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
