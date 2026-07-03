# GLM Martingale Core Round 7 Cost-Aware Gap Repair Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Repair Round 1-6 evidence gaps, then search only martingale/DCA/grid-native mechanisms that may improve the current ann/DD frontier without repeating failed branches.

**Architecture:** Round 7 is a gap-repair-first search. It keeps martingale as the primary engine: indicators, funding, premium, volatility, and regime signals may only control DCA entry, safety-order eligibility, cycle exit, sleeve weights, cooldown, quarantine, and cost gates. It adds a strict non-repeat registry so GLM cannot rerun old failed families without a new mechanism key.

**Tech Stack:** Rust backtest engine and trading engine, Python search scripts, `portfolio_budget_replay`, SQLite market/funding/premium data, JSON/JSONL artifacts under `docs/superpowers/artifacts/glm-martingale-core-round7/`.

---

## 0. Non-Negotiable Strategy Boundary

Round 7 must remain martingale/DCA/grid-core.

Allowed:

- Entry gates for martingale cycles.
- Safety-order gates, scale, freeze, taper, cancel, or cooldown.
- Partial TP, breakeven, trailing lock, and cycle exit rules.
- Symbol/direction quarantine.
- Funding/fee/slippage cost filters for martingale cycles.
- Portfolio weights across martingale sleeves.
- Cash allocation as risk budget.

Forbidden:

- Pure trend following.
- Pure breakout.
- Pure funding carry.
- Pure pair-neutral.
- Pure statistical arbitrage.
- Any strategy that opens non-martingale primary positions.

## 1. Current Baseline

Round 7 starts from these known frontiers:

| Label | Source | Ann | DD | Pos/5 | Use |
|---|---|---:|---:|---:|---|
| `R3-lowdd` | `r3-P1-best-cd11` | 34.5% | 17.8% | 4 | low-DD anchor |
| `R4-combo` | `r4-combo-best` | 34.7% | 17.7% | 4 | live-parity-ish anchor |
| `R5-fine` | `r5-fine-combo-best` | 49.93% | 26.3% | 4 | near-conservative ann |
| `R5-ANKR` | `r5-G-best-ANKRUSDT` | 59.5% | 32.1% | 4 | high-ann anchor |
| `R6-XRPQ` | `r6-C-best-quarantine` | 55.8% | 25.0% | 4 | best ann/DD after quarantine |
| `R6-QB` | `r6-QB-best` | 34.0% | 18.0% | 4 | best DD blend |

Original targets remain:

| Profile | Required ann | Required DD |
|---|---:|---:|
| Conservative | >50% | <=10% |
| Balanced | >90% | <=20% |
| Aggressive | >110% | <=30% |

Near-frontier gates for Round 7:

| Gate | Definition |
|---|---|
| `NF-conservative-dd` | ann >=50%, DD <=20%, pos >=4 |
| `NF-conservative-close` | ann >=45%, DD <=15%, pos >=4 |
| `NF-balanced-dd` | ann >=60%, DD <=20%, pos >=4 |
| `NF-aggressive-dd` | ann >=80%, DD <=30%, pos >=3 |
| `cost-frontier` | ann improves by >=5pp at no more than +2pp DD, or DD improves by >=5pp at no more than -5pp ann |

## 2. External Search Signals Used

External references are not strategy claims; they only motivate Round 7 mechanisms that stay inside martingale rules.

1. 3Commas DCA documentation describes DCA bots around base order, averaging/safety orders, target profit from average/base, multiple TP, and trailing TP. Round 7 maps this to cycle-entry, safety-order, and TP-lock controls.
2. Binance funding documentation states funding is exchanged between long and short perpetual holders and depends on the funding rate/premium. Round 7 uses funding as a cost/side gate because Round 6 measured 677 USDT funding drag on a 5000U budget.
3. The 2025 Dynamic Grid Trading paper argues static grids have near-zero expected return under simple assumptions and improvement comes from dynamic adaptation. Round 7 tests only bounded dynamic reset/eligibility inside martingale DCA, not a standalone DGT strategy.

Sources:

- https://help.3commas.io/en/articles/3108940-dca-bot-interface-and-main-settings
- https://help.3commas.io/en/articles/3108981-how-take-profit-works-smarttrade-and-dca-bots-trailing-feature-explained
- https://www.binance.com/en/support/faq/detail/360033525031
- https://www.binance.com/en/academy/glossary/funding-fees
- https://arxiv.org/abs/2506.11921

## 3. Evidence Layout

Create:

- `docs/superpowers/artifacts/glm-martingale-core-round7/`
- `docs/superpowers/artifacts/glm-martingale-core-round7/promising/`
- `docs/superpowers/artifacts/glm-martingale-core-round7/rejected/`
- `docs/superpowers/artifacts/glm-martingale-core-round7/run-manifests/`
- `docs/superpowers/reports/2026-07-03-glm-martingale-round7-search-ledger.md`
- `docs/superpowers/reports/2026-07-03-glm-round7-handoff-to-chatgpt.md`

Registry:

`docs/superpowers/artifacts/glm-martingale-core-round7/exploration-registry.jsonl`

## 3.5 Branch And Startup Protocol

GLM must start from the latest clean remote state and create a dedicated Round 7 branch:

```bash
git fetch origin
git checkout glm-martingale-core-round6
git pull --ff-only
git checkout -b glm-martingale-core-round7
mkdir -p docs/superpowers/artifacts/glm-martingale-core-round7/promising docs/superpowers/artifacts/glm-martingale-core-round7/rejected docs/superpowers/artifacts/glm-martingale-core-round7/run-manifests
printf '# GLM Martingale Round 7 Cost-Aware Gap Repair Search Ledger\n\n' > docs/superpowers/reports/2026-07-03-glm-martingale-round7-search-ledger.md
touch docs/superpowers/artifacts/glm-martingale-core-round7/exploration-registry.jsonl
sha256sum data/market_data_full.db data/funding_rates.db data/premium_index.db > docs/superpowers/artifacts/glm-martingale-core-round7/run-manifests/r7-data-sha256.txt
git add docs/superpowers/artifacts/glm-martingale-core-round7 docs/superpowers/reports/2026-07-03-glm-martingale-round7-search-ledger.md
git commit -m "docs: 修复思路 初始化Round7马丁成本感知缺口修复台账"
git push -u origin glm-martingale-core-round7
```

If `data/premium_index.db` is absent, replace the `sha256sum` command with:

```bash
sha256sum data/market_data_full.db data/funding_rates.db > docs/superpowers/artifacts/glm-martingale-core-round7/run-manifests/r7-data-sha256.txt
printf 'premium_index.db: MISSING\n' >> docs/superpowers/artifacts/glm-martingale-core-round7/run-manifests/r7-data-sha256.txt
```

Each line must use this schema:

```json
{
  "exploration_id": "r7-A-detail-attribution-001",
  "started_at_utc": "2026-07-03T00:00:00Z",
  "family": "detailed_drawdown_attribution",
  "hypothesis": "Exact trough attribution identifies a repairable DCA control",
  "new_vs_prior": "Round 6 attribution had only segment summary and no symbol/leg state",
  "martingale_core": true,
  "research_only": false,
  "input_signature": "base=R5-ANKR|budget=5000|segments=full+5",
  "status": "running",
  "result_path": "docs/superpowers/artifacts/glm-martingale-core-round7/r7-detailed-dd-attribution.json"
}
```

On completion append another line with:

```json
{
  "exploration_id": "r7-A-detail-attribution-001",
  "completed_at_utc": "2026-07-03T00:00:00Z",
  "status": "completed",
  "decision": "route_found",
  "non_repeat_key": "detailed-dd-attribution-r5-ankr-v1",
  "result": "Top DD windows attributed to symbol/direction/leg/funding states; routes B/C/D/E/F selected"
}
```

## 4. Task A: Repair Round 1-6 Evidence And Non-Repeat Registry

**Goal:** Stop repeated exploration. Build one machine-readable registry of all failed, blocked, partial, and promising branches from Rounds 1-6.

**Files:**

- Create: `scripts/glm_r7_merge_non_repeat_registry.py`
- Create: `docs/superpowers/artifacts/glm-martingale-core-round7/round1-6-non-repeat-registry.json`
- Modify: `docs/superpowers/reports/2026-07-03-glm-martingale-round7-search-ledger.md`

- [ ] **Step A1: Create merge script**

The script must read these files:

```text
docs/superpowers/reports/2026-07-01-glm-martingale-core-search-ledger.md
docs/superpowers/artifacts/glm-martingale-core-round2/exploration-registry.jsonl
docs/superpowers/artifacts/glm-martingale-core-round3/exploration-registry.jsonl
docs/superpowers/artifacts/glm-martingale-core-round4/exploration-registry.jsonl
docs/superpowers/artifacts/glm-martingale-core-round5/exploration-registry.jsonl
docs/superpowers/artifacts/glm-martingale-core-round6/exploration-registry.jsonl
docs/superpowers/reports/2026-07-03-glm-round1-6-execution-audit.md
```

Output JSON shape:

```json
{
  "generated_at_utc": "2026-07-03T00:00:00Z",
  "blocked": [],
  "do_not_repeat": [
    {
      "key": "same-symbol-hedged-grid",
      "round": "R5",
      "reason": "400 candidates, ann 0-4.5%, hedge cancels return",
      "allowed_reopen_condition": "Only if hedge leg is itself a capped martingale rescue sleeve and primary PnL remains DCA/grid"
    }
  ],
  "partial_or_missing": [],
  "promising": []
}
```

- [ ] **Step A2: Run merge**

```bash
python3 scripts/glm_r7_merge_non_repeat_registry.py \
  --out docs/superpowers/artifacts/glm-martingale-core-round7/round1-6-non-repeat-registry.json
```

Expected:

- At least 20 `do_not_repeat` entries.
- At least these `partial_or_missing` keys: `r6-task-a-detail`, `r6-task-e-safety-freeze-engine`, `r6-task-f-dynamic-blend`, `r6-task-g-r5-r6-live-parity`, `r5-ankr-lowdd-result`.

- [ ] **Step A3: Commit**

```bash
git add scripts/glm_r7_merge_non_repeat_registry.py docs/superpowers/artifacts/glm-martingale-core-round7 docs/superpowers/reports/2026-07-03-glm-martingale-round7-search-ledger.md
git commit -m "docs: 修复思路 Round7合并前六轮马丁探索non-repeat台账"
```

## 5. Task B: Detailed Drawdown And Cost Attribution Repair

**Goal:** Repair the incomplete Round 6 Task A. This task must identify exact drawdown windows, symbol/direction contributions, cycle leg state, TP stage, funding, fee, and indicator state. Without this, GLM must not launch broad searches.

**Files:**

- Create: `scripts/glm_r7_detailed_dd_cost_attribution.py`
- Create: `docs/superpowers/artifacts/glm-martingale-core-round7/r7-detailed-dd-cost-attribution.json`
- Create: `docs/superpowers/reports/2026-07-03-glm-round7-detailed-dd-cost-attribution.md`
- Modify: `docs/superpowers/reports/2026-07-03-glm-martingale-round7-search-ledger.md`

- [ ] **Step B1: Run attribution for six anchors**

```bash
python3 scripts/glm_r7_detailed_dd_cost_attribution.py \
  --configs \
    R3-lowdd=docs/superpowers/artifacts/glm-martingale-core-round3/promising/r3-P1-best-cd11.json \
    R4-combo=docs/superpowers/artifacts/glm-martingale-core-round4/promising/r4-combo-best.json \
    R5-fine=docs/superpowers/artifacts/glm-martingale-core-round5/promising/r5-fine-combo-best.json \
    R5-ANKR=docs/superpowers/artifacts/glm-martingale-core-round5/promising/r5-G-best-ANKRUSDT.json \
    R6-XRPQ=docs/superpowers/artifacts/glm-martingale-core-round6/promising/r6-C-best-quarantine.json \
    R6-QB=docs/superpowers/artifacts/glm-martingale-core-round6/promising/r6-QB-best.json \
  --budget 5000 \
  --market-data data/market_data_full.db \
  --funding-data data/funding_rates.db \
  --premium-data data/premium_index.db \
  --out docs/superpowers/artifacts/glm-martingale-core-round7/r7-detailed-dd-cost-attribution.json \
  --report docs/superpowers/reports/2026-07-03-glm-round7-detailed-dd-cost-attribution.md
```

Required JSON fields per candidate:

```json
{
  "candidate": "R5-ANKR",
  "full": {"ann": 59.5, "dd": 32.1, "ret": 393.2},
  "dd_windows": [
    {
      "rank": 1,
      "peak_ts": 1700000000000,
      "trough_ts": 1700100000000,
      "recovery_ts": 1700200000000,
      "dd_pct": 15.7,
      "top_contributors": [
        {
          "strategy_id": "L2-ANKRUSDT",
          "symbol": "ANKRUSDT",
          "direction": "long",
          "realized_pnl_quote": -10.0,
          "unrealized_pnl_quote": -250.0,
          "funding_quote": -20.0,
          "fee_quote": -5.0,
          "filled_safety_legs": 5,
          "skipped_safety_legs": 0,
          "blocked_safety_legs": 0,
          "tp_stage_before_trough": 0,
          "entry_state": {
            "atr_pct": 1.2,
            "adx": 28.0,
            "rsi": 42.0,
            "roc720": 12.0,
            "premium": 0.0002,
            "funding_rate": 0.0001
          },
          "trough_state": {
            "atr_pct": 2.4,
            "adx": 38.0,
            "rsi": 31.0,
            "roc720": -9.0,
            "premium": -0.0001,
            "funding_rate": 0.0001
          }
        }
      ],
      "route": ["r7-C-cost-gate", "r7-F-safety-freeze"]
    }
  ],
  "cost_attribution": {
    "total_fee_quote": 310.0,
    "total_funding_quote": 677.0,
    "funding_by_symbol_direction": []
  }
}
```

- [ ] **Step B2: Route decision**

The report must contain this table:

```markdown
| candidate | largest DD route | top symbol/direction | funding drag % of budget | safety legs at trough | TP stage before trough | next task |
```

Route rules:

| Condition | Next task |
|---|---|
| funding drag >5% of budget or concentrated in one side | Task C |
| fees + funding consume >25% of gross profit | Task D |
| safety legs added after portfolio DD already above 10% | Task F |
| one symbol/direction dominates two or more top DD windows | Task E |
| DD is mainly startup exposure | Task G |
| candidate is promising but uses R5/R6/R7 backtest-only feature | Task H |

- [ ] **Step B3: Commit**

```bash
git add scripts/glm_r7_detailed_dd_cost_attribution.py docs/superpowers/artifacts/glm-martingale-core-round7/r7-detailed-dd-cost-attribution.json docs/superpowers/reports/2026-07-03-glm-round7-detailed-dd-cost-attribution.md docs/superpowers/reports/2026-07-03-glm-martingale-round7-search-ledger.md
git commit -m "docs: 修复思路 Round7补全马丁回撤与成本归因"
```

## 6. Task C: Funding And Fee Cost-Aware Martingale Gate

**Goal:** Round 6 found funding drag is material. Prior rounds rejected funding as a standalone return source; Round 7 must test it as a cost gate inside martingale cycle entry, safety-order eligibility, and side allocation.

**Files:**

- Modify: `crates/shared-domain/src/martingale.rs`
- Modify: `apps/backtest-engine/src/martingale/kline_engine.rs`
- Create: `scripts/glm_r7_cost_aware_martingale_gate_search.py`
- Test: `apps/backtest-engine/src/martingale/kline_engine.rs`
- Output: `docs/superpowers/artifacts/glm-martingale-core-round7/r7-cost-aware-gate-grid.json`

Add config fields under `MartingaleRiskLimits`:

```rust
#[serde(default)]
pub funding_cost_gate_window_hours: Option<f64>;
#[serde(default)]
pub max_expected_funding_cost_bps: Option<f64>;
#[serde(default)]
pub max_expected_fee_funding_share_pct: Option<f64>;
#[serde(default)]
pub block_safety_when_expected_cost_bps: Option<f64>;
#[serde(default)]
pub funding_side_bias_mode: Option<String>;
```

Allowed `funding_side_bias_mode` values:

| Value | Meaning |
|---|---|
| `disabled` | No funding gate |
| `entry_only` | Block new cycle if expected funding is adverse |
| `entry_and_safety` | Block new cycle and later safety orders if expected cost is adverse |
| `side_weight_scale` | Scale first order by funding favorability while staying within martingale sleeve |

- [ ] **Step C1: Add failing tests**

```bash
cargo test -p backtest-engine funding_cost_gate_blocks_adverse_new_cycle -- --nocapture
cargo test -p backtest-engine funding_cost_gate_blocks_later_safety_order -- --nocapture
cargo test -p backtest-engine funding_side_bias_scales_first_order_without_changing_direction -- --nocapture
```

- [ ] **Step C2: Implement and run tests**

```bash
cargo test -p backtest-engine funding_cost_gate -- --nocapture
```

- [ ] **Step C3: Run search**

```bash
python3 scripts/glm_r7_cost_aware_martingale_gate_search.py \
  --base-candidates R5-ANKR,R6-XRPQ,R5-fine,R4-combo,R3-lowdd \
  --budget 5000 \
  --market-data data/market_data_full.db \
  --funding-data data/funding_rates.db \
  --out docs/superpowers/artifacts/glm-martingale-core-round7/r7-cost-aware-gate-grid.json \
  --workers 20
```

Grid:

| Field | Values |
|---|---|
| funding window | `8h`, `24h`, `72h`, `168h` |
| max expected funding cost | `5`, `10`, `20`, `35`, `50` bps |
| max fee+funding share of expected TP | `10%`, `20%`, `35%`, `50%` |
| safety cost block | disabled, `10`, `20`, `35` bps |
| side bias mode | `entry_only`, `entry_and_safety`, `side_weight_scale` |

Reject if:

- ann drops by more than 8pp without reducing DD by at least 4pp.
- trade count drops below 30% of baseline.
- improvement exists only in h1_2023.

Promote if:

- `cost-frontier` passes, or
- any target/near-frontier gate passes.

- [ ] **Step C4: Commit**

```bash
git add crates/shared-domain apps/backtest-engine scripts/glm_r7_cost_aware_martingale_gate_search.py docs/superpowers/artifacts/glm-martingale-core-round7 docs/superpowers/reports/2026-07-03-glm-martingale-round7-search-ledger.md
git commit -m "feat: 修复思路 Round7增加马丁资金费率与手续费成本闸门"
```

## 7. Task D: Turnover Quality Gate And Cooldown Scheduler

**Goal:** Reduce cycles where expected gross TP is too small after fee, funding, and slippage. This is not a repeat of fixed cooldown; cooldown becomes a lagged cost/quality scheduler.

**Files:**

- Create: `scripts/glm_r7_turnover_quality_scheduler.py`
- Output: `docs/superpowers/artifacts/glm-martingale-core-round7/r7-turnover-quality-grid.json`

No engine change is required if the script can generate configs using existing cooldown, TP, safety condition, and Task C cost fields. If Task C fields are not implemented, this task waits.

- [ ] **Step D1: Run scheduler grid**

```bash
python3 scripts/glm_r7_turnover_quality_scheduler.py \
  --base-candidates R5-ANKR,R6-XRPQ,R5-fine,R4-combo \
  --cost-gate-artifact docs/superpowers/artifacts/glm-martingale-core-round7/r7-cost-aware-gate-grid.json \
  --out docs/superpowers/artifacts/glm-martingale-core-round7/r7-turnover-quality-grid.json \
  --workers 20
```

Grid:

| Field | Values |
|---|---|
| rolling lookback | `7d`, `14d`, `30d`, `60d` |
| min gross TP / cost ratio | `1.5`, `2.0`, `3.0`, `4.0` |
| cooldown if ratio fails | `12h`, `24h`, `72h` |
| cooldown if portfolio DD > threshold | disabled, `12h`, `24h`, `72h` |
| portfolio DD threshold | `8%`, `12%`, `16%`, `20%` |

Reject if it repeats fixed cooldown behavior:

- Same selected cooldown for more than 90% of cycles.
- No measurable cost reduction.

Promote if:

- fee+funding cost per 100U gross profit drops by at least 20%, and
- ann/DD satisfies `cost-frontier`.

- [ ] **Step D2: Commit**

```bash
git add scripts/glm_r7_turnover_quality_scheduler.py docs/superpowers/artifacts/glm-martingale-core-round7/r7-turnover-quality-grid.json docs/superpowers/reports/2026-07-03-glm-martingale-round7-search-ledger.md
git commit -m "docs: 修复思路 Round7搜索马丁周转质量与动态冷却"
```

## 8. Task E: Attribution-Driven Symbol/Direction Quarantine Completion

**Goal:** Complete the missing Round 6 quarantine scope. Do not rerun a blind quarantine grid. Run it only on candidates and scopes justified by Task B attribution.

**Files:**

- Modify if needed: `crates/shared-domain/src/martingale.rs`
- Modify if needed: `apps/backtest-engine/src/martingale/kline_engine.rs`
- Create: `scripts/glm_r7_attribution_quarantine_search.py`
- Output: `docs/superpowers/artifacts/glm-martingale-core-round7/r7-attribution-quarantine-grid.json`

- [ ] **Step E1: Verify existing quarantine tests**

```bash
cargo test -p backtest-engine quarantine -- --nocapture
```

If tests fail, repair before running any search.

- [ ] **Step E2: Run only routed scopes**

```bash
python3 scripts/glm_r7_attribution_quarantine_search.py \
  --attribution docs/superpowers/artifacts/glm-martingale-core-round7/r7-detailed-dd-cost-attribution.json \
  --base-candidates R5-ANKR,R6-XRPQ,R5-fine,R4-combo \
  --out docs/superpowers/artifacts/glm-martingale-core-round7/r7-attribution-quarantine-grid.json \
  --workers 20
```

Grid:

| Field | Values |
|---|---|
| scope | `symbol`, `symbol_direction`, `direction` |
| stop count trigger | `1`, `2`, `3` |
| stop count window | `24h`, `72h`, `168h`, `336h` |
| pause hours | `24`, `72`, `168`, `336` |
| recovery expression | disabled, `close > ema(50)`, `ema(50) > ema(200)`, `rsi(14) between 45 and 60` |
| candidates | only candidates routed by Task B |

Reject as repeat if:

- It runs only XRP with the same q1/w24/p24 style parameters from Round 6.
- It does not include at least one non-XRP routed candidate when attribution shows non-XRP concentration.

Promote if:

- ann >=50 and DD <=22, or
- DD improves by >=5pp while ann remains >=45 and pos >=4.

- [ ] **Step E3: Commit**

```bash
git add crates/shared-domain apps/backtest-engine scripts/glm_r7_attribution_quarantine_search.py docs/superpowers/artifacts/glm-martingale-core-round7 docs/superpowers/reports/2026-07-03-glm-martingale-round7-search-ledger.md
git commit -m "docs: 修复思路 Round7完成归因驱动的马丁隔离搜索"
```

## 9. Task F: Safety-Order Freeze, Cancel, And Taper Engine Repair

**Goal:** Complete the missing Round 6 Task E. This is the most important unfinished martingale-native risk control because Round 6 showed DD comes from existing unrealized positions and later safety behavior.

**Files:**

- Modify: `crates/shared-domain/src/martingale.rs`
- Modify: `apps/backtest-engine/src/martingale/kline_engine.rs`
- Create: `scripts/glm_r7_safety_freeze_taper_search.py`
- Test: `apps/backtest-engine/src/martingale/kline_engine.rs`
- Output: `docs/superpowers/artifacts/glm-martingale-core-round7/r7-safety-freeze-taper-grid.json`

Add or complete fields:

```rust
#[serde(default)]
pub freeze_safety_after_partial_tp_stage: Option<u32>;
#[serde(default)]
pub freeze_safety_when_portfolio_dd_pct: Option<f64>;
#[serde(default)]
pub cancel_unfilled_safety_after_partial_tp: Option<bool>;
#[serde(default)]
pub taper_safety_after_leg: Option<u32>;
#[serde(default)]
pub taper_safety_scale: Option<f64>;
#[serde(default)]
pub taper_safety_when_portfolio_dd_pct: Option<f64>;
```

Rules:

1. Freeze blocks future unfilled safety legs.
2. Cancel means no later simulated safety leg may fill after the trigger.
3. Taper scales future safety-order quote, not the already filled position.
4. Existing filled legs continue to TP/SL normally.
5. This must remain a martingale cycle; no opposite-position hedge is allowed.

- [ ] **Step F1: Add failing tests**

```bash
cargo test -p backtest-engine freeze_safety_after_partial_tp_blocks_later_legs -- --nocapture
cargo test -p backtest-engine freeze_safety_when_portfolio_dd_blocks_later_legs -- --nocapture
cargo test -p backtest-engine taper_safety_after_leg_scales_future_safety_orders -- --nocapture
cargo test -p backtest-engine taper_safety_does_not_change_existing_filled_legs -- --nocapture
```

- [ ] **Step F2: Implement and run tests**

```bash
cargo test -p backtest-engine freeze_safety -- --nocapture
cargo test -p backtest-engine taper_safety -- --nocapture
```

- [ ] **Step F3: Run search**

```bash
python3 scripts/glm_r7_safety_freeze_taper_search.py \
  --attribution docs/superpowers/artifacts/glm-martingale-core-round7/r7-detailed-dd-cost-attribution.json \
  --base-candidates R5-ANKR,R6-XRPQ,R5-fine,R4-combo \
  --out docs/superpowers/artifacts/glm-martingale-core-round7/r7-safety-freeze-taper-grid.json \
  --workers 20
```

Grid:

| Field | Values |
|---|---|
| freeze after TP stage | disabled, `0`, `1`, `2` |
| freeze at portfolio DD | disabled, `8%`, `12%`, `16%`, `20%` |
| cancel unfilled safety after TP | disabled, enabled |
| taper after leg | disabled, `2`, `3`, `4`, `5` |
| taper scale | `0.25`, `0.5`, `0.75` |
| taper at portfolio DD | disabled, `8%`, `12%`, `16%` |

Reject if:

- It produces identical results to baseline, proving fields are not wired.
- It reduces DD by closing profitable re-entry loops but ann falls below 35.

Promote if:

- `NF-conservative-dd` passes, or
- ann remains >=50 with DD <=22, or
- DD improves by >=6pp while ann remains >=45.

- [ ] **Step F4: Commit**

```bash
git add crates/shared-domain apps/backtest-engine scripts/glm_r7_safety_freeze_taper_search.py docs/superpowers/artifacts/glm-martingale-core-round7 docs/superpowers/reports/2026-07-03-glm-martingale-round7-search-ledger.md
git commit -m "feat: 修复思路 Round7实现马丁安全单冻结取消与递减"
```

## 10. Task G: Dynamic Cost/DD Risk-Budget Blend

**Goal:** Finish the missing dynamic half of Round 6 Task F. Static blends reduced DD but crushed ann. Dynamic blend must use only lagged realized evidence and cannot become statistical arbitrage.

**Files:**

- Create: `scripts/glm_r7_dynamic_cost_dd_blend.py`
- Output: `docs/superpowers/artifacts/glm-martingale-core-round7/r7-dynamic-cost-dd-blend.json`
- Report: `docs/superpowers/reports/2026-07-03-glm-round7-dynamic-blend.md`

- [ ] **Step G1: Build candidate curves**

```bash
python3 scripts/glm_r7_dynamic_cost_dd_blend.py \
  --mode build-curves \
  --candidates R3-lowdd,R4-combo,R5-fine,R5-ANKR,R6-XRPQ,R6-QB \
  --budget 5000 \
  --market-data data/market_data_full.db \
  --funding-data data/funding_rates.db \
  --out docs/superpowers/artifacts/glm-martingale-core-round7/r7-dynamic-cost-dd-blend-curves.json
```

- [ ] **Step G2: Run lagged allocator grid**

```bash
python3 scripts/glm_r7_dynamic_cost_dd_blend.py \
  --mode rolling-allocator \
  --curves docs/superpowers/artifacts/glm-martingale-core-round7/r7-dynamic-cost-dd-blend-curves.json \
  --out docs/superpowers/artifacts/glm-martingale-core-round7/r7-dynamic-cost-dd-blend.json \
  --report docs/superpowers/reports/2026-07-03-glm-round7-dynamic-blend.md \
  --workers 12
```

Grid:

| Field | Values |
|---|---|
| lookback | `14d`, `30d`, `60d`, `90d` |
| rebalance cadence | `7d`, `14d`, `30d` |
| max sleeve allocation | `20%`, `35%`, `50%`, `70%` |
| max ANKR allocation | `20%`, `35%`, `50%` |
| switch-to-cash DD trigger | disabled, `6%`, `10%`, `14%` |
| score | `ann_minus_dd`, `ret_over_dd`, `ret_minus_cost`, `ret_over_cost_and_dd` |

Anti-overfit rules:

- One fixed allocator parameter set must be used across all segments.
- No segment-specific hand selection.
- Report leave-one-segment-out results for h1_2023, h2_2023, 2024, 2025, 2026_ytd.

Reject if:

- It only improves h1_2023.
- It uses future curve data to choose the active sleeve.
- It is worse than the best static blend on ann/DD.

Promote if:

- ann >=50 and DD <=20, or
- ann >=45 and DD <=15, or
- any original target passes.

- [ ] **Step G3: Commit**

```bash
git add scripts/glm_r7_dynamic_cost_dd_blend.py docs/superpowers/artifacts/glm-martingale-core-round7 docs/superpowers/reports/2026-07-03-glm-round7-dynamic-blend.md docs/superpowers/reports/2026-07-03-glm-martingale-round7-search-ledger.md
git commit -m "docs: 修复思路 Round7执行马丁动态成本回撤预算组合"
```

## 11. Task H: Trading-Engine Parity Gate

**Goal:** Do not let backtest-only features become deployable. Implement parity for any feature used by a promoted Round 5-7 candidate.

**Files:**

- Modify: `apps/trading-engine/src/martingale_runtime.rs`
- Modify: `apps/trading-engine/src/main.rs`
- Modify: `apps/trading-engine/tests/martingale_runtime.rs`
- Create: `docs/superpowers/reports/2026-07-03-glm-round7-live-parity-report.md`

Mandatory parity list:

| Feature | Required if |
|---|---|
| last-executed SO basis | any R5/R6/R7 candidate is promoted |
| vol-target first-order scale | any R5/R6/R7 candidate uses vol-target |
| funding/fee cost gate | Task C promotes |
| quarantine | Task E promotes |
| safety freeze/cancel/taper | Task F promotes |
| dynamic blend | Task G promotes to live portfolio controller |

- [ ] **Step H1: Add targeted tests**

```bash
cargo test -p trading-engine martingale_last_executed_basis_uses_previous_fill_price -- --nocapture
cargo test -p trading-engine martingale_vol_target_scales_first_order_from_atr_context -- --nocapture
cargo test -p trading-engine martingale_funding_cost_gate_blocks_new_cycle -- --nocapture
cargo test -p trading-engine martingale_quarantine_blocks_live_new_cycle -- --nocapture
cargo test -p trading-engine martingale_freeze_safety_cancels_open_safety_orders -- --nocapture
```

- [ ] **Step H2: Implement required parity only for promoted features**

Run:

```bash
cargo test -p trading-engine martingale -- --nocapture
```

Report table:

```markdown
| Feature | backtest status | trading-engine status | test command | result | live blocker |
```

If a feature cannot be safely implemented, the candidate must be marked:

```json
{
  "live_parity_status": "backtest_only",
  "live_blocker": "trading-engine missing funding cost gate"
}
```

- [ ] **Step H3: Commit**

```bash
git add apps/trading-engine docs/superpowers/reports/2026-07-03-glm-round7-live-parity-report.md docs/superpowers/reports/2026-07-03-glm-martingale-round7-search-ledger.md
git commit -m "feat: 修复思路 Round7补齐马丁候选实盘引擎一致性"
```

## 12. Task I: Final Validation And Candidate Package

**Goal:** Validate every promoted candidate against the original constraints and record every failure so Round 8 does not repeat it.

**Files:**

- Create: `docs/superpowers/reports/2026-07-03-glm-round7-final-candidates.md`
- Create if any pass or near-frontier: `docs/superpowers/artifacts/glm-martingale-core-round7/promising/<candidate>.json`

- [ ] **Step I1: Full replay**

For each promoted candidate:

```bash
target/release/portfolio_budget_replay \
  --config docs/superpowers/artifacts/glm-martingale-core-round7/promising/<candidate>.json \
  --budget 5000 \
  --start-ms 1672531200000 \
  --end-ms 1780271999999 \
  --market-data data/market_data_full.db \
  --funding-data data/funding_rates.db \
  --profile aggressive \
  --portfolio-id r7-final-full \
  --exchange-min-notional 5
```

- [ ] **Step I2: Segment replay**

Run the same command for:

| Segment | start-ms | end-ms |
|---|---:|---:|
| h1_2023 | 1672531200000 | 1688169599999 |
| h2_2023 | 1688169600000 | 1704067199999 |
| 2024 | 1704067200000 | 1735689599999 |
| 2025 | 1735689600000 | 1767225599999 |
| 2026_ytd | 1767225600000 | 1780271999999 |

- [ ] **Step I3: Anti-overfit report**

Report these checks:

```markdown
| Check | Required |
|---|---|
| full ann/DD | original target or near-frontier gate |
| positive segments | >=4 for conservative/balanced, >=3 for aggressive |
| 2024-2026 aggregate | >0 |
| h1_2023 contribution | <=60%, or explain with 2024-2026 positive evidence |
| min-notional | all first/safety orders >= exchange minimum |
| feature parity | `passed` or `backtest_only` clearly marked |
| non-repeat key | recorded for every rejection |
```

- [ ] **Step I4: Commit**

```bash
git add docs/superpowers/reports/2026-07-03-glm-round7-final-candidates.md docs/superpowers/artifacts/glm-martingale-core-round7 docs/superpowers/reports/2026-07-03-glm-martingale-round7-search-ledger.md
git commit -m "docs: 修复思路 Round7归档马丁最终候选与失败记录"
```

## 13. Handoff Back To ChatGPT

Create:

`docs/superpowers/reports/2026-07-03-glm-round7-handoff-to-chatgpt.md`

Required sections:

1. Branch, latest commit, and push status.
2. Registry line count.
3. Exact list of Round 1-6 gaps repaired.
4. Task B detailed attribution summary.
5. Task C-G result table: candidate count, wall time, best ann/DD, segment results, decision, non-repeat key.
6. Task H live-parity table.
7. Original target pass table.
8. Near-frontier shortlist with config paths.
9. Rejected-family table that future agents must not repeat.
10. Open blockers needing user approval.

Commit:

```bash
git add docs/superpowers/reports/2026-07-03-glm-round7-handoff-to-chatgpt.md docs/superpowers/artifacts/glm-martingale-core-round7 docs/superpowers/reports/2026-07-03-glm-martingale-round7-search-ledger.md
git commit -m "docs: 修复思路 Round7马丁成本感知与缺口修复交接"
git push -u origin glm-martingale-core-round7
```

## 14. Stop Rules

Do not stop until one of these is true:

1. A conservative, balanced, or aggressive original target passes.
2. Tasks A-I are complete with saved artifacts and no missing evidence.
3. A task is blocked by missing data or live exchange access and the blocker is recorded with a non-repeat key.
4. Long search is still running, but a `running` registry line, process id, output path, and resume command are committed.

Do not claim "all tasks executed" unless every planned artifact path exists.

## 15. Long-Run Operating Rule

Long searches are allowed. Every long job must write:

```json
{
  "exploration_id": "r7-long-search-001",
  "status": "running",
  "pid": 12345,
  "started_at_utc": "2026-07-03T00:00:00Z",
  "command": "python3 scripts/glm_r7_dynamic_cost_dd_blend.py --mode rolling-allocator --curves docs/superpowers/artifacts/glm-martingale-core-round7/r7-dynamic-cost-dd-blend-curves.json --out docs/superpowers/artifacts/glm-martingale-core-round7/r7-dynamic-cost-dd-blend.json --report docs/superpowers/reports/2026-07-03-glm-round7-dynamic-blend.md --workers 12",
  "expected_artifact": "docs/superpowers/artifacts/glm-martingale-core-round7/r7-dynamic-cost-dd-blend.json",
  "resume_or_monitor_command": "tail -f /tmp/r7-long-search-001.log"
}
```

Commit the running entry before leaving the workstation.
