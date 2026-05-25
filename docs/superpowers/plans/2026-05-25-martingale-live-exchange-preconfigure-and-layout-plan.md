# Martingale Live Exchange Preconfigure And Layout Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add Binance USDT-M exchange preconfiguration for martingale live portfolios and reorganize the backtest/live martingale pages so the workflow is safer and easier to observe.

**Architecture:** Extend `shared-binance` with signed USDT-M POST/readback methods, add API-server portfolio exchange preflight/preconfigure endpoints with explicit risk confirmations, and add frontend controls in the live portfolio detail page. Separately refactor existing backtest page layout into clearer sections without changing backtest algorithms or persisted strategy semantics.

**Tech Stack:** Rust (`shared-binance`, `api-server`, `shared-db`), Next.js/React (`apps/web`), existing Docker Compose deployment, existing `requestBacktestApi` API proxy helpers.

**Spec:** `docs/superpowers/specs/2026-05-25-martingale-live-exchange-preconfigure-design.md`

---

## File Map

- `crates/shared-binance/src/client.rs` — add Binance USDT-M signed POST helpers and readback parsing for position mode / margin type / leverage.
- `apps/api-server/src/services/martingale_exchange_preconfigure_service.rs` — new focused service for deriving target exchange settings, checking current settings, applying changes, and producing response JSON.
- `apps/api-server/src/services/mod.rs` or `apps/api-server/src/lib.rs` — expose the new service module following existing service module pattern.
- `apps/api-server/src/routes/martingale_portfolios.rs` — add `GET exchange-preflight` and `POST exchange-preconfigure` routes.
- `apps/web/app/api/user/martingale-portfolios/[id]/exchange-preflight/route.ts` — proxy read-only preflight request.
- `apps/web/app/api/user/martingale-portfolios/[id]/exchange-preconfigure/route.ts` — proxy preconfigure request.
- `apps/web/components/backtest/exchange-preconfigure-panel.tsx` — new focused UI for target/current exchange state and risk confirmations.
- `apps/web/components/backtest/live-portfolio-controls.tsx` — integrate the new panel and reorganize detail layout into overview / exchange preconfigure / members / controls.
- `apps/web/components/backtest/backtest-console.tsx` — reorganize backtest page into top task area, result exploration tabs, full-width charts, collapsible sandbox/publish section.
- `apps/web/components/backtest/backtest-result-table.tsx` — reduce default visible clutter and keep leverage visible in portfolio members.
- `apps/web/lib/api-types.ts` — add frontend types for exchange preconfigure response if existing local types are insufficient.

---

### Task 1: Add Binance USDT-M Exchange Setting Client Methods

**Files:**
- Modify: `crates/shared-binance/src/client.rs`

- [ ] **Step 1: Write failing tests for signed endpoints**

Add tests inside the existing `#[cfg(test)] mod tests` in `crates/shared-binance/src/client.rs` near existing live order endpoint tests:

```rust
#[test]
fn usdm_exchange_setting_endpoints_submit_signed_posts() {
    let server = MockBinanceServer::start();
    server.enqueue_json(200, r#"{"dualSidePosition":true}"#);
    server.enqueue_json(200, r#"{"code":200,"msg":"success"}"#);
    server.enqueue_json(200, r#"{"symbol":"BTCUSDT","leverage":6,"maxNotionalValue":"1000000"}"#);

    let client = live_test_client(server.base_url());

    client
        .set_usdm_position_mode(true)
        .expect("set position mode");
    client
        .set_usdm_margin_type("BTCUSDT", "isolated")
        .expect("set margin type");
    client
        .set_usdm_leverage("BTCUSDT", 6)
        .expect("set leverage");

    let requests = server.requests();
    assert_eq!(requests[0].method, "POST");
    assert!(requests[0].path.starts_with("/fapi/v1/positionSide/dual?"));
    assert!(requests[0].path.contains("dualSidePosition=true"));

    assert_eq!(requests[1].method, "POST");
    assert!(requests[1].path.starts_with("/fapi/v1/marginType?"));
    assert!(requests[1].path.contains("symbol=BTCUSDT"));
    assert!(requests[1].path.contains("marginType=ISOLATED"));

    assert_eq!(requests[2].method, "POST");
    assert!(requests[2].path.starts_with("/fapi/v1/leverage?"));
    assert!(requests[2].path.contains("symbol=BTCUSDT"));
    assert!(requests[2].path.contains("leverage=6"));
}

#[test]
fn usdm_margin_type_already_target_is_idempotent_success() {
    let server = MockBinanceServer::start();
    server.enqueue_json(400, r#"{"code":-4046,"msg":"No need to change margin type."}"#);
    let client = live_test_client(server.base_url());

    client
        .set_usdm_margin_type("BTCUSDT", "isolated")
        .expect("already isolated should be success");
}
```

If helper names differ, use existing mock server helpers from the same test module, but keep the assertions above exactly equivalent.

- [ ] **Step 2: Run tests and verify RED**

Run:

```bash
cargo test -p shared-binance usdm_exchange_setting_endpoints_submit_signed_posts usdm_margin_type_already_target_is_idempotent_success -- --nocapture
```

Expected: fail because `set_usdm_position_mode`, `set_usdm_margin_type`, and `set_usdm_leverage` do not exist.

- [ ] **Step 3: Implement minimal client methods**

In `impl BinanceClient` in `crates/shared-binance/src/client.rs`, add public methods using existing live HTTP and signed request machinery:

```rust
pub fn set_usdm_position_mode(
    &self,
    dual_side_position: bool,
) -> Result<(), CredentialValidationError> {
    let http = self.live_http_client()?;
    let server_time = self.fetch_server_time(&http, BinanceMarket::Usdm)?;
    let value = if dual_side_position { "true" } else { "false" };
    let _: serde_json::Value = self.signed_request(
        &http,
        "POST",
        self.live_config.base_url(BinanceMarket::Usdm),
        "/fapi/v1/positionSide/dual",
        server_time,
        &[("dualSidePosition".to_owned(), value.to_owned())],
    )?;
    Ok(())
}

pub fn set_usdm_margin_type(
    &self,
    symbol: &str,
    margin_type: &str,
) -> Result<(), CredentialValidationError> {
    let normalized_symbol = symbol.trim().to_uppercase();
    let normalized_margin_type = match margin_type.trim().to_ascii_lowercase().as_str() {
        "isolated" => "ISOLATED",
        "cross" | "crossed" => "CROSSED",
        _ => return Err(CredentialValidationError::new("margin type must be isolated or cross")),
    };
    let http = self.live_http_client()?;
    let server_time = self.fetch_server_time(&http, BinanceMarket::Usdm)?;
    let result: Result<serde_json::Value, CredentialValidationError> = self.signed_request(
        &http,
        "POST",
        self.live_config.base_url(BinanceMarket::Usdm),
        "/fapi/v1/marginType",
        server_time,
        &[
            ("symbol".to_owned(), normalized_symbol),
            ("marginType".to_owned(), normalized_margin_type.to_owned()),
        ],
    );
    match result {
        Ok(_) => Ok(()),
        Err(error) if error.to_string().contains("-4046") || error.to_string().to_ascii_lowercase().contains("no need to change margin type") => Ok(()),
        Err(error) => Err(error),
    }
}

pub fn set_usdm_leverage(
    &self,
    symbol: &str,
    leverage: u32,
) -> Result<(), CredentialValidationError> {
    if !(1..=125).contains(&leverage) {
        return Err(CredentialValidationError::new("leverage must be between 1 and 125"));
    }
    let http = self.live_http_client()?;
    let server_time = self.fetch_server_time(&http, BinanceMarket::Usdm)?;
    let _: serde_json::Value = self.signed_request(
        &http,
        "POST",
        self.live_config.base_url(BinanceMarket::Usdm),
        "/fapi/v1/leverage",
        server_time,
        &[
            ("symbol".to_owned(), symbol.trim().to_uppercase()),
            ("leverage".to_owned(), leverage.to_string()),
        ],
    )?;
    Ok(())
}
```

- [ ] **Step 4: Run tests and verify GREEN**

Run:

```bash
cargo test -p shared-binance usdm_exchange_setting_endpoints_submit_signed_posts usdm_margin_type_already_target_is_idempotent_success -- --nocapture
```

Expected: both tests pass.

- [ ] **Step 5: Commit**

```bash
git add crates/shared-binance/src/client.rs
git commit -m "feat: add Binance futures exchange setting APIs" -m "问题描述: 马丁组合启动前需要自动设置 Hedge Mode、逐仓和杠杆，当前 Binance client 只能下单和读取，不能执行预配置。" -m "修复思路: 增加 USDT-M position mode、margin type、leverage signed POST 方法，并将已是目标 margin type 作为幂等成功处理。"
```

---

### Task 2: Add Exchange Preflight And Preconfigure API Service

**Files:**
- Create: `apps/api-server/src/services/martingale_exchange_preconfigure_service.rs`
- Modify: `apps/api-server/src/services/mod.rs` or `apps/api-server/src/lib.rs` depending on current module exports
- Modify: `apps/api-server/src/routes/martingale_portfolios.rs`

- [ ] **Step 1: Write failing service tests**

Create tests at the bottom of `apps/api-server/src/services/martingale_exchange_preconfigure_service.rs` with a fake exchange client trait:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn missing_confirmations_reject_preconfigure() {
        let portfolio = portfolio_fixture("long_short", vec![strategy_fixture("BTCUSDT", "long", 6)]);
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
}
```

Use actual `MartingalePortfolioRecord` fixture construction from existing service tests in `apps/api-server/src/services/martingale_publish_service.rs`.

- [ ] **Step 2: Run tests and verify RED**

Run:

```bash
cargo test -p api-server martingale_exchange_preconfigure -- --nocapture
```

Expected: fail because module/types/functions do not exist.

- [ ] **Step 3: Implement service types and target derivation**

Create `apps/api-server/src/services/martingale_exchange_preconfigure_service.rs`:

```rust
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
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
        return Err(SharedDbError::new("account-level Hedge Mode confirmation is required"));
    }
    if !request.confirm_no_auto_orders {
        return Err(SharedDbError::new("no-auto-orders confirmation is required"));
    }
    if !target.symbols.is_empty() && !request.confirm_symbol_margin_leverage_change {
        return Err(SharedDbError::new("symbol margin/leverage confirmation is required"));
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
        let direction = strategy.get("direction").and_then(Value::as_str).unwrap_or("");
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
            .ok_or_else(|| SharedDbError::new(format!("{symbol} leverage is required")))? as u32;
        if !(1..=125).contains(&leverage) {
            return Err(SharedDbError::new(format!("{symbol} leverage must be between 1 and 125")));
        }
        if let Some(existing) = symbols.get(&symbol) {
            if existing.margin_mode != margin_mode {
                return Err(SharedDbError::new(format!("{symbol} margin mode conflict")));
            }
            if existing.leverage != leverage {
                return Err(SharedDbError::new(format!("{symbol} leverage conflict")));
            }
        } else {
            symbols.insert(symbol, TargetSymbolSettings { margin_mode, leverage });
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
        symbols: target.symbols.into_iter().map(|(symbol, settings)| SymbolExchangeCheck {
            symbol,
            target_margin_mode: settings.margin_mode,
            current_margin_mode: None,
            target_leverage: settings.leverage,
            current_leverage: None,
            status: "unknown".to_owned(),
            message: message.to_owned(),
        }).collect(),
        warnings: vec!["exchange readback is required before live start".to_owned()],
    }
}

pub fn exchange_preconfigure_summary(response: &ExchangePreconfigureResponse) -> Value {
    json!({
        "status": response.status,
        "hedge_mode": response.hedge_mode,
        "symbols": response.symbols,
        "warnings": response.warnings,
    })
}
```

- [ ] **Step 4: Wire routes with target-only scaffold response**

In `apps/api-server/src/routes/martingale_portfolios.rs`, add routes:

```rust
.route("/martingale-portfolios/{id}/exchange-preflight", get(exchange_preflight_portfolio))
.route("/martingale-portfolios/{id}/exchange-preconfigure", post(exchange_preconfigure_portfolio))
```

Add handlers that:

1. Load session and portfolio using existing publish/backtest service pattern.
2. For preflight: call `target_exchange_settings_from_portfolio`, return `response_from_target_without_exchange_readback(target, "readback_required", "exchange readback is added in Task 3")`; do not report this as success.
3. For preconfigure: call `validate_preconfigure_confirmations`; then return the same target response.

This step exists to make the route contract compile before Task 3 adds real exchange calls; the response status must be `readback_required` so it cannot be mistaken for a successful exchange check.

- [ ] **Step 5: Run tests and check**

Run:

```bash
cargo test -p api-server martingale_exchange_preconfigure -- --nocapture
cargo check -p api-server
```

Expected: tests pass and api-server compiles.

- [ ] **Step 6: Commit**

```bash
git add apps/api-server/src/services/martingale_exchange_preconfigure_service.rs apps/api-server/src/routes/martingale_portfolios.rs apps/api-server/src/services/mod.rs apps/api-server/src/lib.rs
git commit -m "feat: add martingale exchange preconfigure API contract" -m "问题描述: 实盘组合启动前缺少交易所配置检查和风险确认接口。" -m "修复思路: 增加目标配置解析、风险确认校验和 exchange-preflight/exchange-preconfigure 路由，为后续接入 Binance 设置调用提供稳定合同。"
```

---

### Task 3: Wire Real Binance Readback And Apply Flow

**Files:**
- Modify: `apps/api-server/src/services/martingale_exchange_preconfigure_service.rs`
- Modify: `apps/api-server/src/routes/martingale_portfolios.rs`
- Modify: `crates/shared-binance/src/client.rs` if readback helpers are missing

- [ ] **Step 1: Write failing apply-flow test with fake exchange**

In `martingale_exchange_preconfigure_service.rs` tests add:

```rust
#[test]
fn preconfigure_runs_hedge_then_margin_then_leverage_then_readback() {
    let portfolio = portfolio_fixture(
        "long_short",
        vec![strategy_fixture("BTCUSDT", "long", 6), strategy_fixture("ETHUSDT", "short", 4)],
    );
    let mut exchange = FakeExchange::new()
        .with_hedge_mode(false)
        .with_symbol("BTCUSDT", "cross", 1)
        .with_symbol("ETHUSDT", "cross", 1);
    let request = ExchangePreconfigureRequest {
        confirm_account_level_hedge_mode_change: true,
        confirm_no_auto_orders: true,
        confirm_symbol_margin_leverage_change: true,
    };

    let response = preconfigure_exchange_with_client(&portfolio, &request, &mut exchange).expect("preconfigure");

    assert_eq!(response.status, "succeeded");
    assert_eq!(exchange.calls, vec![
        "set_position_mode:true",
        "set_margin_type:BTCUSDT:isolated",
        "set_leverage:BTCUSDT:6",
        "set_margin_type:ETHUSDT:isolated",
        "set_leverage:ETHUSDT:4",
        "readback",
    ]);
    assert!(response.symbols.iter().all(|symbol| symbol.status == "ok"));
}
```

Define `FakeExchange` in the test module to implement the trait added in Step 3 below.

- [ ] **Step 2: Run test and verify RED**

Run:

```bash
cargo test -p api-server preconfigure_runs_hedge_then_margin_then_leverage_then_readback -- --nocapture
```

Expected: fail because `ExchangeSettingsClient` trait and `preconfigure_exchange_with_client` do not exist.

- [ ] **Step 3: Implement exchange trait and apply flow**

In `martingale_exchange_preconfigure_service.rs`, add:

```rust
pub trait ExchangeSettingsClient {
    fn read_usdm_hedge_mode(&self) -> Result<bool, SharedDbError>;
    fn read_usdm_symbol_settings(&self, symbols: &[String]) -> Result<BTreeMap<String, TargetSymbolSettings>, SharedDbError>;
    fn set_usdm_position_mode(&mut self, enabled: bool) -> Result<(), SharedDbError>;
    fn set_usdm_margin_type(&mut self, symbol: &str, margin_mode: &str) -> Result<(), SharedDbError>;
    fn set_usdm_leverage(&mut self, symbol: &str, leverage: u32) -> Result<(), SharedDbError>;
}

pub fn preflight_exchange_with_client(
    portfolio: &MartingalePortfolioRecord,
    exchange: &impl ExchangeSettingsClient,
) -> Result<ExchangePreconfigureResponse, SharedDbError> {
    let target = target_exchange_settings_from_portfolio(portfolio)?;
    let symbols = target.symbols.keys().cloned().collect::<Vec<_>>();
    let current_hedge = exchange.read_usdm_hedge_mode()?;
    let current_symbols = exchange.read_usdm_symbol_settings(&symbols)?;
    Ok(compare_target_and_current(target, current_hedge, current_symbols, "checked"))
}

pub fn preconfigure_exchange_with_client(
    portfolio: &MartingalePortfolioRecord,
    request: &ExchangePreconfigureRequest,
    exchange: &mut impl ExchangeSettingsClient,
) -> Result<ExchangePreconfigureResponse, SharedDbError> {
    validate_preconfigure_confirmations(portfolio, request)?;
    let target = target_exchange_settings_from_portfolio(portfolio)?;
    let current_hedge = exchange.read_usdm_hedge_mode()?;
    if target.requires_hedge_mode && !current_hedge {
        exchange.set_usdm_position_mode(true)?;
    }
    for (symbol, settings) in &target.symbols {
        exchange.set_usdm_margin_type(symbol, &settings.margin_mode)?;
        exchange.set_usdm_leverage(symbol, settings.leverage)?;
    }
    let symbols = target.symbols.keys().cloned().collect::<Vec<_>>();
    let readback_hedge = exchange.read_usdm_hedge_mode()?;
    let readback_symbols = exchange.read_usdm_symbol_settings(&symbols)?;
    Ok(compare_target_and_current(target, readback_hedge, readback_symbols, "succeeded"))
}
```

Implement `compare_target_and_current` so status is `succeeded` only when hedge/symbol settings all match; otherwise `failed` with per-symbol `status="mismatch"`.

- [ ] **Step 4: Implement Binance adapter**

Add adapter in API server route/service layer that wraps `shared_binance::BinanceClient` and implements `ExchangeSettingsClient`. Use existing user exchange credential loading pattern from strategy start/preflight code. Convert `CredentialValidationError` into `SharedDbError` preserving message.

Readback behavior:

- `read_usdm_hedge_mode`: call existing position mode read helper or add a public wrapper in `shared-binance`.
- `read_usdm_symbol_settings`: use Binance position risk/account endpoint; parse `marginType` and `leverage` per symbol into `TargetSymbolSettings`.

- [ ] **Step 5: Persist summary**

After preflight/preconfigure route gets response, merge into portfolio risk summary under `exchange_preconfigure`. If repository lacks a helper to update portfolio risk summary, add one in `crates/shared-db/src/backtest.rs`:

```rust
pub fn update_martingale_portfolio_risk_summary(
    &self,
    owner: &str,
    portfolio_id: &str,
    risk_summary: serde_json::Value,
) -> Result<Option<MartingalePortfolioRecord>, SharedDbError>
```

Use SQL:

```sql
UPDATE martingale_portfolios
SET risk_summary = $3, updated_at = now()
WHERE owner = $1 AND portfolio_id = $2
RETURNING ...
```

Follow existing `set_martingale_portfolio_status` row mapping.

- [ ] **Step 6: Run tests**

Run:

```bash
cargo test -p api-server martingale_exchange_preconfigure -- --nocapture
cargo test -p shared-binance usdm -- --nocapture
cargo check -p shared-db -p shared-binance -p api-server
```

Expected: all pass.

- [ ] **Step 7: Commit**

```bash
git add apps/api-server/src/services/martingale_exchange_preconfigure_service.rs apps/api-server/src/routes/martingale_portfolios.rs crates/shared-db/src/backtest.rs crates/shared-binance/src/client.rs
git commit -m "feat: apply Binance exchange preconfiguration" -m "问题描述: exchange-preconfigure API 只有合同，尚未真实读取和设置 Binance Futures 状态。" -m "修复思路: 接入 Binance readback 与设置调用，按 Hedge Mode、margin type、leverage 顺序执行，并把结果写入 Portfolio risk_summary。"
```

---

### Task 4: Add Frontend API Proxies And Exchange Preconfigure Panel

**Files:**
- Create: `apps/web/app/api/user/martingale-portfolios/[id]/exchange-preflight/route.ts`
- Create: `apps/web/app/api/user/martingale-portfolios/[id]/exchange-preconfigure/route.ts`
- Create: `apps/web/components/backtest/exchange-preconfigure-panel.tsx`
- Modify: `apps/web/lib/api-types.ts`

- [ ] **Step 1: Add API proxy routes**

Create `exchange-preflight/route.ts`:

```ts
import { proxyBackend } from "@/app/api/_utils/proxy";

export async function GET(request: Request, context: { params: Promise<{ id: string }> }) {
  const { id } = await context.params;
  return proxyBackend(request, {
    backendPath: `/martingale-portfolios/${id}/exchange-preflight`,
  });
}
```

Create `exchange-preconfigure/route.ts`:

```ts
import { proxyBackend } from "@/app/api/_utils/proxy";

export async function POST(request: Request, context: { params: Promise<{ id: string }> }) {
  const { id } = await context.params;
  return proxyBackend(request, {
    backendPath: `/martingale-portfolios/${id}/exchange-preconfigure`,
    method: "POST",
  });
}
```

If this repo uses a different helper signature in nearby routes, copy that exact pattern.

- [ ] **Step 2: Add frontend types**

In `apps/web/lib/api-types.ts`, add:

```ts
export type ExchangeHedgeModeCheck = {
  target: boolean;
  current?: boolean | null;
  status: string;
  message: string;
};

export type ExchangeSymbolCheck = {
  symbol: string;
  target_margin_mode: string;
  current_margin_mode?: string | null;
  target_leverage: number;
  current_leverage?: number | null;
  status: string;
  message: string;
};

export type ExchangePreconfigureResponse = {
  status: string;
  hedge_mode: ExchangeHedgeModeCheck;
  symbols: ExchangeSymbolCheck[];
  warnings: string[];
};
```

- [ ] **Step 3: Create panel component**

Create `apps/web/components/backtest/exchange-preconfigure-panel.tsx`:

```tsx
"use client";

import { useState } from "react";
import type { ExchangePreconfigureResponse } from "@/lib/api-types";
import { pickText, type UiLanguage } from "@/lib/ui/preferences";
import { requestBacktestApi } from "@/components/backtest/request-client";

type Props = {
  portfolioId: string;
  lang: UiLanguage;
  disabled?: boolean;
};

export function ExchangePreconfigurePanel({ portfolioId, lang, disabled }: Props) {
  const [result, setResult] = useState<ExchangePreconfigureResponse | null>(null);
  const [pending, setPending] = useState(false);
  const [feedback, setFeedback] = useState("");
  const [confirmHedge, setConfirmHedge] = useState(false);
  const [confirmOrders, setConfirmOrders] = useState(false);
  const [confirmSymbols, setConfirmSymbols] = useState(false);

  async function check() {
    setPending(true);
    setFeedback(pickText(lang, "正在检查交易所配置…", "Checking exchange settings..."));
    const response = await requestBacktestApi(`/api/user/martingale-portfolios/${portfolioId}/exchange-preflight`, { cache: "no-store" });
    setPending(false);
    if (!response.ok) {
      setFeedback(response.message);
      return;
    }
    setResult(response.data as ExchangePreconfigureResponse);
    setFeedback(pickText(lang, "检查完成。", "Check complete."));
  }

  async function configure() {
    setPending(true);
    setFeedback(pickText(lang, "正在预配置 Binance Futures…", "Preconfiguring Binance Futures..."));
    const response = await requestBacktestApi(`/api/user/martingale-portfolios/${portfolioId}/exchange-preconfigure`, {
      method: "POST",
      headers: { "content-type": "application/json" },
      body: JSON.stringify({
        confirm_account_level_hedge_mode_change: confirmHedge,
        confirm_no_auto_orders: confirmOrders,
        confirm_symbol_margin_leverage_change: confirmSymbols,
      }),
    });
    setPending(false);
    if (!response.ok) {
      setFeedback(response.message);
      return;
    }
    setResult(response.data as ExchangePreconfigureResponse);
    setFeedback(pickText(lang, "交易所预配置完成，请再次检查后启动。", "Exchange preconfiguration complete; review before start."));
  }

  const canConfigure = confirmHedge && confirmOrders && confirmSymbols && !pending && !disabled;

  return (
    <section className="rounded-2xl border border-border bg-card p-4 shadow-sm space-y-4">
      <div className="flex flex-wrap items-start justify-between gap-3">
        <div>
          <h2 className="text-lg font-semibold">{pickText(lang, "交易所预配置", "Exchange preconfiguration")}</h2>
          <p className="text-sm text-muted-foreground">
            {pickText(lang, "自动设置 Binance USDT-M 的 Hedge Mode、逐仓/全仓和杠杆；不会自动下单。", "Automatically sets Binance USDT-M Hedge Mode, margin type, and leverage; it never places orders.")}
          </p>
        </div>
        <button className="rounded-full border border-border px-3 py-2 text-sm font-medium disabled:opacity-60" disabled={pending || disabled} onClick={() => void check()} type="button">
          {pickText(lang, "检查交易所配置", "Check exchange settings")}
        </button>
      </div>

      <div className="rounded-xl border border-amber-500/40 bg-amber-500/5 p-3 text-sm text-amber-800 dark:text-amber-200">
        {pickText(lang, "注意：Hedge Mode 是账户级设置，会影响该 Binance Futures 账户下所有 USDT-M 交易。", "Warning: Hedge Mode is account-level and affects all USDT-M trading on this Binance Futures account.")}
      </div>

      {result ? <ExchangeResultTable result={result} lang={lang} /> : null}

      <div className="space-y-2 text-sm">
        <ConfirmBox checked={confirmHedge} onChange={setConfirmHedge} label={pickText(lang, "我确认允许系统修改账户级 Hedge Mode。", "I confirm the system may change account-level Hedge Mode.")} />
        <ConfirmBox checked={confirmOrders} onChange={setConfirmOrders} label={pickText(lang, "我确认该操作不会自动下单，启动仍需我手动确认。", "I confirm this will not place orders; live start still requires my manual confirmation.")} />
        <ConfirmBox checked={confirmSymbols} onChange={setConfirmSymbols} label={pickText(lang, "我确认允许系统修改这些交易对的逐仓/全仓和杠杆。", "I confirm the system may change margin type and leverage for these symbols.")} />
      </div>

      <button className="w-full rounded-full bg-primary px-4 py-2 text-sm font-medium text-primary-foreground disabled:opacity-60" disabled={!canConfigure} onClick={() => void configure()} type="button">
        {pending ? pickText(lang, "处理中…", "Processing...") : pickText(lang, "自动预配置交易所", "Auto-preconfigure exchange")}
      </button>
      <p className="text-sm text-muted-foreground" aria-live="polite">{feedback}</p>
    </section>
  );
}

function ConfirmBox({ checked, onChange, label }: { checked: boolean; onChange: (next: boolean) => void; label: string }) {
  return <label className="flex items-start gap-2"><input checked={checked} onChange={(event) => onChange(event.currentTarget.checked)} type="checkbox" /><span>{label}</span></label>;
}

function ExchangeResultTable({ result, lang }: { result: ExchangePreconfigureResponse; lang: UiLanguage }) {
  return (
    <div className="space-y-3">
      <div className="grid gap-2 rounded-xl border border-border p-3 text-sm md:grid-cols-4">
        <span className="text-muted-foreground">Hedge Mode</span>
        <span>{String(result.hedge_mode.current ?? "?")} → {String(result.hedge_mode.target)}</span>
        <span>{result.hedge_mode.status}</span>
        <span className="text-muted-foreground">{result.hedge_mode.message}</span>
      </div>
      <div className="overflow-x-auto rounded-xl border border-border">
        <table className="min-w-full text-sm">
          <thead className="bg-secondary/40 text-muted-foreground">
            <tr>
              <th className="px-3 py-2 text-left">Symbol</th>
              <th className="px-3 py-2 text-left">Margin</th>
              <th className="px-3 py-2 text-left">Leverage</th>
              <th className="px-3 py-2 text-left">Status</th>
              <th className="px-3 py-2 text-left">Message</th>
            </tr>
          </thead>
          <tbody>
            {result.symbols.map((symbol) => (
              <tr className="border-t border-border" key={symbol.symbol}>
                <td className="px-3 py-2 font-medium">{symbol.symbol}</td>
                <td className="px-3 py-2">{symbol.current_margin_mode ?? "?"} → {symbol.target_margin_mode}</td>
                <td className="px-3 py-2">{symbol.current_leverage ?? "?"}x → {symbol.target_leverage}x</td>
                <td className="px-3 py-2">{symbol.status}</td>
                <td className="px-3 py-2 text-muted-foreground">{symbol.message}</td>
              </tr>
            ))}
          </tbody>
        </table>
      </div>
      {result.warnings.length > 0 ? <ul className="list-disc space-y-1 pl-5 text-sm text-amber-600">{result.warnings.map((warning) => <li key={warning}>{warning}</li>)}</ul> : null}
    </div>
  );
}
```

- [ ] **Step 4: Run frontend build**

Run:

```bash
cd apps/web && npm run build
```

Expected: build passes.

- [ ] **Step 5: Commit**

```bash
git add apps/web/app/api/user/martingale-portfolios apps/web/components/backtest/exchange-preconfigure-panel.tsx apps/web/lib/api-types.ts
git commit -m "feat: add exchange preconfigure panel" -m "问题描述: 用户无法在实盘组合页查看或自动修正 Binance 交易所侧 Hedge Mode、逐仓和杠杆配置。" -m "修复思路: 增加前端代理路由、类型和预配置面板，要求风险确认后才调用后端设置接口。"
```

---

### Task 5: Reorganize Live Portfolio Detail Page

**Files:**
- Modify: `apps/web/components/backtest/live-portfolio-controls.tsx`
- Import: `apps/web/components/backtest/exchange-preconfigure-panel.tsx`

- [ ] **Step 1: Identify current detail sections**

Open `apps/web/components/backtest/live-portfolio-controls.tsx` and find `MartingalePortfolioDetail`. Keep existing data loading and action handlers. Do not change status API semantics.

- [ ] **Step 2: Import panel**

Add:

```tsx
import { ExchangePreconfigurePanel } from "@/components/backtest/exchange-preconfigure-panel";
```

- [ ] **Step 3: Reorder JSX into four sections**

Inside `MartingalePortfolioDetail`, render sections in this order:

1. Overview card: status, portfolio id, source task, market, direction, strategy count, max leverage.
2. `<ExchangePreconfigurePanel portfolioId={portfolio.portfolio_id} lang={lang} disabled={portfolio.status === "running"} />`
3. Strategy members card/list: existing strategy cards/table, keeping member actions.
4. Operations card: confirm start / pause / stop controls and warning text.

Use existing helper components (`Card`, `CardHeader`, `CardBody`, `MetricBlock`, `Chip`) rather than introducing a new UI system.

- [ ] **Step 4: Add blocking reason copy near start button**

Near confirm start button, display:

```tsx
<p className="text-xs text-muted-foreground">
  {pickText(lang, "启动前仍会二次校验 Hedge Mode、逐仓/全仓与杠杆；若交易所状态被手动改动，启动会被拒绝。", "Before live start, Hedge Mode, margin type, and leverage are checked again; if exchange settings changed manually, start is rejected.")}
</p>
```

- [ ] **Step 5: Build**

Run:

```bash
cd apps/web && npm run build
```

Expected: build passes.

- [ ] **Step 6: Commit**

```bash
git add apps/web/components/backtest/live-portfolio-controls.tsx
git commit -m "refactor: clarify martingale portfolio detail workflow" -m "问题描述: 实盘组合详情页状态、成员、启动控制和新增交易所预配置混在一起，启动前准备不清晰。" -m "修复思路: 重排为概览、交易所预配置、策略成员、运行控制四块，并保留启动前二次校验提示。"
```

---

### Task 6: Reorganize Backtest Console Layout

**Files:**
- Modify: `apps/web/components/backtest/backtest-console.tsx`
- Modify: `apps/web/components/backtest/backtest-result-table.tsx`

- [ ] **Step 1: Add result tab state**

In `BacktestConsole`, add:

```tsx
type ResultTab = "single" | "portfolio" | "charts";
const [resultTab, setResultTab] = useState<ResultTab>("portfolio");
const [sandboxExpanded, setSandboxExpanded] = useState(false);
```

When `editPortfolioSandbox` or `addCandidateToSandbox` runs, call `setSandboxExpanded(true)`.

- [ ] **Step 2: Split top task area**

Wrap existing wizard/professional controls and task list in a top grid:

```tsx
<div className="grid gap-4 xl:grid-cols-[minmax(0,1fr)_380px]">
  <section className="rounded-2xl border border-border bg-card p-4 shadow-sm">
    {/* existing tab buttons + wizard/professional panel */}
  </section>
  <BacktestTaskList ... />
</div>
```

Keep existing props and handlers unchanged.

- [ ] **Step 3: Add result tabs**

Above result area add buttons:

```tsx
<div className="flex flex-wrap gap-2 rounded-2xl border border-border bg-card p-2 shadow-sm">
  {(["single", "portfolio", "charts"] as const).map((tab) => (
    <button
      key={tab}
      className={cn("rounded-full px-4 py-2 text-sm font-medium", resultTab === tab ? "bg-primary text-primary-foreground" : "hover:bg-secondary")}
      onClick={() => setResultTab(tab)}
      type="button"
    >
      {tab === "single" ? pickText(lang, "单币 Top10", "Single Top10") : tab === "portfolio" ? pickText(lang, "组合 Top3", "Portfolio Top3") : pickText(lang, "图表与明细", "Charts & details")}
    </button>
  ))}
</div>
```

- [ ] **Step 4: Render result sections by tab**

Keep `BacktestResultTable`, but pass a new optional prop if needed to hide single or portfolio sections. If modifying the table is too invasive, split rendering in `BacktestConsole` by keeping table in `single` and `portfolio` tabs and charts in `charts` tab. The visible behavior must be:

- `single`: show per-symbol candidate groups.
- `portfolio`: show portfolio cards.
- `charts`: show `BacktestCharts` full-width and selected detail title.

- [ ] **Step 5: Make sandbox collapsible**

Wrap the existing sandbox JSX with:

```tsx
<section className="rounded-2xl border border-border bg-card p-4 shadow-sm space-y-3">
  <div className="flex items-center justify-between gap-3">
    <div>
      <h2 className="text-lg font-semibold">{pickText(lang, "组合沙盒与发布", "Portfolio sandbox & publish")}</h2>
      <p className="text-sm text-muted-foreground">{pickText(lang, "编辑自动组合或手动加入候选，重算后再作为发布篮子。", "Edit auto portfolios or add candidates, recalculate, then use as publish basket.")}</p>
    </div>
    <button className="rounded-full border border-border px-3 py-1 text-xs font-medium" onClick={() => setSandboxExpanded((open) => !open)} type="button">
      {sandboxExpanded ? pickText(lang, "收起", "Collapse") : pickText(lang, "展开", "Expand")}
    </button>
  </div>
  {sandboxExpanded ? existingSandboxAndPublishJsx : null}
</section>
```

- [ ] **Step 6: Keep chart full width**

Ensure `BacktestCharts` is not inside a narrow grid. Use:

```tsx
<section className="rounded-2xl border border-border bg-card p-4 shadow-sm">
  <BacktestCharts ... />
</section>
```

- [ ] **Step 7: Build**

Run:

```bash
cd apps/web && npm run build
```

Expected: build passes.

- [ ] **Step 8: Commit**

```bash
git add apps/web/components/backtest/backtest-console.tsx apps/web/components/backtest/backtest-result-table.tsx
git commit -m "refactor: reorganize martingale backtest workspace" -m "问题描述: 回测页创建任务、任务列表、候选、组合、图表、沙盒和发布区域同时堆叠，影响观察和操作。" -m "修复思路: 重排为任务区、结果 tabs、全宽图表和折叠式沙盒发布区，降低默认信息密度。"
```

---

### Task 7: Full Verification And Deployment

**Files:**
- No code changes expected unless verification finds defects.

- [ ] **Step 1: Run backend tests**

```bash
cargo test -p shared-binance usdm -- --nocapture
cargo test -p api-server martingale_exchange_preconfigure -- --nocapture
cargo check -p shared-db -p shared-binance -p api-server -p trading-engine -p backtest-engine -p backtest-worker
```

Expected: tests pass; existing unrelated warnings may remain.

- [ ] **Step 2: Run frontend build**

```bash
cd apps/web && npm run build
```

Expected: build passes.

- [ ] **Step 3: Deploy changed services**

```bash
docker compose --env-file /home/bumblebee/Project/grid_binance/.worktrees/full-v1/.env -f deploy/docker/docker-compose.yml build api-server web
docker compose --env-file /home/bumblebee/Project/grid_binance/.worktrees/full-v1/.env -f deploy/docker/docker-compose.yml up -d api-server web
```

Expected: `api-server` and `web` restart successfully.

- [ ] **Step 4: Check service health**

```bash
docker compose --env-file /home/bumblebee/Project/grid_binance/.worktrees/full-v1/.env -f deploy/docker/docker-compose.yml ps api-server web nginx trading-engine backtest-worker
```

Expected: API, web, nginx, trading-engine are healthy/up. Do not touch port `3000` directly; public frontend remains via nginx `8080`.

- [ ] **Step 5: Smoke test UI routes**

Open or curl through nginx if authenticated route is not easily scriptable:

```bash
curl -sS http://127.0.0.1:8080/zh/app/backtest | head
curl -sS http://127.0.0.1:8080/zh/app/martingale-portfolios | head
```

Expected: HTML response, no 500 page.

- [ ] **Step 6: Commit deployment note if needed**

If Task 7 required fixes, commit them with a message including problem and fix idea. If no fixes, no commit needed.

---

## Self-Review

- Spec coverage: Tasks 1-3 cover Binance client, API preflight/preconfigure, risk confirmation, readback, persistence. Tasks 4-6 cover frontend preconfigure panel and page information architecture. Task 7 covers verification/deployment.
- Placeholder scan: no TBD/TODO placeholders remain. Each task has exact files, commands, and expected results.
- Type consistency: exchange response fields match frontend types and spec JSON: `status`, `hedge_mode`, `symbols`, `target_margin_mode`, `current_margin_mode`, `target_leverage`, `current_leverage`.
- Known prerequisite: current root `.git` was previously mounted read-only. Before implementation, ensure `.git` is writable or execute in a writable worktree; otherwise code can be changed/deployed but commits will fail.
