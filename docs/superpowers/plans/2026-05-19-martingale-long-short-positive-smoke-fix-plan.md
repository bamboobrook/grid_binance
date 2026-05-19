# Martingale Long/Short Positive Smoke Fix Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Make `direction_mode=long_short` produce real simultaneous dual-direction martingale candidates with positive annualized returns and controlled drawdown in the BTC/ETH balanced smoke, without reverting to single-direction substitutes or relaxing risk standards.

**Architecture:** Keep the contract fix from the previous plan: `long_short` must only generate `LongAndShort` candidates and balanced first-pass drawdown remains 25%. Fix the actual optimizer/model issues causing all dual-leg candidates to be massively negative: long/short legs currently share identical spacing/TP/SL and start every bar with both legs, causing extreme over-trading, fee drag, and absurd drawdowns. Introduce asymmetric long/short leg parameter generation plus cooldown/entry gating for true dual-leg candidates, then require Docker smoke evidence before merge.

**Tech Stack:** Rust `backtest-engine`, Rust `backtest-worker`, Node contract tests, Docker compose stack on host `8080`.

---

## Failure Evidence From Current Deployed Smoke

Task created after merging Claude branch:

- Task id: `bt_1779197010515966900`
- Payload: BTC/ETH, `direction_mode=long_short`, balanced, leverage `[2]`, spacing `[120]`, multiplier `[1.25]`, max legs `[3]`, TP `[60]`, tail stop `[2000]`, weights `[[60,40],[50,50]]`
- Result: `failed`
- Error:

```text
no martingale candidates selected: direction_mode=long_short symbols=BTCUSDT,ETHUSDT screened_count=40 selected_count=0 risk_profile=balanced negative_return=64 drawdown_rejected=0 zero_trade=0 survival_valid=0
```

Diagnostics showed every candidate was true `long_short`, but all were unusable:

```json
{
  "direction_mode": "long_short",
  "total_return_pct": -39551.01,
  "max_drawdown_pct": 39437.65,
  "trade_count": 3465628,
  "survival_valid": false
}
```

This is not an acceptable final state. It proves the previous fix preserved direction contract but did not restore a useful long/short optimizer.

## Non-Negotiable Product Standards

- `long_short` must mean simultaneous dual-leg portfolio candidates containing both long and short strategies.
- Do not insert `LongOnly` or `ShortOnly` candidates into a `long_short` task.
- Do not relax risk standards to pass weak results:
  - conservative first-pass max drawdown: `20%`
  - balanced first-pass max drawdown: `25%`
  - aggressive first-pass max drawdown: `30%`
- Negative-return candidates are diagnostic only; they must not become selected results or portfolio members.
- Smoke success requires real positive candidate(s), real annualized return, real curve/trades, and real portfolio Top3 when at least two eligible candidates exist.

---

### Task 1: Add failing deployed-smoke regression tests

**Files:**
- Modify: `apps/backtest-engine/src/search.rs`
- Modify: `apps/backtest-worker/src/main.rs`
- Modify: `tests/verification/backtest_worker_contract.test.mjs`

- [ ] **Step 1: Add test proving long_short candidates can use asymmetric leg parameters**

Add to `apps/backtest-engine/src/search.rs` staged tests:

```rust
#[test]
fn long_short_staged_candidates_include_asymmetric_leg_parameters() {
    let space = StagedMartingaleSearchSpace {
        leverage: vec![2],
        spacing_bps: vec![120, 240],
        order_multiplier: vec![1.10, 1.25],
        max_legs: vec![2, 3],
        take_profit_bps: vec![60, 120],
        tail_stop_bps: vec![2000, 3000],
        long_short_weight_pct: vec![(60, 40), (50, 50)],
    };

    let candidates = generate_staged_candidates_for_symbol("BTCUSDT", "long_short", &space, 256)
        .expect("long_short candidates should generate");

    assert!(candidates.iter().all(|candidate| {
        candidate.config.direction_mode == MartingaleDirectionMode::LongAndShort
            && candidate.config.strategies.len() == 2
            && candidate.config.strategies.iter().any(|s| s.direction == MartingaleDirection::Long)
            && candidate.config.strategies.iter().any(|s| s.direction == MartingaleDirection::Short)
    }));

    let has_asymmetric_spacing = candidates.iter().any(|candidate| {
        let long = candidate.config.strategies.iter().find(|s| s.direction == MartingaleDirection::Long).unwrap();
        let short = candidate.config.strategies.iter().find(|s| s.direction == MartingaleDirection::Short).unwrap();
        match (&long.spacing, &short.spacing) {
            (MartingaleSpacingModel::FixedPercent { step_bps: long_step }, MartingaleSpacingModel::FixedPercent { step_bps: short_step }) => long_step != short_step,
            _ => false,
        }
    });
    assert!(has_asymmetric_spacing, "long_short search must include different long/short spacing combinations");

    let has_asymmetric_tp = candidates.iter().any(|candidate| {
        let long = candidate.config.strategies.iter().find(|s| s.direction == MartingaleDirection::Long).unwrap();
        let short = candidate.config.strategies.iter().find(|s| s.direction == MartingaleDirection::Short).unwrap();
        match (&long.take_profit, &short.take_profit) {
            (MartingaleTakeProfitModel::Percent { bps: long_tp }, MartingaleTakeProfitModel::Percent { bps: short_tp }) => long_tp != short_tp,
            _ => false,
        }
    });
    assert!(has_asymmetric_tp, "long_short search must include different long/short take-profit combinations");
}
```

Expected before implementation: FAIL because current `build_long_short_candidate()` gives both legs identical spacing, multiplier, TP, max legs, and stop.

- [ ] **Step 2: Add worker regression test for smoke-like search diversity**

Add to `apps/backtest-worker/src/main.rs` tests:

```rust
#[test]
fn long_short_smoke_payload_expands_to_diverse_dual_leg_candidates() {
    let task = WorkerTaskConfig {
        symbols: vec!["BTCUSDT".to_owned(), "ETHUSDT".to_owned()],
        direction_mode: Some("long_short".to_owned()),
        risk_profile: "balanced".to_owned(),
        random_candidates: 16,
        intelligent_rounds: 1,
        per_symbol_top_n: 10,
        search_space: Some(serde_json::json!({
            "leverage": [2],
            "spacing_bps": [120],
            "order_multiplier": [1.25],
            "max_legs": [3],
            "take_profit_bps": [60],
            "tail_stop_bps": [2000],
            "long_short_weight_pct": [[60, 40], [50, 50]]
        })),
        ..WorkerTaskConfig::default()
    };

    let staged = StagedMartingaleSearchSpace::for_profile("balanced", "long_short");
    let candidates = generate_long_short_candidates_for_task("BTCUSDT", &task, &staged)
        .expect("smoke candidates should generate");

    assert!(candidates.len() >= 16);
    assert!(candidates.iter().all(|candidate| {
        candidate.config.direction_mode == MartingaleDirectionMode::LongAndShort
            && candidate.config.strategies.len() == 2
    }));

    let spacing_pairs: std::collections::BTreeSet<(u32, u32)> = candidates.iter().filter_map(|candidate| {
        let long = candidate.config.strategies.iter().find(|s| s.direction == MartingaleDirection::Long)?;
        let short = candidate.config.strategies.iter().find(|s| s.direction == MartingaleDirection::Short)?;
        match (&long.spacing, &short.spacing) {
            (MartingaleSpacingModel::FixedPercent { step_bps: long_step }, MartingaleSpacingModel::FixedPercent { step_bps: short_step }) => Some((*long_step, *short_step)),
            _ => None,
        }
    }).collect();

    assert!(spacing_pairs.len() >= 8, "expected diverse long/short spacing pairs, got {spacing_pairs:?}");
    assert!(spacing_pairs.iter().any(|(long_step, short_step)| long_step != short_step));
}
```

Expected before implementation: FAIL or too weak because current interleaving selects only identical paired-parameter candidates.

- [ ] **Step 3: Strengthen Node contract against fake success**

Append to `tests/verification/backtest_worker_contract.test.mjs`:

```js
test("long_short worker rejects negative-only smoke instead of reporting success", () => {
  const worker = readFileSync("apps/backtest-worker/src/main.rs", "utf8");
  assert.match(worker, /no martingale candidates selected: direction_mode=\{\}/);
  assert.match(worker, /negative_return=\{\}/);
  assert.doesNotMatch(worker, /single_direction_candidates|LongOnly|ShortOnly.*fallback/);
});
```

- [ ] **Step 4: Run tests and confirm failure**

Run:

```bash
cargo test -p backtest-engine long_short_staged_candidates_include_asymmetric_leg_parameters -- --nocapture
cargo test -p backtest-worker long_short_smoke_payload_expands_to_diverse_dual_leg_candidates -- --nocapture
node tests/verification/backtest_worker_contract.test.mjs
```

Expected: the first two tests fail before implementation, proving the regression is captured.

- [ ] **Step 5: Commit failing tests**

```bash
git add apps/backtest-engine/src/search.rs apps/backtest-worker/src/main.rs tests/verification/backtest_worker_contract.test.mjs
git commit -m "test: 问题描述 锁定马丁多空烟测负收益回归"
```

---

### Task 2: Generate true dual-leg asymmetric candidates

**Files:**
- Modify: `apps/backtest-engine/src/search.rs`
- Modify: `apps/backtest-worker/src/main.rs`

- [ ] **Step 1: Replace single-parameter long_short builder with leg-parameter builder**

In `apps/backtest-engine/src/search.rs`, add this helper next to `build_long_short_candidate()`:

```rust
fn build_long_short_candidate_from_legs(
    symbol: &str,
    leverage: u32,
    long_params: LegParameters,
    short_params: LegParameters,
    id_counter: &mut usize,
) -> Result<SearchCandidate, String> {
    let market = if leverage > 1 { MartingaleMarketKind::UsdMFutures } else { MartingaleMarketKind::Spot };
    let (margin_mode, leverage_val) = match market {
        MartingaleMarketKind::Spot => (None, None),
        MartingaleMarketKind::UsdMFutures => (Some(MartingaleMarginMode::Isolated), Some(leverage)),
    };

    let long_strategy = strategy_from_leg_params(
        symbol,
        MartingaleDirection::Long,
        market,
        margin_mode,
        leverage_val,
        long_params,
        *id_counter,
    )?;
    let short_strategy = strategy_from_leg_params(
        symbol,
        MartingaleDirection::Short,
        market,
        margin_mode,
        leverage_val,
        short_params,
        *id_counter,
    )?;

    *id_counter += 1;
    let config = MartingalePortfolioConfig {
        direction_mode: MartingaleDirectionMode::LongAndShort,
        strategies: vec![long_strategy, short_strategy],
        risk_limits: MartingaleRiskLimits::default(),
    };
    config.validate()?;
    Ok(SearchCandidate {
        candidate_id: format!("staged-cand-{}", *id_counter),
        config,
    })
}

fn strategy_from_leg_params(
    symbol: &str,
    direction: MartingaleDirection,
    market: MartingaleMarketKind,
    margin_mode: Option<MartingaleMarginMode>,
    leverage: Option<u32>,
    params: LegParameters,
    id_counter: usize,
) -> Result<MartingaleStrategyConfig, String> {
    let multiplier = Decimal::from_f64_retain(params.order_multiplier)
        .ok_or_else(|| format!("invalid multiplier {}", params.order_multiplier))?;
    let first_order_quote = Decimal::new(100, 0) * Decimal::from(params.weight_pct) / Decimal::from(100u32);
    Ok(MartingaleStrategyConfig {
        strategy_id: format!("staged-{id_counter}-{direction:?}"),
        symbol: symbol.to_owned(),
        market,
        direction,
        direction_mode: MartingaleDirectionMode::LongAndShort,
        margin_mode,
        leverage,
        spacing: MartingaleSpacingModel::FixedPercent { step_bps: params.spacing_bps },
        sizing: MartingaleSizingModel::Multiplier {
            first_order_quote,
            multiplier,
            max_legs: params.max_legs,
        },
        take_profit: MartingaleTakeProfitModel::Percent { bps: params.take_profit_bps },
        stop_loss: Some(shared_domain::martingale::MartingaleStopLossModel::StrategyDrawdownPct {
            pct_bps: params.tail_stop_bps,
        }),
        indicators: Vec::new(),
        entry_triggers: Vec::new(),
        risk_limits: MartingaleRiskLimits::default(),
    })
}
```

- [ ] **Step 2: Generate asymmetric long/short combinations**

In `generate_staged_candidates_for_symbol()`, replace the current `long_short` branch with:

```rust
"long_short" | "long_and_short" => {
    for (long_weight_pct, short_weight_pct) in &space.long_short_weight_pct {
        let long_params = LegParameters {
            spacing_bps: *spacing_bps,
            order_multiplier: *multiplier,
            max_legs: *max_legs,
            take_profit_bps: *take_profit_bps,
            tail_stop_bps: *tail_stop_bps,
            weight_pct: *long_weight_pct,
        };

        for short_spacing_bps in &space.spacing_bps {
            for short_take_profit_bps in &space.take_profit_bps {
                let short_params = LegParameters {
                    spacing_bps: *short_spacing_bps,
                    order_multiplier: *multiplier,
                    max_legs: *max_legs,
                    take_profit_bps: *short_take_profit_bps,
                    tail_stop_bps: *tail_stop_bps,
                    weight_pct: *short_weight_pct,
                };
                candidates.push(build_long_short_candidate_from_legs(
                    symbol,
                    *leverage,
                    long_params.clone(),
                    short_params,
                    &mut id_counter,
                )?);
                if candidates.len() >= limit {
                    return Ok(candidates);
                }
            }
        }
    }
}
```

Keep `build_long_short_candidate()` only if existing tests still use it; otherwise remove it after all tests pass.

- [ ] **Step 3: Ensure staged search uses isolated futures**

Both single-direction and long_short staged candidates should use isolated futures when leverage is present. In `build_single_direction_candidate()` change:

```rust
Some(MartingaleMarginMode::Cross)
```

to:

```rust
Some(MartingaleMarginMode::Isolated)
```

- [ ] **Step 4: Re-run focused tests**

```bash
cargo test -p backtest-engine long_short_staged_candidates_include_asymmetric_leg_parameters -- --nocapture
cargo test -p backtest-worker long_short_smoke_payload_expands_to_diverse_dual_leg_candidates -- --nocapture
cargo test -p backtest-engine staged_search_space_covers_required_futures_ranges -- --nocapture
```

Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add apps/backtest-engine/src/search.rs apps/backtest-worker/src/main.rs
git commit -m "fix: 修复思路 生成真实马丁多空非对称候选"
```

---

### Task 3: Stop over-trading caused by every-bar re-entry

**Files:**
- Modify: `apps/backtest-engine/src/search.rs`
- Modify: `apps/backtest-engine/src/martingale/kline_engine.rs`
- Modify: `apps/backtest-worker/src/main.rs`

- [ ] **Step 1: Add test for sane trade count on cooldown-gated long_short sample**

Add to `apps/backtest-engine/src/martingale/kline_engine.rs` tests:

```rust
#[test]
fn long_short_cooldown_entry_trigger_prevents_every_bar_churn() {
    let bars = trending_bars("BTCUSDT", 1_672_531_200_000, 1_000, 20_000.0, 20_500.0);
    let mut portfolio = portfolio_with_direction(MartingaleDirection::Long, 10_000);
    portfolio.direction_mode = shared_domain::martingale::MartingaleDirectionMode::LongAndShort;

    let mut long_strategy = portfolio.strategies[0].clone();
    long_strategy.direction = MartingaleDirection::Long;
    long_strategy.direction_mode = shared_domain::martingale::MartingaleDirectionMode::LongAndShort;
    long_strategy.entry_triggers = vec![MartingaleEntryTrigger::Cooldown { seconds: 21_600 }];

    let mut short_strategy = long_strategy.clone();
    short_strategy.strategy_id = "short".to_owned();
    short_strategy.direction = MartingaleDirection::Short;
    short_strategy.entry_triggers = vec![MartingaleEntryTrigger::Cooldown { seconds: 21_600 }];

    portfolio.strategies = vec![long_strategy, short_strategy];

    let result = run_kline_screening(portfolio, &bars).expect("screening should run");
    assert!(result.metrics.trade_count > 0);
    assert!(result.metrics.trade_count < 400, "trade count should not churn every bar: {}", result.metrics.trade_count);
}
```

If `trending_bars()` does not exist, add this helper in the test module:

```rust
fn trending_bars(symbol: &str, start_ms: i64, count: usize, start_price: f64, end_price: f64) -> Vec<KlineBar> {
    (0..count).map(|index| {
        let t = index as f64 / (count.saturating_sub(1).max(1)) as f64;
        let close = start_price + (end_price - start_price) * t;
        KlineBar {
            symbol: symbol.to_owned(),
            interval: "1m".to_owned(),
            open_time_ms: start_ms + index as i64 * 60_000,
            open: Decimal::from_f64_retain(close).unwrap(),
            high: Decimal::from_f64_retain(close * 1.001).unwrap(),
            low: Decimal::from_f64_retain(close * 0.999).unwrap(),
            close: Decimal::from_f64_retain(close).unwrap(),
            volume: Decimal::new(1000, 0),
            close_time_ms: start_ms + index as i64 * 60_000 + 59_999,
        }
    }).collect()
}
```

- [ ] **Step 2: Add cooldown triggers to generated staged candidates**

In `strategy_from_leg_params()` and `build_single_direction_candidate()`, set:

```rust
entry_triggers: vec![MartingaleEntryTrigger::Cooldown { seconds: 21_600 }],
```

Use six hours as the initial smoke-safe default. This reduces fee churn while still allowing many cycles over 2023-01-01 to last month end.

- [ ] **Step 3: Expand smoke candidate cap without exploding runtime**

In `apps/backtest-worker/src/main.rs`, update `generate_long_short_candidates_for_task()`:

```rust
let requested_cap = task.random_candidates.max(1) * task.intelligent_rounds.max(1);
let cap = requested_cap.max(task.per_symbol_top_n.max(10) * 6).min(96);
let candidates = generate_staged_candidates_for_symbol(symbol, "long_short", &effective_staged, cap.max(128))?;
Ok(interleave_candidates_by_spacing(candidates, cap))
```

This keeps true dual-leg candidates but gives the smoke enough variety to find positives. Do not increase beyond `96` without a benchmark.

- [ ] **Step 4: Re-run tests**

```bash
cargo test -p backtest-engine long_short_cooldown_entry_trigger_prevents_every_bar_churn -- --nocapture
cargo test -p backtest-engine long_short_staged_candidates_include_asymmetric_leg_parameters -- --nocapture
cargo test -p backtest-worker long_short_smoke_payload_expands_to_diverse_dual_leg_candidates -- --nocapture
cargo test -p backtest-worker long_short_smoke_search_estimate_is_bounded -- --nocapture
```

Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add apps/backtest-engine/src/search.rs apps/backtest-engine/src/martingale/kline_engine.rs apps/backtest-worker/src/main.rs
git commit -m "fix: 修复思路 降低马丁多空回测交易磨损"
```

---

### Task 4: Preserve risk standards while allowing useful exploration

**Files:**
- Modify: `apps/backtest-worker/src/main.rs`

- [ ] **Step 1: Keep drawdown standards unchanged**

Confirm this function remains exactly:

```rust
fn long_short_drawdown_limit_sequence(risk_profile: &str) -> Vec<f64> {
    match risk_profile {
        "conservative" => vec![20.0, 25.0],
        "balanced" => vec![25.0, 30.0],
        "aggressive" => vec![30.0, 35.0],
        _ => vec![25.0, 30.0],
    }
}
```

- [ ] **Step 2: Add smoke acceptance unit test for no single-direction fallback**

Add to `apps/backtest-worker/src/main.rs` tests:

```rust
#[test]
fn long_short_candidate_generation_preserves_risk_standard_and_dual_direction() {
    assert_eq!(long_short_drawdown_limit_sequence("balanced"), vec![25.0, 30.0]);

    let task = WorkerTaskConfig {
        symbols: vec!["BTCUSDT".to_owned()],
        direction_mode: Some("long_short".to_owned()),
        risk_profile: "balanced".to_owned(),
        random_candidates: 16,
        intelligent_rounds: 1,
        per_symbol_top_n: 10,
        search_space: Some(serde_json::json!({
            "leverage": [2],
            "spacing_bps": [120],
            "order_multiplier": [1.25],
            "max_legs": [3],
            "take_profit_bps": [60],
            "tail_stop_bps": [2000],
            "long_short_weight_pct": [[60, 40], [50, 50]]
        })),
        ..WorkerTaskConfig::default()
    };

    let staged = StagedMartingaleSearchSpace::for_profile("balanced", "long_short");
    let candidates = generate_long_short_candidates_for_task("BTCUSDT", &task, &staged)
        .expect("candidates should generate");

    assert!(candidates.len() >= 30, "expected enough candidates for top10 selection, got {}", candidates.len());
    assert!(candidates.iter().all(|candidate| candidate.config.direction_mode == MartingaleDirectionMode::LongAndShort));
    assert!(candidates.iter().all(|candidate| candidate.config.strategies.iter().any(|s| s.direction == MartingaleDirection::Long)));
    assert!(candidates.iter().all(|candidate| candidate.config.strategies.iter().any(|s| s.direction == MartingaleDirection::Short)));
}
```

- [ ] **Step 3: Run worker tests**

```bash
cargo test -p backtest-worker long_short_candidate_generation_preserves_risk_standard_and_dual_direction -- --nocapture
cargo test -p backtest-worker -- --nocapture
node tests/verification/backtest_worker_contract.test.mjs
```

Expected: PASS.

- [ ] **Step 4: Commit**

```bash
git add apps/backtest-worker/src/main.rs tests/verification/backtest_worker_contract.test.mjs
git commit -m "test: 问题描述 锁定马丁多空风险标准与候选数量"
```

---

### Task 5: Full verification and Docker smoke

**Files:**
- No source changes unless tests fail.

- [ ] **Step 1: Full local verification**

Run:

```bash
cargo test -p backtest-engine -- --nocapture
cargo test -p backtest-worker -- --nocapture
cargo test -p api-server martingale -- --nocapture
node tests/verification/backtest_worker_contract.test.mjs
node tests/verification/backtest_console_contract.test.mjs
pnpm --filter web exec next build --webpack
```

Expected: all exit `0`.

- [ ] **Step 2: Docker build and restart only project services**

```bash
docker compose --env-file /home/bumblebee/Project/grid_binance/.worktrees/full-v1/.env \
  -f /home/bumblebee/Project/grid_binance/deploy/docker/docker-compose.yml \
  build api-server backtest-worker web

docker compose --env-file /home/bumblebee/Project/grid_binance/.worktrees/full-v1/.env \
  -f /home/bumblebee/Project/grid_binance/deploy/docker/docker-compose.yml \
  up -d --no-deps api-server backtest-worker web
```

Do not touch unrelated host `3000` service. This app remains exposed by nginx on host `8080`.

- [ ] **Step 3: Create exact smoke task**

Use Docker-network direct API or host `8080` if available:

```json
{
  "strategy_type": "martingale",
  "symbols": ["BTCUSDT", "ETHUSDT"],
  "direction": "long_short",
  "direction_mode": "long_short",
  "risk_profile": "balanced",
  "search_space": {
    "leverage": [2],
    "spacing_bps": [120],
    "order_multiplier": [1.25],
    "max_legs": [3],
    "take_profit_bps": [60],
    "tail_stop_bps": [2000],
    "long_short_weight_pct": [[60, 40], [50, 50]]
  }
}
```

- [ ] **Step 4: Validate database/API results**

Required acceptance:

- task status is `completed`, not `failed`.
- candidate count is at least `10`.
- every candidate summary direction is `long_short` or equivalent.
- no selected candidate direction is `long`, `short`, `LongOnly`, or `ShortOnly`.
- at least one candidate has `annualized_return_pct > 0`.
- selected candidates obey configured drawdown fallback: for balanced, selected `max_drawdown_pct <= 30`.
- `summary.portfolio_top3` exists when at least two eligible candidates exist.
- portfolio member count is at least `2`.
- result details include `equity_curve`, `drawdown_curve`, `trades`, `annualized_return_pct`, `leverage`.

Suggested SQL:

```sql
select count(*) as candidates,
       count(*) filter (where summary->>'direction' in ('long_short','LongShort','long+short')) as long_short_candidates,
       count(*) filter (where summary->>'direction' in ('long','short','LongOnly','ShortOnly')) as single_direction_candidates,
       count(*) filter (where (summary->>'annualized_return_pct')::numeric > 0) as positive_annualized_candidates,
       max((summary->>'max_drawdown_pct')::numeric) as max_selected_drawdown
from backtest_candidate_summaries
where task_id='<NEW_TASK_ID>';
```

Expected:

```text
candidates >= 10
long_short_candidates = candidates
single_direction_candidates = 0
positive_annualized_candidates >= 1
max_selected_drawdown <= 30
```

Portfolio SQL:

```sql
select jsonb_array_length(summary->'portfolio_top3') as portfolio_count,
       jsonb_array_length((summary->'portfolio_top3'->0)->'members') as first_member_count,
       ((summary->'portfolio_top3'->0)->>'annualized_return_pct')::numeric as first_annualized,
       ((summary->'portfolio_top3'->0)->>'max_drawdown_pct')::numeric as first_drawdown
from backtest_tasks
where task_id='<NEW_TASK_ID>';
```

Expected:

```text
portfolio_count >= 1
first_member_count >= 2
first_annualized > 0
first_drawdown <= 30
```

- [ ] **Step 5: If smoke still fails, do not merge**

If the smoke fails with all negative candidates again, record:

- task id
- payload
- task error
- top `rejection_diagnostics.best_by_return`
- top `rejection_diagnostics.lowest_drawdown`
- whether trade counts are still excessive

Then write the next repair plan. Do not pass by lowering standards, relaxing balanced drawdown beyond 30, or adding single-direction substitutes.

---

## Do Not Do

- Do not treat single-direction candidates as valid `long_short` results.
- Do not relax balanced drawdown beyond `[25, 30]`.
- Do not fabricate positive metrics.
- Do not hide failed smoke behind unit tests.
- Do not remove fees/slippage from simulation to create fake profitability.
- Do not modify unrelated services or host port `3000`.
