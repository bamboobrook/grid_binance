# P4 Cycle-Level Exit Mechanisms Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Implement two live-parity cycle-level exit mechanisms (`max_cycle_age_hours` + `regime_break_stop`) in backtest + trading-engine, then re-run the 2025 search to break the "2025 short revenue vs full-period survival" barrier.

**Architecture:** `regime_break_stop` = new `MartingaleStopLossModel::RegimeBreakStop{ema_period, drawdown_pct_bps}` SL variant (path 2 from spec — avoids expression-engine AND extension); `max_cycle_age_hours` = new `MartingaleRiskLimits` field (strategy-level). Both trigger via the existing `ExitDecision::StrategyStop` channel (no priority-struct change). Three-way parity: backtest `kline_engine.rs::triggered_stop` ↔ trading-engine `martingale_exit_signal` ↔ `live_parity_check`.

**Tech Stack:** Rust (shared-domain + backtest-engine + trading-engine crates), `cargo test`, existing `search_small_capital_martingale` + `portfolio_budget_replay` bins.

**Spec:** `docs/superpowers/specs/2026-06-29-p4-cycle-exit-mechanisms-design.md`

## Global Constraints
- TP must stay `Percent{bps}`; SL white-list after P4 = `None | StrategyDrawdownPct{..} | RegimeBreakStop{..}`.
- serde `rename_all = "snake_case"` everywhere in `martingale.rs`; new fields `#[serde(default)]`.
- `max_cycle_age_hours` is `Option<f64>` passed through directly (NOT via `resolve_threshold` — must default to disabled/None, not a default value).
- No change to `exit_rules.rs::evaluate_exit_priority` priority order.
- Every task ends with `cargo test -p <crate>` green + commit. Worktree isolation (created at execution time via using-git-worktrees).
- Verbatim metric gates (from ChatGPT plan §6): conservative 50/10, balanced 90/20, aggressive 110/30.

## File Structure
- **Modify** `crates/shared-domain/src/martingale.rs` — add SL variant + risk_limits field (Task 1)
- **Modify** `apps/backtest-engine/src/martingale/kline_engine.rs` — `StrategyRuntime.cycle_started_at_ms`, entry/reset_cycle wiring, `triggered_stop` two new branches (Tasks 2, 3)
- **Modify** `apps/trading-engine/src/martingale_runtime.rs` — `CycleState.started_at_ms`, `start_cycle` wiring (Task 4)
- **Modify** `apps/trading-engine/src/main.rs` — `martingale_exit_signal` signature+branches, `apply_martingale_market_ticks` wiring, reconcile cycle-start derivation (Tasks 4, 5)
- **Modify** `apps/trading-engine/src/martingale_exit.rs` — `martingale_regime_break_triggered` helper (Task 5)
- **Modify** `apps/backtest-engine/src/martingale/budget_replay.rs` — `live_parity_check` allow `RegimeBreakStop` (Task 6)
- **Modify** `apps/backtest-engine/src/bin/search_small_capital_martingale.rs` — emit `RegimeBreakStop` + `max_cycle_age_hours` candidates (Task 7)
- **Test** inline modules in `kline_engine.rs` + `apps/trading-engine/tests/martingale_runtime.rs`

---

### Task 1: Config schema (shared-domain)

**Files:**
- Modify: `crates/shared-domain/src/martingale.rs:97-106` (SL enum), `:145-170` (RiskLimits)
- Test: inline `#[cfg(test)] mod tests` in `martingale.rs`

**Interfaces:**
- Produces: `MartingaleStopLossModel::RegimeBreakStop { ema_period: u32, drawdown_pct_bps: u32 }`; `MartingaleRiskLimits.max_cycle_age_hours: Option<f64>`

- [ ] **Step 1: Write failing serde test** (append to the existing test mod in `martingale.rs`; if none exists, add `#[cfg(test)] mod tests { use super::*; ... }`)

```rust
#[test]
fn regime_break_stop_and_max_cycle_age_serde_roundtrip() {
    let json = r#"{
        "direction_mode": "long_only",
        "strategies": [{
            "strategy_id": "t", "symbol": "BTCUSDT", "market": "usd_m_futures",
            "direction": "long", "direction_mode": "long_only",
            "margin_mode": "isolated", "leverage": 5,
            "spacing": {"fixed_percent": {"step_bps": 180}},
            "sizing": {"multiplier": {"first_order_quote": "10", "multiplier": "1.5", "max_legs": 5}},
            "take_profit": {"percent": {"bps": 100}},
            "stop_loss": {"regime_break_stop": {"ema_period": 50, "drawdown_pct_bps": 1000}},
            "indicators": [], "entry_triggers": [{"immediate": {}}],
            "risk_limits": {"max_cycle_age_hours": 48.0}
        }],
        "risk_limits": {}
    }"#;
    let cfg: MartingalePortfolioConfig = serde_json::from_str(json).unwrap();
    let sl = cfg.strategies[0].stop_loss.as_ref().unwrap();
    assert_eq!(sl, &MartingaleStopLossModel::RegimeBreakStop { ema_period: 50, drawdown_pct_bps: 1000 });
    assert_eq!(cfg.strategies[0].risk_limits.max_cycle_age_hours, Some(48.0));
    // re-serialize keeps snake_case
    let re = serde_json::to_string(&cfg).unwrap();
    assert!(re.contains("\"regime_break_stop\""));
    assert!(re.contains("\"max_cycle_age_hours\""));
}

#[test]
fn risk_limits_default_omits_max_cycle_age() {
    let json = r#"{}"#;
    let rl: MartingaleRiskLimits = serde_json::from_str(json).unwrap();
    assert_eq!(rl.max_cycle_age_hours, None);
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `PATH=$HOME/.cargo/bin:$PATH cargo test -p shared-domain regime_break_stop_and_max_cycle_age -- --nocapture 2>&1 | tail -15`
Expected: FAIL — `RegimeBreakStop` variant / field does not exist (compile error).

- [ ] **Step 3: Add SL variant** — in `MartingaleStopLossModel` enum (`martingale.rs:97-106`), append before the closing brace:

```rust
    RegimeBreakStop {
        ema_period: u32,
        drawdown_pct_bps: u32,
    },
```

- [ ] **Step 4: Add risk_limits field** — in `MartingaleRiskLimits` (`:145-170`), append after `safety_skip_adx_threshold`:

```rust
    /// Force-close a cycle (market, whole-cycle) once it exceeds this many hours
    /// since leg-0 entry. `None` = disabled. Strategy-level guard.
    #[serde(default)]
    pub max_cycle_age_hours: Option<f64>,
```

- [ ] **Step 5: Run test to verify it passes**

Run: `PATH=$HOME/.cargo/bin:$PATH cargo test -p shared-domain regime_break_stop 2>&1 | tail -10 && PATH=$HOME/.cargo/bin:$PATH cargo test -p shared-domain risk_limits_default_omits 2>&1 | tail -5`
Expected: both PASS.

- [ ] **Step 6: Commit**

```bash
git add crates/shared-domain/src/martingale.rs
git commit -m "feat(martingale): add RegimeBreakStop SL variant + max_cycle_age_hours risk limit"
```

---

### Task 2: backtest `max_cycle_age_hours`

**Files:**
- Modify: `apps/backtest-engine/src/martingale/kline_engine.rs` — `StrategyRuntime` struct (`:753-765`), `reset_cycle` (`:818-826`), entry block (`:317-324`), `triggered_stop` (`:1352-1431`)
- Test: inline test mod in `kline_engine.rs`

**Interfaces:**
- Consumes: `MartingaleRiskLimits.max_cycle_age_hours` (Task 1)
- Produces: `StrategyRuntime.cycle_started_at_ms: Option<i64>`; cycle-age stop surfaces as `StopSignal{strategy_stop:true, price:Some(bar.close)}` → `ExitDecision::StrategyStop`

- [ ] **Step 1: Write failing test** (append to the `#[cfg(test)]` mod in `kline_engine.rs`; reuse existing test helpers for building a `StrategyRuntime` + `KlineBar` — look at an existing `fn ..._test()` in the file for the exact helper names like `make_bar`/`make_runtime`)

```rust
#[test]
fn max_cycle_age_force_closes_after_threshold() {
    // Build a long cycle opened at t0; advance bars so age > max_cycle_age_hours.
    // Assert triggered_stop returns strategy_stop=true with price=bar.close.
    // (Use the same runtime/bar builders as existing kline_engine tests.)
    let mut state = sample_long_runtime_with_leg();      // existing helper or inline
    state.cycle_started_at_ms = Some(0_i64);
    state.strategy.risk_limits.max_cycle_age_hours = Some(2.0); // 2h
    let bar = sample_bar_at_ms(2 * 3_600_000 + 1);        // 2h + 1ms later
    let mut ctx = IndicatorRuntimeContext::default();
    let closes = BTreeMap::from([(state.strategy.symbol.clone(), bar.close)]);
    let sig = triggered_stop(&state, &bar, std::slice::from_ref(&state), &closes, &mut ctx).unwrap();
    assert!(sig.strategy_stop);
    assert_eq!(sig.price, Some(bar.close));
}

#[test]
fn max_cycle_age_not_triggered_before_threshold() {
    let mut state = sample_long_runtime_with_leg();
    state.cycle_started_at_ms = Some(0_i64);
    state.strategy.risk_limits.max_cycle_age_hours = Some(2.0);
    let bar = sample_bar_at_ms(3_600_000); // only 1h
    let mut ctx = IndicatorRuntimeContext::default();
    let closes = BTreeMap::from([(state.strategy.symbol.clone(), bar.close)]);
    let sig = triggered_stop(&state, &bar, std::slice::from_ref(&state), &closes, &mut ctx).unwrap();
    assert!(!sig.strategy_stop);
}
```
NOTE: if `sample_long_runtime_with_leg` / `sample_bar_at_ms` helpers do not exist, inline their construction by copying the pattern from the nearest existing test in `kline_engine.rs` (look for a test that builds `StrategyRuntime` with a leg + a `KlineBar`). The test mod already imports the needed types.

- [ ] **Step 2: Run test to verify it fails**

Run: `PATH=$HOME/.cargo/bin:$PATH cargo test -p backtest-engine max_cycle_age 2>&1 | tail -15`
Expected: FAIL — compile error (`cycle_started_at_ms` field missing) or `strategy_stop` false.

- [ ] **Step 3: Add field + wiring**
  - In `StrategyRuntime` struct (`:753-765`), add field: `cycle_started_at_ms: Option<i64>,`
  - In `reset_cycle` (`:818-826`), add line: `self.cycle_started_at_ms = None;`
  - In the entry block, right after the `add_leg(&mut strategy_states[state_index], 0, ...)?;` call (`:317-324`), add:
    ```rust
                    strategy_states[state_index].cycle_started_at_ms = Some(bar.open_time_ms);
    ```
  - (Find every `StrategyRuntime { ... }` literal construction — there are several in tests + `run_kline_screening_with_funding` init; add `cycle_started_at_ms: None,` to each so it compiles. Use `grep -n "cycle_id:" apps/backtest-engine/src/martingale/kline_engine.rs` to locate struct literals.)

- [ ] **Step 4: Add age check at top of `triggered_stop`** (`:1352`), BEFORE the `let Some(stop_loss) = ...` early-return:

```rust
    // max_cycle_age_hours — strategy-level cycle-age guard (independent of stop_loss)
    if let Some(max_hours) = state.strategy.risk_limits.max_cycle_age_hours {
        if let Some(started) = state.cycle_started_at_ms {
            let age_hours = (bar.open_time_ms - started) as f64 / 3_600_000.0;
            if age_hours >= max_hours {
                return Ok(StopSignal {
                    strategy_stop: true,
                    price: Some(bar.close),
                    ..StopSignal::default()
                });
            }
        }
    }
```

- [ ] **Step 5: Run test to verify it passes**

Run: `PATH=$HOME/.cargo/bin:$PATH cargo test -p backtest-engine max_cycle_age 2>&1 | tail -10`
Expected: both PASS.

- [ ] **Step 6: Run full backtest-engine suite (regression)**

Run: `PATH=$HOME/.cargo/bin:$PATH cargo test -p backtest-engine 2>&1 | tail -15`
Expected: all PASS (no existing test broken by the new field/wiring).

- [ ] **Step 7: Commit**

```bash
git add apps/backtest-engine/src/martingale/kline_engine.rs
git commit -m "feat(backtest): max_cycle_age_hours cycle-age force-close"
```

---

### Task 3: backtest `regime_break_stop`

**Files:**
- Modify: `apps/backtest-engine/src/martingale/kline_engine.rs` — `triggered_stop` match (`:1352-1431`)
- Test: inline test mod in `kline_engine.rs`

**Interfaces:**
- Consumes: `MartingaleStopLossModel::RegimeBreakStop` (Task 1); `IndicatorRuntimeContext::latest_ema(&mut self, &str, usize) -> Option<f64>` (`indicator_runtime.rs:445`); `strategy_net_pnl` + `capital_used_quote`
- Produces: long: `close < ema && drawdown>=thr` → strategy_stop; short: `close > ema && drawdown>=thr`

- [ ] **Step 1: Write failing tests**

```rust
#[test]
fn regime_break_long_closes_when_close_below_ema_and_drawdown() {
    let mut state = sample_long_runtime_with_leg(); // entry above ema, now losing > thr
    state.strategy.stop_loss = Some(MartingaleStopLossModel::RegimeBreakStop {
        ema_period: 50, drawdown_pct_bps: 500, // 5%
    });
    // arrange: current close below ema50 AND drawdown >= 5%.
    // Use indicator_context seeded so latest_ema(symbol,50) returns Some(high_ema)
    // and latest_close_by_symbol / bar.close = low_close such that drawdown>=5%.
    let (bar, closes, mut ctx) = regime_break_long_fixture(&state.strategy.symbol); // helper
    let sig = triggered_stop(&state, &bar, std::slice::from_ref(&state), &closes, &mut ctx).unwrap();
    assert!(sig.strategy_stop);
}

#[test]
fn regime_break_not_triggered_when_close_above_ema() {
    let mut state = sample_long_runtime_with_leg();
    state.strategy.stop_loss = Some(MartingaleStopLossModel::RegimeBreakStop {
        ema_period: 50, drawdown_pct_bps: 500,
    });
    let (bar, closes, mut ctx) = regime_break_long_safe_fixture(&state.strategy.symbol); // close>ema
    let sig = triggered_stop(&state, &bar, std::slice::from_ref(&state), &closes, &mut ctx).unwrap();
    assert!(!sig.strategy_stop);
}

#[test]
fn regime_break_short_closes_when_close_above_ema_and_drawdown() {
    let mut state = sample_short_runtime_with_leg();
    state.strategy.stop_loss = Some(MartingaleStopLossModel::RegimeBreakStop {
        ema_period: 50, drawdown_pct_bps: 500,
    });
    let (bar, closes, mut ctx) = regime_break_short_fixture(&state.strategy.symbol); // close>ema, short losing
    let sig = triggered_stop(&state, &bar, std::slice::from_ref(&state), &closes, &mut ctx).unwrap();
    assert!(sig.strategy_stop);
}
```
NOTE: `regime_break_*_fixture` helpers — build a `KlineBar` + `BTreeMap<String,f64>` closes + an `IndicatorRuntimeContext` with enough bars that `latest_ema(symbol,50)` returns a known value. Copy the seeding pattern from existing `indicator_runtime.rs` tests (`:978-1004` show how bars are fed to populate EMA). If wiring a full EMA cache in-test is heavy, an acceptable alternative is to extend `IndicatorRuntimeContext` test surface minimally — but prefer feeding real bars so the EMA is genuine (parity-relevant).

- [ ] **Step 2: Run tests to verify they fail**

Run: `PATH=$HOME/.cargo/bin:$PATH cargo test -p backtest-engine regime_break 2>&1 | tail -15`
Expected: FAIL (branch not implemented → falls through, strategy_stop false / compile err on fixtures).

- [ ] **Step 3: Add match arm in `triggered_stop`** (insert into the `match stop_loss { ... }`, e.g. after the `StrategyDrawdownPct` arm):

```rust
        MartingaleStopLossModel::RegimeBreakStop {
            ema_period,
            drawdown_pct_bps,
        } => {
            let invested = state.capital_used_quote();
            if invested <= 0.0 {
                return Ok(StopSignal::default());
            }
            let Some(ema) =
                indicator_context.latest_ema(&state.strategy.symbol, *ema_period as usize)
            else {
                return Ok(StopSignal::default()); // EMA warmup — do not trigger
            };
            let current_price = latest_close_by_symbol
                .get(&state.strategy.symbol)
                .copied()
                .unwrap_or(bar.close);
            let pnl = strategy_net_pnl(state, current_price)?;
            let drawdown_pct = (-pnl).max(0.0) / invested * 100.0;
            if drawdown_pct < *drawdown_pct_bps as f64 / 100.0 {
                return Ok(StopSignal::default()); // drawdown below threshold — do not trigger
            }
            let regime_broke = match state.strategy.direction {
                MartingaleDirection::Long => current_price < ema,
                MartingaleDirection::Short => current_price > ema,
            };
            Ok(StopSignal {
                strategy_stop: regime_broke,
                price: regime_broke.then_some(bar.close),
                ..StopSignal::default()
            })
        }
```

- [ ] **Step 4: Run tests to verify they pass**

Run: `PATH=$HOME/.cargo/bin:$PATH cargo test -p backtest-engine regime_break 2>&1 | tail -10`
Expected: all 3 PASS.

- [ ] **Step 5: Full backtest-engine regression**

Run: `PATH=$HOME/.cargo/bin:$PATH cargo test -p backtest-engine 2>&1 | tail -10`
Expected: all PASS.

- [ ] **Step 6: Commit**

```bash
git add apps/backtest-engine/src/martingale/kline_engine.rs
git commit -m "feat(backtest): regime_break_stop SL variant (close vs ema AND drawdown)"
```

---

### Task 4: trading-engine `max_cycle_age_hours`

**Files:**
- Modify: `apps/trading-engine/src/martingale_runtime.rs` — `CycleState` (`:122-127`), `start_cycle` (`:312-338`)
- Modify: `apps/trading-engine/src/main.rs` — `martingale_exit_signal` (`:1881-1931`), `apply_martingale_market_ticks` (`:1805-1873`), reconcile cycle-start derivation (`:685-721`)
- Test: `apps/trading-engine/tests/martingale_runtime.rs`

**Interfaces:**
- Consumes: `MartingaleRiskLimits.max_cycle_age_hours`; `MartingaleRuntimeContext.now_ms`; `strategy.runtime.events` (entry event `created_at`)
- Produces: `CycleState.started_at_ms: Option<i64>`; new `martingale_exit_signal` branch → `MartingaleExitSignal{event_type:"martingale_cycle_age_stop"}`

- [ ] **Step 1: Write failing test** in `apps/trading-engine/tests/martingale_runtime.rs`

```rust
#[test]
fn cycle_age_stop_triggers_when_age_exceeds_limit() {
    // Build a runtime with an open cycle started in the past (started_at_ms set),
    // risk_limits.max_cycle_age_hours = Some(1.0), now_ms = started + 2h.
    // Assert martingale_exit_signal returns Some with event_type "martingale_cycle_age_stop".
    // Follow the construction pattern of existing tests in this file (they build
    // MartingaleStrategyConfig + position + call martingale_exit_signal).
    let cfg = sample_long_config_with_max_cycle_age(1.0); // helper: config w/ risk_limits
    let pos = sample_position(/* avg entry, qty */);
    let now_ms = /* started + 2h in ms */;
    let cycle_started_at_ms = /* started ms */;
    let sig = martingale_exit_signal(
        &cfg, &pos, /*current_price*/, Decimal::ZERO, /*entry_fees*/,
        Some(now_ms), Some(cycle_started_at_ms), /*indicator_ctx: &mut ctx*/,
    );
    assert_eq!(sig.unwrap().event_type, "martingale_cycle_age_stop");
}
```
NOTE: this test pins the new `martingale_exit_signal` signature (extra `now_ms`, `cycle_started_at_ms`, `indicator_ctx` params — see Step 4). Existing tests calling the old signature will be updated in Step 4.

- [ ] **Step 2: Run test to verify it fails**

Run: `PATH=$HOME/.cargo/bin:$PATH cargo test -p trading-engine cycle_age_stop 2>&1 | tail -15`
Expected: FAIL — signature mismatch / not implemented.

- [ ] **Step 3: Add `started_at_ms` to `CycleState`** (`martingale_runtime.rs:122-127`):

```rust
#[derive(Debug, Clone)]
struct CycleState {
    cycle_id: String,
    anchor_price: Decimal,
    next_leg_index: u32,
    started_at_ms: Option<i64>,
}
```
In `start_cycle` (`:312-338`), set it where `CycleState { ... }` is constructed:
```rust
        self.strategy_mut(strategy_id)?.cycle = Some(CycleState {
            cycle_id: cycle_id.clone(),
            anchor_price,
            next_leg_index: 0,
            started_at_ms: context.now_ms,
        });
```
Find any other `CycleState { ... }` literal (`grep -n "CycleState {" apps/trading-engine/src/`) and add `started_at_ms: None,`.

- [ ] **Step 4: Extend `martingale_exit_signal` signature + age branch** (`main.rs:1881`). New signature:

```rust
fn martingale_exit_signal(
    config: &MartingaleStrategyConfig,
    position: &StrategyRuntimePosition,
    current_price: Decimal,
    realized_pnl: Decimal,
    entry_fees: Decimal,
    now_ms: Option<i64>,
    cycle_started_at_ms: Option<i64>,
    indicator_ctx: &mut IndicatorRuntimeContext,
) -> Option<MartingaleExitSignal> {
```
Add the age branch at the very top of the body (before TP), and add the regime branch after the existing SL-drawdown branch (Task 5 uses the same `indicator_ctx` param, so add the param now even if regime branch comes in Task 5):

```rust
    // max_cycle_age_hours
    if let Some(max_hours) = config.risk_limits.max_cycle_age_hours {
        if let (Some(now), Some(started)) = (now_ms, cycle_started_at_ms) {
            if ((now - started) as f64 / 3_600_000.0) >= max_hours {
                return Some(MartingaleExitSignal {
                    event_type: "martingale_cycle_age_stop",
                    label: "cycle age stop",
                    threshold_price: current_price,
                });
            }
        }
    }
```
Update the existing call site in `apply_martingale_market_ticks` (`:1835` area) to pass the new args:
```rust
        let now_ms = Some(chrono::Utc::now().timestamp_millis());
        let cycle_started_at_ms = martingale_cycle_started_at_ms(strategy);
        let mut indicator_ctx = persisted_indicator_context_for_strategy(strategy); // see Step 5
        let Some(exit) = martingale_exit_signal(
            &strategy_config,
            &position,
            tick.price,
            realized_pnl,
            entry_fees,
            now_ms,
            cycle_started_at_ms,
            &mut indicator_ctx,
        ) else { continue; };
```

- [ ] **Step 5: Add helpers** in `main.rs` (near `last_martingale_cycle_closed_at_ms` `:826-842` — copy its events-scan pattern but search for the cycle-start event):

```rust
fn martingale_cycle_started_at_ms(strategy: &Strategy) -> Option<i64> {
    strategy.runtime.events.iter()
        .find(|ev| ev.event_type == "entry" || ev.event_type == "martingale_cycle_started")
        .and_then(|ev| ev.created_at)
        .map(|dt| dt.timestamp_millis())
}

fn persisted_indicator_context_for_strategy(_strategy: &Strategy) -> IndicatorRuntimeContext {
    // Return the persisted indicator context (same object threaded through reconcile
    // at main.rs:588/592). If threading the live handle into apply_martingale_market_ticks
    // requires a signature change to its caller, do so (pass indicator_ctx down from the
    // reconcile/tick loop that already holds it). As a minimal first cut, construct an empty
    // context here and NOTE in a follow-up that regime_break live accuracy depends on the
    // real persisted context — but Task 5 MUST wire the real context for parity.
    IndicatorRuntimeContext::default()
}
```
IMPORTANT: `persisted_indicator_context_for_strategy` returning a default(empty) context means `regime_break_stop` live (Task 5) will see EMA=None → never trigger. Task 5 MUST replace this stub by threading the real persisted `indicator_ctx` (held in the reconcile caller) into `apply_martingale_market_ticks`. Leave a `// TODO(task5): wire real persisted indicator_ctx` marker.

- [ ] **Step 6: Run test + update existing callers**

Run: `PATH=$HOME/.cargo/bin:$PATH cargo test -p trading-engine cycle_age_stop 2>&1 | tail -15`
Expected: the new test PASSES; other tests that called the old `martingale_exit_signal` signature will fail to compile — update each call site to pass `Some(now), Some(started), &mut ctx` (test fixtures can use `None, None, &mut IndicatorRuntimeContext::default()` when they don't exercise age/regime).

- [ ] **Step 7: Full trading-engine regression**

Run: `PATH=$HOME/.cargo/bin:$PATH cargo test -p trading-engine 2>&1 | tail -15`
Expected: all PASS.

- [ ] **Step 8: Commit**

```bash
git add apps/trading-engine/src/martingale_runtime.rs apps/trading-engine/src/main.rs apps/trading-engine/tests/martingale_runtime.rs
git commit -m "feat(trading-engine): max_cycle_age_hours live parity"
```

---

### Task 5: trading-engine `regime_break_stop` + wire real indicator_ctx

**Files:**
- Modify: `apps/trading-engine/src/martingale_exit.rs` — add `martingale_regime_break_triggered` helper
- Modify: `apps/trading-engine/src/main.rs` — `martingale_exit_signal` regime branch; replace the `persisted_indicator_context_for_strategy` stub by threading real `indicator_ctx` into `apply_martingale_market_ticks`
- Test: `apps/trading-engine/tests/martingale_runtime.rs`

**Interfaces:**
- Consumes: `MartingaleStopLossModel::RegimeBreakStop`; real persisted `IndicatorRuntimeContext` (EMA values); `martingale_strategy_drawdown_pct` arithmetic
- Produces: `MartingaleExitSignal{event_type:"martingale_regime_break_stop"}`

- [ ] **Step 1: Write failing test**

```rust
#[test]
fn regime_break_stop_triggers_live_when_close_crosses_ema_and_drawdown() {
    // config.stop_loss = RegimeBreakStop{ema_period:50, drawdown_pct_bps:500}
    // position: long, losing > 5%; indicator_ctx seeded so latest_ema(symbol,50)=Some(high)
    // current_price < ema → expect event_type "martingale_regime_break_stop"
    let (cfg, pos, price, mut ctx) = regime_break_live_fixture();
    let sig = martingale_exit_signal(&cfg, &pos, price, Decimal::ZERO, entry_fees_for(&pos),
        Some(now_ms), Some(started_ms), &mut ctx).unwrap();
    assert_eq!(sig.event_type, "martingale_regime_break_stop");
}
```

- [ ] **Step 2: Run to verify fail**

Run: `PATH=$HOME/.cargo/bin:$PATH cargo test -p trading-engine regime_break_stop_triggers_live 2>&1 | tail -15`
Expected: FAIL.

- [ ] **Step 3: Add helper in `martingale_exit.rs`** (compute drawdown the same way `martingale_strategy_drawdown_pct` does, plus regime check):

```rust
pub fn martingale_regime_break_triggered(
    config: &MartingaleStrategyConfig,
    quantity: Decimal,
    average_entry_price: Decimal,
    current_price: Decimal,
    realized_pnl: Decimal,
    entry_fees: Decimal,
    indicator_ctx: &mut IndicatorRuntimeContext,
) -> Option<bool> {
    let (ema_period, dd_bps) = match &config.stop_loss {
        Some(MartingaleStopLossModel::RegimeBreakStop { ema_period, drawdown_pct_bps }) => (*ema_period, *drawdown_pct_bps),
        _ => return None,
    };
    let dd = martingale_strategy_drawdown_pct(config, quantity, average_entry_price, current_price, realized_pnl, entry_fees)?;
    if dd < dd_bps as f64 / 100.0 { return Some(false); }
    let ema = indicator_ctx.latest_ema(&config.symbol, ema_period as usize)?;
    let price = current_price.to_f64()?;
    let broke = match config.direction {
        MartingaleDirection::Long => price < ema,
        MartingaleDirection::Short => price > ema,
    };
    Some(broke)
}
```
(Add `use backtest_engine::martingale::indicator_runtime::IndicatorRuntimeContext;` import in `martingale_exit.rs` if not present — it's in the same workspace; check how `martingale_runtime.rs` already imports it.)

- [ ] **Step 4: Add regime branch in `martingale_exit_signal`** (after the existing strategy-drawdown branch, before final `None`):

```rust
    if let Some(true) = martingale_regime_break_triggered(
        config, position.quantity, position.average_entry_price,
        current_price, realized_pnl, entry_fees, indicator_ctx,
    ) {
        return Some(MartingaleExitSignal {
            event_type: "martingale_regime_break_stop",
            label: "regime break stop",
            threshold_price: current_price,
        });
    }
```

- [ ] **Step 5: Replace the Task-4 indicator_ctx stub** — thread the real persisted `IndicatorRuntimeContext` from the reconcile caller into `apply_martingale_market_ticks`. Change `apply_martingale_market_ticks` signature to accept `indicator_ctx: &mut IndicatorRuntimeContext` (or `&IndicatorRuntimeContext` if `latest_ema`'s `&mut self` can be relaxed by pre-caching EMAs during reconcile — preferred: pre-cache the needed EMA periods during reconcile so the tick path can use `&`). Update the caller (the reconcile/tick loop that already holds `indicator_ctx`) to pass it. Remove the `persisted_indicator_context_for_strategy` stub (or keep it only as a test default).

- [ ] **Step 6: Run test + full regression**

Run: `PATH=$HOME/.cargo/bin:$PATH cargo test -p trading-engine regime_break_stop_triggers_live 2>&1 | tail -10 && PATH=$HOME/.cargo/bin:$PATH cargo test -p trading-engine 2>&1 | tail -15`
Expected: PASS.

- [ ] **Step 7: Commit**

```bash
git add apps/trading-engine/src/martingale_exit.rs apps/trading-engine/src/main.rs apps/trading-engine/tests/martingale_runtime.rs
git commit -m "feat(trading-engine): regime_break_stop live parity + wire persisted indicator_ctx"
```

---

### Task 6: `live_parity_check` allow RegimeBreakStop + wire gate into search/publish (P7)

**Files:**
- Modify: `apps/backtest-engine/src/martingale/budget_replay.rs:533-563` (`live_parity_check`)
- Modify: `apps/backtest-engine/src/bin/search_small_capital_martingale.rs` — call `live_parity_check` before writing output
- Modify: `apps/api-server/src/services/martingale_publish_service.rs` — call `live_parity_check` on publish (locate the existing publish path)
- Test: inline test in `budget_replay.rs`

**Interfaces:**
- Consumes: `MartingaleStopLossModel::RegimeBreakStop` (now live-implemented in Task 5)
- Produces: `live_parity_check` passes portfolios using `RegimeBreakStop`; search/publish reject non-parity portfolios

- [ ] **Step 1: Write failing test** in `budget_replay.rs` test mod

```rust
#[test]
fn live_parity_allows_regime_break_stop() {
    let mut cfg = sample_portfolio_config(); // existing helper or inline minimal config
    cfg.strategies[0].stop_loss = Some(MartingaleStopLossModel::RegimeBreakStop { ema_period: 50, drawdown_pct_bps: 1000 });
    let out = live_parity_check(&cfg);
    assert!(out.passes, "violations: {:?}", out.violations);
}

#[test]
fn live_parity_still_rejects_indicator_sl() {
    let mut cfg = sample_portfolio_config();
    cfg.strategies[0].stop_loss = Some(MartingaleStopLossModel::Indicator { expression: "close<ema(50)".into() });
    assert!(!live_parity_check(&cfg).passes);
}
```

- [ ] **Step 2: Run to verify fail**

Run: `PATH=$HOME/.cargo/bin:$PATH cargo test -p backtest-engine live_parity_allows_regime 2>&1 | tail -10`
Expected: FAIL (RegimeBreakStop still in the `Some(other) => violation` arm).

- [ ] **Step 3: Allow the variant** — in `live_parity_check` SL match (`:546-548`), add the arm:

```rust
            Some(MartingaleStopLossModel::StrategyDrawdownPct { .. }) => true,
            Some(MartingaleStopLossModel::RegimeBreakStop { .. }) => true,
            Some(other) => { /* existing violation push */ false }
```

- [ ] **Step 4: Wire `live_parity_check` into search** — in `search_small_capital_martingale.rs`, before writing the `SearchReport` (after `rows` are finalized, near the `serde_json::to_writer` call), add:

```rust
    let parity = live_parity_check(&portfolio_for_top_rows(&rows)); // or check each candidate row's implied config
    eprintln!("live_parity passes={} violations={:?}", parity.passes, parity.violations);
```
NOTE: search always emits `Percent` TP + `StrategyDrawdownPct`/`RegimeBreakStop` SL, so parity should pass. The point is to wire the call so future non-parity experiments are caught. If wiring per-candidate is heavy, at minimum call it once on a representative portfolio and log. (Locate the `SearchReport` serialization via `grep -n "SearchReport\|serde_json::to" apps/backtest-engine/src/bin/search_small_capital_martingale.rs`.)

- [ ] **Step 5: Wire into publish** — in `martingale_publish_service.rs`, find the publish/validate path (`grep -n "fn publish\|validate" apps/api-server/src/services/martingale_publish_service.rs`) and add a `live_parity_check(&config)` gate that rejects publish when `!passes` (return an error listing violations). Follow the existing error-return pattern in that file.

- [ ] **Step 6: Run tests + regression**

Run: `PATH=$HOME/.cargo/bin:$PATH cargo test -p backtest-engine live_parity 2>&1 | tail -10 && PATH=$HOME/.cargo/bin:$PATH cargo build -p backtest-engine --bin search_small_capital_martingale --release 2>&1 | tail -5 && PATH=$HOME/.cargo/bin:$PATH cargo build -p api-server 2>&1 | tail -5`
Expected: tests PASS; both bins/crates compile.

- [ ] **Step 7: Commit**

```bash
git add apps/backtest-engine/src/martingale/budget_replay.rs apps/backtest-engine/src/bin/search_small_capital_martingale.rs apps/api-server/src/services/martingale_publish_service.rs
git commit -m "feat(parity): allow RegimeBreakStop in live_parity_check + wire gate into search/publish"
```

---

### Task 7: Re-run 2025 search with new mechanisms + full-period validation

**Files:**
- Modify: `apps/backtest-engine/src/bin/search_small_capital_martingale.rs` — add `regime_break` + `max_cycle_age` to the param grid + `--regime-break`/`--max-cycle-age` CLI flags (follow the existing `--entry-filters` pattern at `:390-453`)
- Run: search + segment validation

**Interfaces:**
- Consumes: P4 mechanisms (Tasks 1-6)
- Produces: a refreshed search report + segment-validation JSON comparing with/without regime_break

- [ ] **Step 1: Extend the search param grid** — in `search_small_capital_martingale.rs`, add to the `Param` struct + grid: `regime_break_ema_period: Option<u32>` (None / 50 / 100) and `max_cycle_age_hours: Option<f64>` (None / 24 / 48 / 72 / 120 / 168). When set, the generated `stop_loss` becomes `RegimeBreakStop{..}` and `risk_limits.max_cycle_age_hours` is populated (follow `build_portfolio`/`strategy` at `:748-844`). Add CLI flags `--regime-break ema50,ema100,none` and `--max-cycle-age none,24,48,72,120,168`.

- [ ] **Step 2: Build + smoke run**

Run: `PATH=$HOME/.cargo/bin:$PATH cargo build -p backtest-engine --bin search_small_capital_martingale --release 2>&1 | tail -5`
Then a 1-symbol smoke test:
```bash
./target/release/search_small_capital_martingale \
  --budgets 3000 --symbols DOTUSDT --direction-modes short_only,long_and_short \
  --entry-filters rsi_moderate,bb_moderate \
  --regime-break ema50,ema100,none --max-cycle-age 48,120,none \
  --start-ms 1735689600000 --end-ms 1767225599999 \
  --market-data data/market_data_full.db --funding-data data/funding_rates.db \
  --output /tmp/p4_smoke.json --top-n 10 --grid small --max-params-per-symbol-budget 20
```
Expected: runs, output contains some candidates with `regime_break_stop` SL + `max_cycle_age_hours` set.

- [ ] **Step 3: Full 2025 search (background)** — same command pattern as P2 but with `--regime-break ema50,ema100,none --max-cycle-age 24,72,168,none` and the crash symbols (BCH/DOT/APT/ETC/NEAR/COMP) + long_and_short, output to `docs/superpowers/artifacts/glm-p0-search/screen/2025_p4_3000.json`.

- [ ] **Step 4: Full-period segment validation** — run `scripts/validate_2025_single_strategy_segments.py` (extend it if needed to surface `regime_break`/`max_cycle_age` params) on the new search output, focus on whether `regime_break` + `age` let short/long_and_short candidates survive 2023 H1 (H1 return no longer catastrophic, full ann trends toward balanced 90/DD 20, segment gate passes).

- [ ] **Step 5: Record findings** — write `docs/superpowers/reports/2026-06-29-p4-search-findings.md` with: before/after comparison (P2 vs P4) on the same crash symbols, Pareto front, whether any candidate reaches conservative/balanced/aggressive gate, and the verdict (breakthrough achieved / still blocked → per ChatGPT plan §7 failure proof).

- [ ] **Step 6: Commit**

```bash
git add apps/backtest-engine/src/bin/search_small_capital_martingale.rs scripts/validate_2025_single_strategy_segments.py
git commit -m "feat(search): regime_break + max_cycle_age params; P4 re-search"
# report committed separately (note: search artifacts gitignored)
```

---

## Self-Review

**Spec coverage:** spec §2 (max_cycle_age) → Task 2 + 4; spec §3 (regime_break) → Task 3 + 5; spec §4 (priority) → uses existing StrategyStop (Tasks 2/3, no exit_rules change — matches §4 "不改优先级"); spec §5 (3-way parity) → Task 6 + cross-task; spec §6 (TDD tests) → each task has failing-test-first; spec §7 (impl order) → Tasks 1→7; spec §8 (search params + acceptance) → Task 7. ✓

**Placeholder scan:** Test helpers (`sample_long_runtime_with_leg`, `regime_break_*_fixture`, `sample_portfolio_config`) reference existing patterns in the same test files — each note tells the implementer to copy the nearest existing test's construction. No "TODO implement" without a code block. The one explicit `// TODO(task5)` marker is intentional hand-off between Task 4 and Task 5 (Task 5 removes it). ✓

**Type consistency:** `RegimeBreakStop{ema_period:u32, drawdown_pct_bps:u32}` consistent across Task 1 (def), 3 (backtest match), 5 (live helper), 6 (parity arm). `cycle_started_at_ms` (backtest) / `started_at_ms` (live) — distinct names by design (different structs); both `Option<i64>`. `max_cycle_age_hours: Option<f64>` consistent Task 1/2/4. `MartingaleExitSignal` new event_types `"martingale_cycle_age_stop"` / `"martingale_regime_break_stop"` consistent Task 4/5. ✓

## Execution Handoff

Plan saved to `docs/superpowers/plans/2026-06-29-p4-cycle-exit-mechanisms.md`. Two execution options:

**1. Subagent-Driven (recommended)** — I dispatch a fresh subagent per task, review between tasks, fast iteration. Best for this 7-task Rust plan with cross-task type dependencies.

**2. Inline Execution** — I execute tasks in this session via executing-plans, batch with checkpoints.

Which approach? (Either way, create the worktree first via superpowers:using-git-worktrees before Task 1.)
