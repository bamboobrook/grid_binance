# Strategy Engine Rewrite Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Replace the current mixed grid behavior with a state-machine-based strategy engine that separates default ordinary grids from optional classic bilateral grids, makes preview match runtime, and enforces `只卖不买` plus `止盈停止` correctly.

**Architecture:** Promote strategy type and runtime phase into shared domain and persistence, then rebuild the Rust grid builder and runtime around two explicit execution paths: `ordinary_grid` and `classic_bilateral_grid`. Wire the API service and Next.js workspace to the same normalized strategy definition so the create form, preview, pre-flight, runtime orders, and statistics all consume one source of truth.

**Tech Stack:** Rust workspace (`shared-domain`, `shared-db`, `api-server`, `trading-engine`), PostgreSQL migrations, Next.js/React, Node contract tests, Playwright, Cargo integration tests.

---

## File Structure

### Domain and Persistence
- Modify: `crates/shared-domain/src/strategy.rs`
  Purpose: define `StrategyType`, `ReferencePriceSource`, new runtime phase, ordinary vs bilateral configuration fields, runtime control flags, and level execution records.
- Modify: `crates/shared-db/src/lib.rs`
  Purpose: persist the expanded strategy definition, runtime phase, controls, and per-level execution statistics.
- Create: `db/migrations/0008_strategy_engine_rewrite.sql`
  Purpose: add database columns and tables required by the new strategy definition and per-level runtime accounting.

### Trading Engine
- Modify: `apps/trading-engine/src/grid_builder.rs`
  Purpose: build ordinary-grid ladders with fixed anchor spacing and bilateral ladders with selectable spacing mode.
- Modify: `apps/trading-engine/src/runtime.rs`
  Purpose: expose runtime types for the new state machine and order intents.
- Modify: `apps/trading-engine/src/strategy_runtime.rs`
  Purpose: implement ordinary-grid startup, bilateral startup, draining, and stop-after-take-profit transitions.
- Modify: `apps/trading-engine/src/execution_effects.rs`
  Purpose: convert state-machine actions into concrete order-create, order-cancel, and position-close effects.
- Modify: `apps/trading-engine/src/take_profit.rs`
  Purpose: compute per-level TP from actual fill price and preserve trailing TP rules.
- Modify: `apps/trading-engine/src/statistics.rs`
  Purpose: project level-one and later fills into per-level and strategy-level statistics.

### API Service
- Modify: `apps/api-server/src/services/strategy_service.rs`
  Purpose: normalize form payloads into the new strategy definition, validate ordinary vs bilateral rules, and run hedge-mode pre-flight checks.
- Modify: `apps/api-server/src/services/analytics_service.rs`
  Purpose: expose the new per-level and per-strategy statistics.
- Modify: `apps/api-server/tests/strategy_flow.rs`
  Purpose: verify create/edit/pre-flight/start rules.
- Modify: `apps/api-server/tests/analytics_flow.rs`
  Purpose: verify first-level market fill and later per-level totals appear correctly.

### Web Workspace
- Create: `apps/web/components/strategies/strategy-definition-sections.tsx`
  Purpose: split ordinary-grid fields from classic-bilateral fields so the form stops mixing both products in one block.
- Create: `apps/web/components/strategies/strategy-runtime-controls.tsx`
  Purpose: render `只卖不买` and `止盈停止` controls with clear confirmations.
- Modify: `apps/web/components/strategies/strategy-workspace-form.tsx`
  Purpose: branch early by strategy type and market, send normalized fields to the API, and stop client-side bilateral assumptions for ordinary grids.
- Modify: `apps/web/components/strategies/strategy-visual-preview.tsx`
  Purpose: show only anchor/center, grid lines, and covered range, with no TP overlays.
- Modify: `apps/web/app/api/user/strategies/create/route.ts`
  Purpose: forward the new strategy fields cleanly to the API service.
- Modify: `apps/web/app/[locale]/app/strategies/[id]/page.tsx`
  Purpose: surface runtime controls and strategy-type-specific detail summaries.

### Verification and Docs
- Modify: `apps/trading-engine/tests/grid_runtime.rs`
  Purpose: lock builder formulas and ordinary-grid startup semantics.
- Modify: `apps/trading-engine/tests/execution_effects.rs`
  Purpose: lock draining and stop-after-take-profit execution effects.
- Modify: `tests/verification/strategy_surface_contract.test.mjs`
  Purpose: lock the workspace and preview contract.
- Modify: `tests/e2e/user_app.spec.ts`
  Purpose: exercise the new create-strategy behavior through the browser.
- Modify: `docs/user-guide/create-grid-strategy.md`
  Purpose: document ordinary-grid and classic-bilateral behavior in English.
- Modify: `docs/user-guide/zh/create-grid-strategy.md`
  Purpose: document the same rules in Chinese.

---

### Task 1: Expand the Shared Strategy Model and Persistence Foundation

**Files:**
- Modify: `crates/shared-domain/src/strategy.rs`
- Modify: `crates/shared-db/src/lib.rs`
- Create: `db/migrations/0008_strategy_engine_rewrite.sql`
- Test: `apps/api-server/tests/strategy_flow.rs`

- [ ] **Step 1: Write the failing API test for the new strategy definition shape**

```rust
#[tokio::test]
async fn create_strategy_returns_explicit_strategy_type_and_runtime_phase() {
    let app = test_app().await;
    let session_token = register_and_login(&app, "ordinary-shape@example.com", "pass1234").await;

    let created = create_strategy(
        &app,
        &session_token,
        serde_json::json!({
            "name": "BTC Ordinary",
            "symbol": "BTCUSDT",
            "market": "spot",
            "strategy_type": "ordinary_grid",
            "mode": "spot_grid",
            "reference_price_source": "manual",
            "reference_price": "70000",
            "grid_count": 4,
            "grid_spacing_bps": 100,
            "amount_mode": "quote",
            "levels": [
                { "level_index": 0, "quantity": "0.001", "take_profit_bps": 200 },
                { "level_index": 1, "quantity": "0.001", "take_profit_bps": 200 },
                { "level_index": 2, "quantity": "0.001", "take_profit_bps": 200 },
                { "level_index": 3, "quantity": "0.001", "take_profit_bps": 200 }
            ],
            "runtime_controls": {
                "only_sell_no_buy": false,
                "stop_after_take_profit": false
            }
        }),
    ).await;

    assert_eq!(created.status(), StatusCode::CREATED);
    let body = response_json(created).await;
    assert_eq!(body["strategy_type"], "ordinary_grid");
    assert_eq!(body["runtime_phase"], "draft");
    assert_eq!(body["draft_revision"]["reference_price_source"], "manual");
}
```

- [ ] **Step 2: Run the API strategy test to verify it fails for the missing fields**

Run: `cd /home/bumblebee/Project/grid_binance/.worktrees/full-v1 && source "$HOME/.cargo/env" && cargo test -p api-server --test strategy_flow create_strategy_returns_explicit_strategy_type_and_runtime_phase -- --nocapture`
Expected: FAIL because `strategy_type`, `runtime_phase`, and `reference_price_source` are absent from the current strategy model.

- [ ] **Step 3: Add the minimal shared-domain and persistence structure**

```rust
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub enum StrategyType {
    OrdinaryGrid,
    ClassicBilateralGrid,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub enum ReferencePriceSource {
    Manual,
    Market,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub enum StrategyRuntimePhase {
    Draft,
    PreflightReady,
    Starting,
    Running,
    Draining,
    Stopped,
    Error,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
pub struct RuntimeControls {
    pub only_sell_no_buy: bool,
    pub stop_after_take_profit: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct StrategyRevision {
    pub revision_id: String,
    pub version: u32,
    pub strategy_type: StrategyType,
    pub reference_price_source: ReferencePriceSource,
    pub generation: GridGeneration,
    pub levels: Vec<GridLevel>,
    // keep existing fields and append the new normalized fields here
}
```

```sql
ALTER TABLE strategies
    ADD COLUMN IF NOT EXISTS strategy_type TEXT NOT NULL DEFAULT 'ordinary_grid',
    ADD COLUMN IF NOT EXISTS runtime_phase TEXT NOT NULL DEFAULT 'draft',
    ADD COLUMN IF NOT EXISTS runtime_controls JSONB NOT NULL DEFAULT '{"only_sell_no_buy": false, "stop_after_take_profit": false}'::jsonb;

ALTER TABLE strategy_revisions
    ADD COLUMN IF NOT EXISTS strategy_type TEXT NOT NULL DEFAULT 'ordinary_grid',
    ADD COLUMN IF NOT EXISTS reference_price_source TEXT NOT NULL DEFAULT 'manual';

CREATE TABLE IF NOT EXISTS strategy_runtime_level_lots (
    lot_id TEXT PRIMARY KEY,
    strategy_id TEXT NOT NULL REFERENCES strategies(id) ON DELETE CASCADE,
    level_index INTEGER NOT NULL,
    entry_order_id TEXT,
    take_profit_order_id TEXT,
    entry_fill_price TEXT NOT NULL,
    entry_fill_quantity TEXT NOT NULL,
    take_profit_fill_price TEXT,
    realized_pnl TEXT,
    fee_amount TEXT,
    fee_asset TEXT,
    cycle_state TEXT NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);
```

- [ ] **Step 4: Re-run the API strategy test to verify the new shape exists**

Run: `cd /home/bumblebee/Project/grid_binance/.worktrees/full-v1 && source "$HOME/.cargo/env" && cargo test -p api-server --test strategy_flow create_strategy_returns_explicit_strategy_type_and_runtime_phase -- --nocapture`
Expected: PASS with the response exposing `strategy_type`, `runtime_phase`, and `reference_price_source`.

- [ ] **Step 5: Commit the persistence foundation**

```bash
cd /home/bumblebee/Project/grid_binance/.worktrees/full-v1
git add crates/shared-domain/src/strategy.rs crates/shared-db/src/lib.rs db/migrations/0008_strategy_engine_rewrite.sql apps/api-server/tests/strategy_flow.rs
git commit -m "refactor: 修复思路 扩展策略模型与持久化基础"
```

### Task 2: Rewrite the Grid Builder for Ordinary and Classic Bilateral Formulas

**Files:**
- Modify: `apps/trading-engine/src/grid_builder.rs`
- Modify: `apps/trading-engine/src/runtime.rs`
- Test: `apps/trading-engine/tests/grid_runtime.rs`

- [ ] **Step 1: Write failing grid-builder tests for ordinary fixed-step and bilateral spacing modes**

```rust
#[test]
fn ordinary_spot_grid_uses_anchor_fixed_step_without_bilateral_levels() {
    let ladder = GridBuilder::ordinary_fixed_step(
        GridMode::SpotGrid,
        decimal(70000, 0),
        100,
        4,
    ).expect("ordinary grid should build");

    assert_eq!(
        ladder.levels,
        vec![
            decimal(70000, 0),
            decimal(69300, 0),
            decimal(68600, 0),
            decimal(67900, 0),
        ]
    );
}

#[test]
fn classic_bilateral_grid_supports_fixed_and_geometric_spacing() {
    let fixed = GridBuilder::classic_bilateral_fixed(
        decimal(70000, 0),
        100,
        2,
    ).expect("fixed bilateral grid should build");
    assert_eq!(fixed.lower_levels, vec![decimal(69300, 0), decimal(68600, 0)]);
    assert_eq!(fixed.upper_levels, vec![decimal(70700, 0), decimal(71400, 0)]);

    let geometric = GridBuilder::classic_bilateral_geometric(
        decimal(70000, 0),
        100,
        2,
    ).expect("geometric bilateral grid should build");
    assert_eq!(geometric.lower_levels, vec![decimal(69300, 0), decimal(68607, 0)]);
    assert_eq!(geometric.upper_levels, vec![decimal(70700, 0), decimal(71407, 0)]);
}
```

- [ ] **Step 2: Run the trading-engine grid test to verify it fails**

Run: `cd /home/bumblebee/Project/grid_binance/.worktrees/full-v1 && source "$HOME/.cargo/env" && cargo test -p trading-engine --test grid_runtime ordinary_spot_grid_uses_anchor_fixed_step_without_bilateral_levels -- --nocapture`
Expected: FAIL because the current builder only understands arithmetic/geometric/custom plans for the old mixed model.

- [ ] **Step 3: Implement the new builder entry points and modes**

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GridMode {
    SpotGrid,
    FuturesLong,
    FuturesShort,
    ClassicBilateralSpot,
    ClassicBilateralFutures,
}

pub fn ordinary_fixed_step(
    mode: GridMode,
    anchor_price: Decimal,
    spacing_bps: u32,
    grid_count: u32,
) -> Result<OrdinaryGridPlan, GridBuildError> {
    let step = anchor_price * Decimal::new(spacing_bps as i64, 4);
    let levels = (0..grid_count)
        .map(|index| match mode {
            GridMode::SpotGrid | GridMode::FuturesLong => anchor_price - (step * Decimal::from(index)),
            GridMode::FuturesShort => anchor_price + (step * Decimal::from(index)),
            _ => unreachable!("ordinary builder only supports ordinary modes"),
        })
        .collect::<Vec<_>>();
    Ok(OrdinaryGridPlan { mode, levels })
}

pub fn classic_bilateral_fixed(
    center_price: Decimal,
    spacing_bps: u32,
    levels_per_side: u32,
) -> Result<ClassicBilateralPlan, GridBuildError> {
    // generate upper and lower ladders from the same center price
}
```

- [ ] **Step 4: Re-run the grid-builder tests to verify both modes now build the correct ladders**

Run: `cd /home/bumblebee/Project/grid_binance/.worktrees/full-v1 && source "$HOME/.cargo/env" && cargo test -p trading-engine --test grid_runtime -- --nocapture`
Expected: PASS for both ordinary and classic bilateral formulas.

- [ ] **Step 5: Commit the builder rewrite**

```bash
cd /home/bumblebee/Project/grid_binance/.worktrees/full-v1
git add apps/trading-engine/src/grid_builder.rs apps/trading-engine/src/runtime.rs apps/trading-engine/tests/grid_runtime.rs
git commit -m "feat: 问题描述 重写普通网格与经典双边网格构建器"
```

### Task 3: Rebuild the Ordinary-Grid Runtime State Machine

**Files:**
- Modify: `apps/trading-engine/src/strategy_runtime.rs`
- Modify: `apps/trading-engine/src/runtime.rs`
- Modify: `apps/trading-engine/src/execution_effects.rs`
- Modify: `apps/trading-engine/src/take_profit.rs`
- Test: `apps/trading-engine/tests/grid_runtime.rs`
- Test: `apps/trading-engine/tests/execution_effects.rs`

- [ ] **Step 1: Write failing runtime tests for first-fill startup and fill-gated take profit creation**

```rust
#[test]
fn ordinary_grid_start_executes_level_one_and_places_only_one_take_profit_plus_lower_entries() {
    let config = ordinary_runtime_config(decimal(70000, 0), 100, 4);
    let runtime = GridRuntime::start(config).expect("ordinary runtime should start");

    assert_eq!(runtime.phase(), RuntimePhase::Running);
    assert_eq!(runtime.filled_lots().len(), 1, "level one must be filled immediately");
    assert_eq!(runtime.take_profit_orders().len(), 1, "only the first filled lot has a TP order");
    assert_eq!(runtime.replenishment_orders().len(), 3, "remaining levels stay as one-sided entries");
}

#[test]
fn ordinary_grid_creates_take_profit_only_after_the_corresponding_level_fills() {
    let mut runtime = GridRuntime::start(ordinary_runtime_config(decimal(70000, 0), 100, 4))
        .expect("ordinary runtime should start");

    assert_eq!(runtime.take_profit_orders().len(), 1);
    runtime.record_replenishment_fill(1, decimal(69300, 0), decimal(1, 3)).expect("level two fill");
    assert_eq!(runtime.take_profit_orders().len(), 2);
}
```

- [ ] **Step 2: Run the runtime tests to verify the current engine fails**

Run: `cd /home/bumblebee/Project/grid_binance/.worktrees/full-v1 && source "$HOME/.cargo/env" && cargo test -p trading-engine --test grid_runtime ordinary_grid_start_executes_level_one_and_places_only_one_take_profit_plus_lower_entries -- --nocapture`
Expected: FAIL because the current runtime does not model immediate level-one market fills or level-gated take-profit creation.

- [ ] **Step 3: Implement ordinary-grid startup, filled-lot tracking, and TP gating**

```rust
pub fn start(config: GridRuntimeConfig) -> Result<Self, RuntimeError> {
    match config.strategy_type {
        StrategyType::OrdinaryGrid => Self::start_ordinary(config),
        StrategyType::ClassicBilateralGrid => Self::start_classic_bilateral(config),
    }
}

fn start_ordinary(config: GridRuntimeConfig) -> Result<Self, RuntimeError> {
    let first_level = config.plan.ordinary_levels().first().cloned().ok_or(RuntimeError::EmptyPlan)?;
    let first_fill = FilledLot::market_entry(
        0,
        first_level.anchor_price,
        config.level_sizing(0)?,
    );

    let mut runtime = Self::from_config(config);
    runtime.phase = RuntimePhase::Running;
    runtime.filled_lots.push(first_fill.clone());
    runtime.orders.push(OrderIntent::take_profit_from_fill(&first_fill)?);
    runtime.orders.extend(runtime.plan.remaining_replenishment_orders(1)?);
    Ok(runtime)
}

fn record_replenishment_fill(&mut self, level_index: u32, fill_price: Decimal, quantity: Decimal) -> Result<(), RuntimeError> {
    let lot = FilledLot::entry(level_index, fill_price, quantity);
    self.filled_lots.push(lot.clone());
    self.orders.push(OrderIntent::take_profit_from_fill(&lot)?);
    Ok(())
}
```

- [ ] **Step 4: Re-run the runtime tests to verify level-one startup and fill-gated TP creation**

Run: `cd /home/bumblebee/Project/grid_binance/.worktrees/full-v1 && source "$HOME/.cargo/env" && cargo test -p trading-engine --test grid_runtime -- --nocapture`
Expected: PASS with runtime orders showing one immediate TP and one-sided remaining entries.

- [ ] **Step 5: Commit the ordinary runtime rewrite**

```bash
cd /home/bumblebee/Project/grid_binance/.worktrees/full-v1
git add apps/trading-engine/src/strategy_runtime.rs apps/trading-engine/src/runtime.rs apps/trading-engine/src/execution_effects.rs apps/trading-engine/src/take_profit.rs apps/trading-engine/tests/grid_runtime.rs apps/trading-engine/tests/execution_effects.rs
git commit -m "feat: 修复思路 重构普通网格运行时状态机"
```

### Task 4: Add Draining and Stop-After-Take-Profit Runtime Controls

**Files:**
- Modify: `apps/trading-engine/src/strategy_runtime.rs`
- Modify: `apps/trading-engine/src/execution_effects.rs`
- Test: `apps/trading-engine/tests/execution_effects.rs`

- [ ] **Step 1: Write failing tests for `only_sell_no_buy` and `stop_after_take_profit`**

```rust
#[test]
fn only_sell_no_buy_cancels_entries_and_enters_draining() {
    let mut runtime = seeded_running_ordinary_runtime();
    let effects = runtime.enable_only_sell_no_buy().expect("draining should start");

    assert_eq!(runtime.phase(), RuntimePhase::Draining);
    assert!(effects.cancel_order_ids.iter().all(|id| id.starts_with("entry-")));
    assert!(runtime.replenishment_orders().is_empty());
    assert!(!runtime.take_profit_orders().is_empty());
}

#[test]
fn stop_after_take_profit_stops_after_draining_position_is_fully_closed() {
    let mut runtime = seeded_running_ordinary_runtime();
    runtime.controls.stop_after_take_profit = true;
    runtime.enable_only_sell_no_buy().expect("draining should start");
    runtime.record_take_profit_fill(0, decimal(71400, 0)).expect("first exit");
    runtime.record_take_profit_fill(1, decimal(72100, 0)).expect("final exit");

    assert_eq!(runtime.phase(), RuntimePhase::Stopped);
}
```

- [ ] **Step 2: Run the execution-effects test to verify it fails**

Run: `cd /home/bumblebee/Project/grid_binance/.worktrees/full-v1 && source "$HOME/.cargo/env" && cargo test -p trading-engine --test execution_effects only_sell_no_buy_cancels_entries_and_enters_draining -- --nocapture`
Expected: FAIL because the current runtime does not support a draining phase or stop-after-take-profit semantics.

- [ ] **Step 3: Implement draining transitions and stop conditions**

```rust
pub fn enable_only_sell_no_buy(&mut self) -> Result<ExecutionEffects, RuntimeError> {
    self.controls.only_sell_no_buy = true;
    self.phase = RuntimePhase::Draining;

    let entry_ids = self
        .orders
        .iter()
        .filter(|order| order.kind == OrderKind::Replenishment)
        .map(|order| order.order_id.clone())
        .collect::<Vec<_>>();

    self.orders.retain(|order| order.kind != OrderKind::Replenishment);
    Ok(ExecutionEffects { cancel_order_ids: entry_ids, ..ExecutionEffects::default() })
}

fn maybe_finish_after_take_profit(&mut self) {
    if self.controls.stop_after_take_profit
        && self.phase == RuntimePhase::Draining
        && self.open_position_quantity().is_zero()
    {
        self.phase = RuntimePhase::Stopped;
    }
}
```

- [ ] **Step 4: Re-run the execution-effects tests to verify draining behavior now works**

Run: `cd /home/bumblebee/Project/grid_binance/.worktrees/full-v1 && source "$HOME/.cargo/env" && cargo test -p trading-engine --test execution_effects -- --nocapture`
Expected: PASS with entry orders canceled immediately and the runtime stopping after draining closes the last lot.

- [ ] **Step 5: Commit the draining controls**

```bash
cd /home/bumblebee/Project/grid_binance/.worktrees/full-v1
git add apps/trading-engine/src/strategy_runtime.rs apps/trading-engine/src/execution_effects.rs apps/trading-engine/tests/execution_effects.rs
git commit -m "feat: 问题描述 加入只卖不买与止盈停止状态控制"
```

### Task 5: Normalize Strategy Create/Edit and Server-Side Pre-Flight

**Files:**
- Modify: `apps/api-server/src/services/strategy_service.rs`
- Modify: `crates/shared-db/src/lib.rs`
- Test: `apps/api-server/tests/strategy_flow.rs`

- [ ] **Step 1: Write failing API tests for ordinary-grid validation and futures bilateral hedge-mode enforcement**

```rust
#[tokio::test]
async fn ordinary_grid_rejects_bilateral_configuration_fields() {
    let app = test_app().await;
    let session_token = register_and_login(&app, "ordinary-validation@example.com", "pass1234").await;

    let response = create_strategy(
        &app,
        &session_token,
        serde_json::json!({
            "name": "Bad Ordinary",
            "symbol": "BTCUSDT",
            "market": "spot",
            "strategy_type": "ordinary_grid",
            "mode": "spot_grid",
            "levels_per_side": 3,
            "spacing_mode": "geometric"
        }),
    ).await;

    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    assert!(response_text(response).await.contains("ordinary grid does not accept bilateral fields"));
}

#[tokio::test]
async fn futures_classic_bilateral_preflight_fails_without_hedge_mode() {
    let app = test_app_without_hedge_mode().await;
    let session_token = register_and_login(&app, "hedge-check@example.com", "pass1234").await;
    let created = create_strategy(
        &app,
        &session_token,
        serde_json::json!({
            "name": "ETH Bilateral",
            "symbol": "ETHUSDT",
            "market": "futures_usd_m",
            "strategy_type": "classic_bilateral_grid",
            "mode": "classic_bilateral",
            "reference_price_source": "market",
            "levels_per_side": 3,
            "spacing_mode": "fixed_step"
        }),
    ).await;
    let strategy_id = response_json(created).await["id"].as_str().unwrap().to_owned();

    let preflight = preflight_strategy(&app, &session_token, &strategy_id).await;
    let body = response_json(preflight).await;
    assert_eq!(body["ok"], false);
    assert_eq!(body["failures"][0]["step"], "hedge_mode");
}
```

- [ ] **Step 2: Run the API tests to verify they fail against the current service logic**

Run: `cd /home/bumblebee/Project/grid_binance/.worktrees/full-v1 && source "$HOME/.cargo/env" && cargo test -p api-server --test strategy_flow -- --nocapture`
Expected: FAIL because the current service still accepts mixed semantics and old futures-neutral assumptions.

- [ ] **Step 3: Implement a strategy-definition normalizer and pre-flight checks in the service layer**

```rust
fn normalize_definition(request: CreateStrategyRequest) -> Result<NormalizedStrategyDefinition, StrategyError> {
    match request.strategy_type {
        StrategyType::OrdinaryGrid => {
            if request.levels_per_side.is_some() || request.spacing_mode == Some(SpacingMode::Geometric) {
                return Err(StrategyError::bad_request("ordinary grid does not accept bilateral fields"));
            }
            Ok(NormalizedStrategyDefinition::ordinary(request)?)
        }
        StrategyType::ClassicBilateralGrid => {
            Ok(NormalizedStrategyDefinition::classic_bilateral(request)?)
        }
    }
}

fn preflight_hedge_mode(definition: &NormalizedStrategyDefinition, exchange_snapshot: &ExchangeSnapshot) -> Option<PreflightFailure> {
    if definition.strategy_type == StrategyType::ClassicBilateralGrid
        && definition.market.is_futures()
        && !exchange_snapshot.hedge_mode_enabled
    {
        return Some(PreflightFailure {
            step: "hedge_mode".to_string(),
            reason: "Classic bilateral futures grid requires Binance hedge mode.".to_string(),
            guidance: Some("Switch the Binance futures account to hedge mode and rerun pre-flight.".to_string()),
        });
    }
    None
}
```

- [ ] **Step 4: Re-run the API tests to verify ordinary-grid normalization and hedge-mode pre-flight are enforced**

Run: `cd /home/bumblebee/Project/grid_binance/.worktrees/full-v1 && source "$HOME/.cargo/env" && cargo test -p api-server --test strategy_flow -- --nocapture`
Expected: PASS with a hard ordinary-grid validation error and a specific hedge-mode pre-flight failure.

- [ ] **Step 5: Commit the API normalization layer**

```bash
cd /home/bumblebee/Project/grid_binance/.worktrees/full-v1
git add apps/api-server/src/services/strategy_service.rs crates/shared-db/src/lib.rs apps/api-server/tests/strategy_flow.rs
git commit -m "refactor: 修复思路 统一策略定义归一化与预检规则"
```

### Task 6: Rewrite the Strategy Workspace and Preview Contract

**Files:**
- Create: `apps/web/components/strategies/strategy-definition-sections.tsx`
- Create: `apps/web/components/strategies/strategy-runtime-controls.tsx`
- Modify: `apps/web/components/strategies/strategy-workspace-form.tsx`
- Modify: `apps/web/components/strategies/strategy-visual-preview.tsx`
- Modify: `apps/web/app/api/user/strategies/create/route.ts`
- Modify: `tests/verification/strategy_surface_contract.test.mjs`
- Modify: `tests/e2e/user_app.spec.ts`

- [ ] **Step 1: Write failing UI contract and browser tests for one-sided preview and strategy-type branching**

```javascript
test("strategy workspace separates ordinary and classic bilateral configuration", () => {
  const formSource = read("/home/bumblebee/Project/grid_binance/.worktrees/full-v1/apps/web/components/strategies/strategy-workspace-form.tsx");
  const previewSource = read("/home/bumblebee/Project/grid_binance/.worktrees/full-v1/apps/web/components/strategies/strategy-visual-preview.tsx");

  assert.match(formSource, /ordinary_grid|普通网格/, "workspace should expose the ordinary-grid default type");
  assert.match(formSource, /classic_bilateral_grid|经典双边网格/, "workspace should expose the optional bilateral type");
  assert.doesNotMatch(previewSource, /takeProfitLines|TP Price|止盈价/, "preview chart should drop TP overlays for clarity");
  assert.match(previewSource, /covered range|网格范围/, "preview should emphasize the covered ladder range");
});
```

```typescript
test("ordinary grid preview stays one-sided from the first anchor level", async ({ page }) => {
  const email = uniqueEmail("ordinary-preview");
  const password = "pass1234";

  await registerViaPage(page, email, password);
  await page.goto("/app/strategies/new");
  await page.getByLabel("Strategy Type").selectOption("ordinary_grid");
  await page.getByLabel("Market Type").selectOption("spot");
  await page.getByLabel("Reference Price").fill("70000");
  await page.getByLabel("Grid Count").fill("4");
  await page.getByLabel("Batch Spacing (%)").fill("1");

  await expect(page.locator('[data-preview-ladder-side="upper"]')).toHaveCount(0);
  await expect(page.locator('[data-preview-ladder-side="lower"]')).toHaveCount(3);
});
```

- [ ] **Step 2: Run the workspace verification tests to confirm failure**

Run: `cd /home/bumblebee/Project/grid_binance/.worktrees/full-v1 && node --test tests/verification/strategy_surface_contract.test.mjs`
Expected: FAIL because the current preview still renders TP lines and the workspace still blends ordinary and bilateral assumptions.

- [ ] **Step 3: Split the form sections and simplify the preview component**

```tsx
export function StrategyDefinitionSections(props: StrategyDefinitionSectionsProps) {
  if (props.strategyType === "ordinary_grid") {
    return (
      <OrdinaryGridFields
        market={props.market}
        direction={props.direction}
        referencePriceSource={props.referencePriceSource}
        gridCount={props.gridCount}
        spacingPercent={props.spacingPercent}
      />
    );
  }

  return (
    <ClassicBilateralFields
      market={props.market}
      centerPrice={props.referencePrice}
      levelsPerSide={props.levelsPerSide}
      spacingMode={props.spacingMode}
    />
  );
}
```

```tsx
const chartModel = {
  anchorLine: strategyType === "ordinary_grid" ? anchorPrice : null,
  centerLine: strategyType === "classic_bilateral_grid" ? centerPrice : null,
  lowerLines,
  upperLines,
  coveredRange,
};

return (
  <svg data-strategy-preview-chart="true">
    <RangeBand value={chartModel.coveredRange} />
    {chartModel.anchorLine ? <AnchorLine value={chartModel.anchorLine} /> : null}
    {chartModel.centerLine ? <CenterLine value={chartModel.centerLine} /> : null}
    {chartModel.lowerLines.map((line) => <GridLine key={line.key} side="lower" value={line.value} />)}
    {chartModel.upperLines.map((line) => <GridLine key={line.key} side="upper" value={line.value} />)}
  </svg>
);
```

- [ ] **Step 4: Re-run the verification and browser tests to verify the workspace now matches the new product rule**

Run: `cd /home/bumblebee/Project/grid_binance/.worktrees/full-v1 && node --test tests/verification/strategy_surface_contract.test.mjs`
Expected: PASS with no TP overlay requirement and explicit ordinary vs bilateral branching.

Run: `cd /home/bumblebee/Project/grid_binance/.worktrees/full-v1 && pnpm exec playwright test -c apps/web/playwright.config.ts tests/e2e/user_app.spec.ts --grep "ordinary grid preview stays one-sided from the first anchor level"`
Expected: PASS with the preview showing only the lower ladder for an ordinary spot grid.

- [ ] **Step 5: Commit the workspace rewrite**

```bash
cd /home/bumblebee/Project/grid_binance/.worktrees/full-v1
git add apps/web/components/strategies/strategy-definition-sections.tsx apps/web/components/strategies/strategy-runtime-controls.tsx apps/web/components/strategies/strategy-workspace-form.tsx apps/web/components/strategies/strategy-visual-preview.tsx apps/web/app/api/user/strategies/create/route.ts tests/verification/strategy_surface_contract.test.mjs tests/e2e/user_app.spec.ts
git commit -m "feat: 问题描述 重写策略创建页与单侧预览契约"
```

### Task 7: Project Per-Level Statistics and Update Strategy Docs

**Files:**
- Modify: `apps/trading-engine/src/statistics.rs`
- Modify: `apps/api-server/src/services/analytics_service.rs`
- Modify: `apps/api-server/tests/analytics_flow.rs`
- Modify: `docs/user-guide/create-grid-strategy.md`
- Modify: `docs/user-guide/zh/create-grid-strategy.md`

- [ ] **Step 1: Write failing analytics tests for first-level market fills and later-level per-grid totals**

```rust
#[tokio::test]
async fn first_level_market_fill_is_counted_in_level_statistics() {
    let app = test_app_with_seeded_ordinary_strategy().await;
    let session_token = login_existing(&app, "analytics-grid@example.com", "pass1234").await;

    seed_level_fill(&app, "strategy-ordinary-1", 0, "70000", "0.001", "35.00", "USDT").await;
    let analytics = fetch_strategy_analytics(&app, &session_token, "strategy-ordinary-1").await;
    let body = response_json(analytics).await;

    assert_eq!(body["level_stats"][0]["level_index"], 0);
    assert_eq!(body["level_stats"][0]["entry_fill_price"], "70000");
    assert_eq!(body["level_stats"][0]["fee_amount"], "35.00");
}
```

- [ ] **Step 2: Run the analytics test to confirm the current projection is missing the required per-level data**

Run: `cd /home/bumblebee/Project/grid_binance/.worktrees/full-v1 && source "$HOME/.cargo/env" && cargo test -p api-server --test analytics_flow first_level_market_fill_is_counted_in_level_statistics -- --nocapture`
Expected: FAIL because the current analytics layer does not project the first level as a normal per-grid accounting unit.

- [ ] **Step 3: Implement per-level lot projection and refresh the docs**

```rust
#[derive(Debug, Clone, Serialize)]
pub struct LevelStatistic {
    pub level_index: u32,
    pub entry_fill_price: Decimal,
    pub entry_fill_quantity: Decimal,
    pub take_profit_fill_price: Option<Decimal>,
    pub realized_pnl: Decimal,
    pub fee_amount: Decimal,
    pub fee_asset: Option<String>,
    pub cycle_state: String,
}

pub fn project_level_statistics(lots: &[StrategyRuntimeLevelLot]) -> Vec<LevelStatistic> {
    lots.iter().map(|lot| LevelStatistic {
        level_index: lot.level_index,
        entry_fill_price: lot.entry_fill_price,
        entry_fill_quantity: lot.entry_fill_quantity,
        take_profit_fill_price: lot.take_profit_fill_price,
        realized_pnl: lot.realized_pnl.unwrap_or_default(),
        fee_amount: lot.fee_amount.unwrap_or_default(),
        fee_asset: lot.fee_asset.clone(),
        cycle_state: lot.cycle_state.clone(),
    }).collect()
}
```

```md
## Ordinary Grid

- Level 1 is a real market fill at startup.
- Level 1 is counted like every later filled level.
- Only filled levels create take-profit orders.
- `Only Sell No Buy` cancels all remaining replenishment orders and leaves only existing exits.
```

- [ ] **Step 4: Re-run analytics and doc-facing verification**

Run: `cd /home/bumblebee/Project/grid_binance/.worktrees/full-v1 && source "$HOME/.cargo/env" && cargo test -p api-server --test analytics_flow first_level_market_fill_is_counted_in_level_statistics -- --nocapture`
Expected: PASS with level 1 shown in the per-level statistics response.

Run: `cd /home/bumblebee/Project/grid_binance/.worktrees/full-v1 && pnpm --filter web build`
Expected: PASS because the strategy pages and docs ingestion still compile cleanly after the new copy and analytics wiring.

- [ ] **Step 5: Commit the statistics and documentation update**

```bash
cd /home/bumblebee/Project/grid_binance/.worktrees/full-v1
git add apps/trading-engine/src/statistics.rs apps/api-server/src/services/analytics_service.rs apps/api-server/tests/analytics_flow.rs docs/user-guide/create-grid-strategy.md docs/user-guide/zh/create-grid-strategy.md
git commit -m "docs: 修复思路 同步逐格统计与策略说明文档"
```

### Task 8: Run the Full Verification Matrix for the Rewrite

**Files:**
- Modify as needed from previous tasks only
- Test: `apps/trading-engine/tests/grid_runtime.rs`
- Test: `apps/trading-engine/tests/execution_effects.rs`
- Test: `apps/api-server/tests/strategy_flow.rs`
- Test: `apps/api-server/tests/analytics_flow.rs`
- Test: `tests/verification/strategy_surface_contract.test.mjs`
- Test: `tests/e2e/user_app.spec.ts`

- [ ] **Step 1: Run the Rust engine test suite for the rewritten strategy semantics**

```bash
cd /home/bumblebee/Project/grid_binance/.worktrees/full-v1
source "$HOME/.cargo/env"
cargo test -p trading-engine --test grid_runtime --test execution_effects
```

Expected: PASS for ordinary-grid builder formulas, classic bilateral builder formulas, first-level startup, draining, and stop-after-take-profit.

- [ ] **Step 2: Run the API service suite for create, pre-flight, and analytics behavior**

```bash
cd /home/bumblebee/Project/grid_binance/.worktrees/full-v1
source "$HOME/.cargo/env"
cargo test -p api-server --test strategy_flow --test analytics_flow
```

Expected: PASS for strategy normalization, hedge-mode failure messaging, and per-level statistics.

- [ ] **Step 3: Run the web contract and browser suite**

```bash
cd /home/bumblebee/Project/grid_binance/.worktrees/full-v1
node --test tests/verification/strategy_surface_contract.test.mjs
pnpm exec playwright test -c apps/web/playwright.config.ts tests/e2e/user_app.spec.ts --grep "strategy|ordinary grid"
```

Expected: PASS for workspace branching, simplified preview, and browser-level ordinary-grid flows.

- [ ] **Step 4: Run the production web build and capture any remaining regressions**

```bash
cd /home/bumblebee/Project/grid_binance/.worktrees/full-v1
pnpm --filter web build
```

Expected: PASS with no TypeScript or Next.js build regressions.

- [ ] **Step 5: Commit the verified rewrite batch**

```bash
cd /home/bumblebee/Project/grid_binance/.worktrees/full-v1
git add apps/trading-engine apps/api-server apps/web tests docs db/migrations crates/shared-domain crates/shared-db
git commit -m "feat: 问题描述 完成策略引擎整套重构与验收"
```

## Self-Review

### Spec Coverage Check
- `ordinary_grid` vs `classic_bilateral_grid` separation: Tasks 1, 2, 5
- ordinary-grid first-level immediate fill: Tasks 2, 3
- one-sided replenishment only: Tasks 2, 3, 6
- bilateral mode retained but non-default: Tasks 2, 5, 6
- `only_sell_no_buy` draining semantics: Task 4
- `stop_after_take_profit` stop semantics: Task 4
- futures bilateral hedge-mode pre-flight failure: Task 5
- simplified preview without TP lines: Task 6
- first-level and later-level per-grid statistics: Task 7
- updated docs and acceptance matrix: Tasks 7, 8

### Placeholder Scan
- No `TODO`, `TBD`, or `FIXME` markers remain in this plan.
- Each task includes an explicit failing test, run command, minimal implementation sketch, verification command, and commit step.

### Type Consistency Check
- Strategy types consistently use `ordinary_grid` and `classic_bilateral_grid`.
- Runtime controls consistently use `only_sell_no_buy` and `stop_after_take_profit`.
- Runtime phases consistently use `draft`, `preflight_ready`, `starting`, `running`, `draining`, `stopped`, and `error`.
