# GLM Martingale Core Round 3 Target Breakthrough Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 基于 Round 2 的 `28.6% ann / 22.5% DD / 4/5 positive segments` 新 frontier，继续只用马丁/DCA/grid 核心机制，寻找满足 Conservative/Balanced/Aggressive 三档目标的小资金、多币种、抗过拟合、实盘可复现组合。

**Architecture:** Round 3 分两条线并行推进：P0 先补齐 backtest/live parity 缺口，避免回测候选不可上线；P1-P8 继续扩展马丁核心机制，包括方向感知安全单、反弹确认、安全单 custom ladder、TP/cooldown 细扫、滚动 walk-forward 组合选择、breadth regime、mark/index premium 数据门和马丁内部分层组合。所有候选必须写入探索 registry 和 ledger，失败也必须留下非重复原因。

**Tech Stack:** Rust `backtest-engine` / `trading-engine` / `portfolio_budget_replay`, Python research runners in `scripts/`, SQLite `data/market_data_full.db` and `data/funding_rates.db`, optional derived SQLite for mark/premium/index data, Git branch/worktree evidence workflow.

---

## 0. Hard Constraints

Round 3 必须遵守：

1. 主收益源必须是马丁/DCA/grid cycle，不允许用纯趋势、纯突破、纯 funding、纯 pair-neutral 或纯统计套利替代。
2. 指标只能用于马丁的方向、开仓许可、安全单触发、止盈止损、重入、权重、暂停或 quarantine。
3. 最终候选必须 `<5000U` 本金预算、多币种、可实盘复现、可用当前代码复跑。
4. 三档门槛不变：

| Profile | Annualized return | Max DD | Segment stability |
|---|---:|---:|---:|
| Conservative | `>50%` | `<=10%` | `>=4/5` positive |
| Balanced | `>90%` | `<=20%` | `>=4/5` positive |
| Aggressive | `>110%` | `<=30%` | `>=3/5` positive, prefer `>=4/5` |

5. 任何结果都必须同时报告 full-period、五段 segment、`2024+2025+2026_ytd` aggregate、h1 contribution、budget breach、trade count。
6. 不启动 Binance 实盘，不发布到 `flyingkid`，不写生产 DB。Live parity 只做到代码和测试层，实盘启动需要用户再次批准。

## 1. Round 2 Evidence Baseline

GLM 先读这些文件：

```bash
sed -n '1,220p' docs/superpowers/reports/2026-07-02-glm-round2-full-handoff.md
sed -n '1,220p' docs/superpowers/artifacts/glm-martingale-core-round2/exploration-registry.jsonl
sed -n '1,220p' docs/superpowers/artifacts/glm-martingale-core-round2/promising/r2-H-best-cd12h.json
sed -n '1,220p' docs/superpowers/plans/2026-07-02-glm-martingale-core-round2-exhaustive-search-plan.md
```

Round 2 best:

| Candidate | ann | DD | pos/5 | 2024-2026 agg | h1 contribution | Segment returns |
|---|---:|---:|---:|---:|---:|---|
| `r2-H-best-cd12h` | `28.6%` | `22.5%` | `4/5` | `+22.8%` | `39.0%` | h1 `+36.4`, h2 `+6.6`, 2024 `+27.7`, 2025 `-10.2`, 2026 `+12.9` |

Key interpretation:

1. Round 2 broke the Round 1 frontier, but no profile target is met.
2. The remaining blocker is not only DD; it is insufficient return at acceptable DD, plus 2025 still negative.
3. Current best uses Partial TP + Breakeven + Conditional Safety Order + 12h cooldown.
4. Round 2 applied the same safety condition `rsi(14) < 45` to both long and short legs. That is likely directionally wrong for short safety orders, because short averaging happens when price rises and should normally need overbought confirmation such as `rsi(14) > 55/60` or `close > bb_upper`.
5. Handoff says Round 2 features are live-parity, but code search shows `Partial` and `safety_order_condition` are implemented in backtest paths while trading-engine parity is incomplete or unproven. Treat live-parity as blocked until P0 below passes.

## 2. External Research Signals For Round 3

Use these sources as mechanism inspiration, not as proof of profitability:

1. 3Commas condition-based averaging orders document indicator-gated averaging orders, AND logic, and two execution modes: from base order and from last executed entry order. It also states averaging orders need both indicator conditions and minimum deviation before execution. Source: `https://help.3commas.io/en/articles/9663694-dca-bot-averaging-orders-by-technical-indicators`
2. 3Commas stop-loss breakeven documents moving SL after TP stages, aligning with Round 2 Partial TP + BE mechanism. Source: `https://help.3commas.io/en/articles/9464682-dca-bot-stop-loss-breakeven`
3. Binance Open Interest Statistics and Long/Short Ratio endpoints expose useful live signals, but the official docs state only latest 1 month / 30 days is available, so they cannot validate 2023-2026 final gates unless a separate historical provider is sourced and checksummed. Sources: `https://developers.binance.com/docs/derivatives/usds-margined-futures/market-data/rest-api/Open-Interest-Statistics`, `https://developers.binance.com/docs/derivatives/usds-margined-futures/market-data/rest-api/Long-Short-Ratio`
4. Binance Premium Index Kline endpoint supports historical request parameters and can be tested as a reproducible proxy for crowded perp conditions. Source: `https://developers.binance.com/docs/derivatives/usds-margined-futures/market-data/rest-api/Premium-Index-Kline-Data`
5. Binance Basis endpoint is documented but only latest 30 days is available; treat it like OI/long-short unless a historical source is available. Source: `https://developers.binance.com/docs/derivatives/usds-margined-futures/market-data/rest-api/Basis`
6. Binance Data Vision exists as an official public data portal. It may provide downloadable historical market files, but GLM must verify exact dataset paths and checksums before using anything beyond current local DBs. Source: `https://data.binance.vision/`

## 3. Non-Repeat Matrix From Rounds 1-2

Do not repeat these as-is:

| Family | Non-repeat reason | Round 3 allowed mutation |
|---|---|---|
| Fixed/ATR/multiplier spacing standalone | Prior sweeps found 150bps best; non-150 negative in Round 2 C | Retest spacing only with direction-aware SO and partial TP active |
| Single full-position TP | Round 2 Partial TP improved frontier | Only use as baseline comparator |
| Same `rsi(14) < 45` SO for long and short | Likely directionally wrong; best still leaves 2025 negative | Use direction-specific SO condition |
| Calendar cooldown coarse grid `3/4/6/8/12h` | `12h` best stable, `4h` h1-dependent | Fine-scan `9/10/11/12/13/14/16h`, separate long/short cooldown |
| Symbol set add/drop only | Round 2 G found base6 best | Dynamic rolling selection or breadth regime only |
| Existing portfolio stop/reclaim | Stop rarely fires or hurts ann on B-best | Use only as risk overlay after candidate improves return |
| Inventory cap using existing budget caps | Caps never fired | Implement exposure-aware sizing if pursuing inventory skew |
| OI/long-short/taker sentiment from Binance REST | 2023-2026 local history absent, API only recent | Use premium/index/mark proxy first; paid data only with user approval |
| Funding standalone | Too small to bridge target gap | Use funding only as veto/weight feature |

## 4. Mandatory Round 3 Registry

Create new Round 3 directories:

```bash
mkdir -p docs/superpowers/artifacts/glm-martingale-core-round3/promising
mkdir -p docs/superpowers/artifacts/glm-martingale-core-round3/rejected
mkdir -p docs/superpowers/artifacts/glm-martingale-core-round3/run-manifests
touch docs/superpowers/reports/2026-07-02-glm-martingale-round3-search-ledger.md
touch docs/superpowers/artifacts/glm-martingale-core-round3/exploration-registry.jsonl
```

Every exploration, including failed smoke runs, appends one JSONL record:

```json
{
  "exploration_id": "r3-C-dir-aware-so-001",
  "started_at_utc": "runner writes current UTC, for example 2026-07-02T16:30:00Z",
  "family": "direction_aware_safety_order",
  "hypothesis": "Short safety orders need overbought confirmation while long safety orders need oversold confirmation.",
  "new_vs_prior": "Round 2 applied rsi<45 to both directions; this uses separate long/short conditions.",
  "martingale_core": true,
  "research_only": false,
  "live_parity_status": "backtest_only_until_P0_passes",
  "input_signature": "long_so=rsi<45|short_so=rsi>60|cooldown=12h|tp=800/1600/2600",
  "command": "runner copies the complete shell command including all arguments",
  "status": "running",
  "result_path": "docs/superpowers/artifacts/glm-martingale-core-round3/rejected/r3-C-dir-aware-so-001.json",
  "decision": "pending"
}
```

Ledger entry format:

```markdown
## r3-C-dir-aware-so-001

- Hypothesis:
- New vs prior:
- Command:
- Candidate count:
- Wall time:
- Best full-period result:
- Segment table:
- Gate result:
- Reject/promote reason:
- Non-repeat key:
```

No registry line + no ledger entry = result cannot be cited.

## 5. Worktree And Branch

Use a new worktree from current Round 2 branch:

```bash
git status --short --branch
git worktree add .worktrees/glm-martingale-core-round3 -b glm-martingale-core-round3
cd .worktrees/glm-martingale-core-round3
git status --short --branch
```

Expected:

```text
## glm-martingale-core-round3
```

Before long jobs:

```bash
ps -eo pid,etime,cmd | rg 'portfolio_budget_replay|search_small_capital_martingale|glm_r2|glm_r3|dgt_dynamic_grid_probe' || true
sha256sum data/market_data_full.db data/funding_rates.db
```

Record any running job before deciding whether to stop or reuse it.

## 6. P0: Live-Parity Audit And Patch Gate

Round 3 search may run research/backtest candidates, but final promotion is blocked until this task passes.

### Task 0.1: Audit parity for Round 2 features

**Files:**
- Read: `crates/shared-domain/src/martingale.rs`
- Read: `apps/backtest-engine/src/martingale/kline_engine.rs`
- Read: `apps/backtest-engine/src/martingale/exit_rules.rs`
- Read: `apps/trading-engine/src/martingale_runtime.rs`
- Read: `apps/trading-engine/src/main.rs`
- Create: `docs/superpowers/reports/2026-07-02-glm-round3-live-parity-audit.md`

- [ ] **Step 1: Run code search**

```bash
rg -n "MartingaleTakeProfitModel::Partial|safety_order_condition|reentry_equity_reclaim|breakeven_stop|partial_tp_stage" crates apps
```

Expected: output must show where each feature is implemented. If a feature appears only in backtest-engine and shared-domain, mark it `backtest_only`.

- [ ] **Step 2: Write audit report**

Report table:

```markdown
| Feature | shared config | backtest behavior | trading behavior | status | blocker |
|---|---|---|---|---|---|
| Partial TP stages | yes/no | yes/no | yes/no | live-parity/backtest-only | exact missing path |
| Breakeven after partial TP | yes/no | yes/no | yes/no | live-parity/backtest-only | exact missing path |
| Conditional safety orders | yes/no | yes/no | yes/no | live-parity/backtest-only | exact missing path |
| Equity reclaim re-entry | yes/no | yes/no | yes/no | live-parity/backtest-only | exact missing path |
```

- [ ] **Step 3: Commit audit**

```bash
git add docs/superpowers/reports/2026-07-02-glm-round3-live-parity-audit.md
git commit -m "docs: 修复思路 审计Round3马丁实盘等价缺口"
```

### Task 0.2: Patch trading-engine parity only if missing

**Files:**
- Modify as needed: `apps/trading-engine/src/martingale_runtime.rs`
- Modify as needed: `apps/trading-engine/src/main.rs`
- Modify as needed: `apps/trading-engine/src/take_profit.rs`
- Test: `apps/trading-engine/tests/*`

- [ ] **Step 1: Add failing tests for missing live features**

Required test names:

```text
partial_take_profit_places_stage_sized_reduce_only_close
partial_take_profit_advances_stage_and_keeps_remaining_position
breakeven_stop_activates_after_configured_partial_stage
directional_safety_order_condition_blocks_unconfirmed_safety_leg
portfolio_reentry_equity_reclaim_ends_cooldown_only_after_threshold
```

Run:

```bash
cargo test -p trading-engine partial_take_profit_places_stage_sized_reduce_only_close -- --nocapture
```

Expected: fail before implementation if parity is missing.

- [ ] **Step 2: Implement minimal parity**

Implementation rules:

1. Partial TP must use reduce-only close order quantity equal to configured stage fraction of current position.
2. TP stage must persist across reconcile ticks, not reset from config notes.
3. Breakeven stop activates only after configured stage index.
4. Conditional safety orders must use current indicator context and direction-specific expression.
5. Equity reclaim re-entry must match backtest semantics using known portfolio peak/equity at stop.

- [ ] **Step 3: Run tests**

```bash
cargo test -p trading-engine martingale -- --nocapture
cargo test -p trading-engine partial_take_profit -- --nocapture
cargo test -p trading-engine safety_order -- --nocapture
```

Expected: all targeted tests pass.

- [ ] **Step 4: Commit parity patch**

```bash
git add apps/trading-engine crates/shared-domain docs/superpowers/reports/2026-07-02-glm-round3-live-parity-audit.md
git commit -m "feat: 修复思路 补齐马丁Round3实盘等价机制"
```

## 7. P1: Direction-Aware Safety Order Search

Hypothesis: Round 2 left 2025 negative partly because the short sleeve used long-style `rsi(14)<45` safety confirmation. For shorts, adverse movement is upward, so safety orders should require overbought/rejection signals.

**Files:**
- Create: `scripts/glm_r3_dir_aware_so_search.py`
- Output: `docs/superpowers/artifacts/glm-martingale-core-round3/r3-dir-aware-so-grid.json`

Search grid:

| Long SO condition | Short SO condition |
|---|---|
| `rsi(14) < 45` | `rsi(14) > 55` |
| `rsi(14) < 40` | `rsi(14) > 60` |
| `rsi(14) < 35` | `rsi(14) > 65` |
| `close < bb_lower(20, 2)` | `close > bb_upper(20, 2)` |
| `adx(14) < 25 AND rsi(14) < 45` | `adx(14) < 25 AND rsi(14) > 55` |
| none | none baseline |

Base configs:

1. `r2-H-best-cd12h`.
2. Same config with cooldown `10h`, `11h`, `13h`, `14h`.
3. Same config but short TP ladder `600/1200/2200` while long stays `800/1600/2600`.

- [ ] **Step 1: Implement script with exact segment windows**

Script must run full period plus five segments:

```bash
python3 scripts/glm_r3_dir_aware_so_search.py \
  --out docs/superpowers/artifacts/glm-martingale-core-round3/r3-dir-aware-so-grid.json \
  --workers 20
```

Expected output JSON fields:

```json
{
  "n_candidates": 0,
  "results": [],
  "frontier_improvements": [],
  "near_targets": []
}
```

Completed output must replace zeros/empty arrays with real counts and records.

- [ ] **Step 2: Promotion threshold**

Promote if any candidate satisfies one of:

1. `2025 > 0` and full `ann >= 20%` and `DD <= 25%`.
2. Full `ann >= 35%`, `DD <= 25%`, `positive_segments >=4`.
3. Full `ann >= 50%`, `DD <=30%`, `positive_segments >=3`, `2024-2026 >0`.

- [ ] **Step 3: Commit result**

```bash
git add scripts/glm_r3_dir_aware_so_search.py docs/superpowers/artifacts/glm-martingale-core-round3 docs/superpowers/reports/2026-07-02-glm-martingale-round3-search-ledger.md
git commit -m "docs: 修复思路 记录Round3方向感知安全单搜索"
```

## 8. P2: Rebound-Confirmed Safety Orders

Hypothesis: 3Commas-style “from last executed entry order” and rebound confirmation can prevent adding multiple safety legs during one-way moves. This differs from Round 2 B because Round 2 only checked an indicator at the deviation point.

**Files:**
- Modify: `crates/shared-domain/src/martingale.rs`
- Modify: `apps/backtest-engine/src/martingale/kline_engine.rs`
- Create: `scripts/glm_r3_rebound_so_search.py`

New config fields:

```rust
pub safety_order_rebound_bps: Option<u32>;
pub safety_order_min_price_mode: Option<MartingaleSafetyOrderPriceMode>;
pub safety_order_max_per_signal: Option<u32>;
```

Enum:

```rust
pub enum MartingaleSafetyOrderPriceMode {
    FromBaseOrder,
    FromLastExecutedEntryOrder,
}
```

Search grid:

| Field | Values |
|---|---|
| `safety_order_rebound_bps` | `20`, `40`, `60`, `100`, `150` |
| `safety_order_min_price_mode` | `from_base_order`, `from_last_executed_entry_order` |
| `safety_order_max_per_signal` | `1`, `2` |
| Long condition | `rsi<45`, `rsi<40`, `bb_lower` |
| Short condition | `rsi>55`, `rsi>60`, `bb_upper` |

- [ ] **Step 1: Add tests before implementation**

```bash
cargo test -p backtest-engine rebound_safety_order_waits_for_price_recovery -- --nocapture
```

Expected before implementation: fail because fields/behavior do not exist.

- [ ] **Step 2: Implement backtest feature**

Rules:

1. Long safety: level touched after price falls; execute only after price rebounds by `rebound_bps` from local low.
2. Short safety: level touched after price rises; execute only after price pulls back by `rebound_bps` from local high.
3. `from_last_executed_entry_order` recalculates next trigger from the last executed safety order, not the base.
4. `max_per_signal=1` prevents multiple safety orders in one kline after a large gap.

- [ ] **Step 3: Run search**

```bash
python3 scripts/glm_r3_rebound_so_search.py \
  --out docs/superpowers/artifacts/glm-martingale-core-round3/r3-rebound-so-grid.json \
  --workers 20
```

- [ ] **Step 4: Commit feature and results**

```bash
git add crates/shared-domain apps/backtest-engine scripts/glm_r3_rebound_so_search.py docs/superpowers/artifacts/glm-martingale-core-round3 docs/superpowers/reports/2026-07-02-glm-martingale-round3-search-ledger.md
git commit -m "feat: 修复思路 增加马丁反弹确认安全单搜索"
```

## 9. P3: Partial TP And Cooldown Fine Search

Hypothesis: Round 2 tested only a coarse cooldown grid. The stable/return sweet spot may sit between 8h and 14h, and long/short legs may need different cooldowns.

**Files:**
- Create: `scripts/glm_r3_tp_cooldown_fine_search.py`
- Output: `docs/superpowers/artifacts/glm-martingale-core-round3/r3-tp-cooldown-fine-grid.json`

Search dimensions:

| Dimension | Values |
|---|---|
| Long cooldown | `8h`, `9h`, `10h`, `11h`, `12h`, `13h`, `14h`, `16h` |
| Short cooldown | `6h`, `8h`, `10h`, `12h`, `14h`, `16h` |
| Long TP ladder | `700/1400/2400`, `800/1600/2600`, `900/1800/3000` |
| Short TP ladder | `400/900/1600`, `600/1200/2200`, `800/1600/2600` |
| TP split | `25/35/40`, `30/30/40`, `35/35/30`, `40/30/30` |
| BE after stage | `0`, `1` |
| BE buffer | `50`, `75`, `100`, `150` |

Segment-first pruning:

1. Run `2025` and `2026_ytd` first.
2. Keep candidates where `2025 > -5%`, `2026_ytd > 0`, and segment DD `<=25%`.
3. Only survivors run full 5-segment validation.

- [ ] **Step 1: Implement script with pruning manifest**

Output must include:

```json
{
  "stage1_candidates": 0,
  "stage1_survivors": 0,
  "full_validated": 0,
  "results": [],
  "promoted": []
}
```

- [ ] **Step 2: Run**

```bash
python3 scripts/glm_r3_tp_cooldown_fine_search.py \
  --out docs/superpowers/artifacts/glm-martingale-core-round3/r3-tp-cooldown-fine-grid.json \
  --workers 24
```

- [ ] **Step 3: Commit**

```bash
git add scripts/glm_r3_tp_cooldown_fine_search.py docs/superpowers/artifacts/glm-martingale-core-round3 docs/superpowers/reports/2026-07-02-glm-martingale-round3-search-ledger.md
git commit -m "docs: 修复思路 记录Round3止盈冷却细扫"
```

## 10. P4: Custom Safety Ladder With Partial TP Active

Hypothesis: Round 1 custom spacing failed before Partial TP and conditional SO existed. Retesting custom safety ladders under Round 2 mechanisms is a new mechanism combination, not a repeat.

**Files:**
- Create: `scripts/glm_r3_custom_ladder_search.py`

Grid:

| Ladder name | Steps bps | Intended behavior |
|---|---|---|
| delayed_geometric | `180,270,405,610,915,1370,2055,3000` | avoid early over-averaging |
| balanced_curve | `150,230,350,520,780,1170,1750,2600` | close to 150bps start with wider tail |
| front_light_tail_heavy | `120,220,400,700,1050,1500,2200,3200` | fewer deep legs |
| shallow_fast | `100,160,240,360,540,810,1215,1800` | higher churn, test return ceiling |

Sizing sequences:

| Name | Notional sequence |
|---|---|
| mild | `35,70,130,230,380,600,900,1300` long; `30,60,110,200,330,520,780,1100` short |
| base_like | use multiplier `2.8/2.5` |
| capped_tail | `35,70,140,280,450,650,850,1050` long; `30,60,120,220,360,520,700,900` short |

- [ ] **Step 1: Run with current best TP/SO/cooldown**

```bash
python3 scripts/glm_r3_custom_ladder_search.py \
  --out docs/superpowers/artifacts/glm-martingale-core-round3/r3-custom-ladder-grid.json \
  --workers 20
```

- [ ] **Step 2: Reject criteria**

Reject if all rows have either:

1. `ann < 20%`, or
2. `DD > 30%`, or
3. `positive_segments < 3`, or
4. `2024-2026 aggregate <= 0`.

- [ ] **Step 3: Commit**

```bash
git add scripts/glm_r3_custom_ladder_search.py docs/superpowers/artifacts/glm-martingale-core-round3 docs/superpowers/reports/2026-07-02-glm-martingale-round3-search-ledger.md
git commit -m "docs: 修复思路 记录Round3自定义安全单阶梯搜索"
```

## 11. P5: Rolling Walk-Forward Martingale Config Selector

Hypothesis: A static config is leaving return on the table. A live-reproducible selector can choose among predeclared martingale configs using only prior rolling-window metrics.

This is not segment-specific overfit because the selector can only use data before the current timestamp.

**Files:**
- Create: `scripts/glm_r3_walkforward_selector.py`

Candidate library:

1. `r2-H-best-cd12h`.
2. Direction-aware SO best from P1.
3. Fine cooldown best from P3.
4. Higher-return but segment-risk candidate from Round 2 B: `partial_600_1200_2200-so_rsi30`.
5. 008-best as fallback.

Selector features from prior `lookback_days`:

| Feature | Values |
|---|---|
| Lookback | `30d`, `60d`, `90d`, `180d` |
| Score | `ann - 1.5*dd`, `ret/dd`, `positive_closed_cycle_rate - dd_penalty` |
| Switch cadence | `7d`, `14d`, `30d` |
| Max switch count per month | `1`, `2`, `4` |
| Cash fallback | on/off |

- [ ] **Step 1: Implement replay combiner**

The selector must:

1. Use only closed equity/cycle information available before each switch timestamp.
2. Keep a fixed list of configs declared at simulation start.
3. Never choose by segment label or future returns.
4. Output switch log with timestamp, chosen config, prior score, reason.

- [ ] **Step 2: Run**

```bash
python3 scripts/glm_r3_walkforward_selector.py \
  --out docs/superpowers/artifacts/glm-martingale-core-round3/r3-walkforward-selector.json \
  --workers 12
```

- [ ] **Step 3: Promotion threshold**

Promote only if:

1. Full `ann >= 40%`.
2. Full `DD <= 25%`.
3. `positive_segments >=4`.
4. Switch log proves no future labels were used.

- [ ] **Step 4: Commit**

```bash
git add scripts/glm_r3_walkforward_selector.py docs/superpowers/artifacts/glm-martingale-core-round3 docs/superpowers/reports/2026-07-02-glm-martingale-round3-search-ledger.md
git commit -m "docs: 修复思路 记录Round3滚动走前马丁配置选择"
```

## 12. P6: Cross-Section Breadth Regime For Long Pause / Short Tilt

Hypothesis: 2025 loss comes from broad-bear chop where long legs still run and short legs under-capture. A cross-sectional breadth gate can pause longs and tilt shorts without using calendar labels.

**Files:**
- Create: `scripts/glm_r3_breadth_regime_search.py`

Universe for breadth:

```text
BTCUSDT, ETHUSDT, BNBUSDT, TRXUSDT, BCHUSDT, SOLUSDT, AAVEUSDT, DOTUSDT,
NEARUSDT, ADAUSDT, DOGEUSDT, XRPUSDT, LINKUSDT, AVAXUSDT, OPUSDT, ARBUSDT,
LTCUSDT, ETCUSDT, ATOMUSDT, UNIUSDT
```

Features:

| Feature | Definition |
|---|---|
| `breadth_above_ema50` | fraction of universe with close > ema50 |
| `breadth_above_ema200` | fraction with close > ema200 |
| `breadth_20d_return_positive` | fraction with 20d return > 0 |
| `dispersion` | cross-sectional stddev of 20d returns |

Rules:

| Rule | Values |
|---|---|
| Pause longs when breadth_above_ema50 < | `0.25`, `0.35`, `0.45` |
| Tilt short weight when breadth_above_ema50 < | `0.25`, `0.35` |
| Resume longs when breadth_above_ema50 > | `0.45`, `0.55`, `0.65` |
| Max short weight multiplier | `1.25`, `1.5`, `2.0` |
| Max long weight multiplier | `0.25`, `0.5`, `0.75`, `1.0` |

- [ ] **Step 1: Implement research-only breadth simulator**

This can be research-only first because expression language may not support cross-sectional aggregates.

- [ ] **Step 2: Run**

```bash
python3 scripts/glm_r3_breadth_regime_search.py \
  --out docs/superpowers/artifacts/glm-martingale-core-round3/r3-breadth-regime-grid.json \
  --workers 16
```

- [ ] **Step 3: Promotion threshold**

Promote to engine work only if:

1. 2025 flips positive, or
2. full ann improves by at least `+10 pct points` without DD increase, or
3. full ann `>=50%` and DD `<=30%`.

- [ ] **Step 4: Commit**

```bash
git add scripts/glm_r3_breadth_regime_search.py docs/superpowers/artifacts/glm-martingale-core-round3 docs/superpowers/reports/2026-07-02-glm-martingale-round3-search-ledger.md
git commit -m "docs: 修复思路 记录Round3市场宽度马丁门控搜索"
```

## 13. P7: Premium/Mark/Index Data Gate

Hypothesis: Since OI/long-short/taker history is unavailable locally and Binance REST limits them to recent data, mark/index/premium klines may provide a replayable crowding proxy.

**Files:**
- Create: `scripts/glm_r3_premium_data_check.py`
- Create if data exists: `data/premium_index_klines.db`

Data checks:

1. Can Binance Premium Index Kline data be downloaded for 2023-2026 for the six Round 2 symbols?
2. Can Binance Data Vision provide the same data as files?
3. Are timestamps aligned with `market_data_full.db`?
4. Does the dataset cover all five required segments?
5. Can every file be checksummed and re-downloaded?

- [ ] **Step 1: Check availability only**

Do not run huge downloads first. Check one symbol and one month:

```bash
python3 scripts/glm_r3_premium_data_check.py \
  --symbols BNBUSDT \
  --start-ms 1704067200000 \
  --end-ms 1706745599999 \
  --out docs/superpowers/artifacts/glm-martingale-core-round3/r3-premium-data-check.json
```

- [ ] **Step 2: If available, build derived features**

Features:

| Feature | Gate idea |
|---|---|
| premium z-score high | reduce long / allow short if not squeeze |
| premium z-score low | reduce short / allow long if regime recovers |
| premium crosses zero | cooldown reset or cycle pause |
| mark-index spread spike | pause new safety orders |

- [ ] **Step 3: Run only after data coverage passes**

```bash
python3 scripts/glm_r3_premium_gate_search.py \
  --premium-db data/premium_index_klines.db \
  --out docs/superpowers/artifacts/glm-martingale-core-round3/r3-premium-gate-grid.json \
  --workers 16
```

- [ ] **Step 4: Commit data report or search result**

```bash
git add scripts/glm_r3_premium_data_check.py scripts/glm_r3_premium_gate_search.py docs/superpowers/artifacts/glm-martingale-core-round3 docs/superpowers/reports/2026-07-02-glm-martingale-round3-search-ledger.md
git commit -m "docs: 修复思路 记录Round3溢价数据可用性与马丁门控"
```

Do not commit large DB files unless the repo already tracks comparable market DB artifacts. Prefer checksum report and local path.

## 14. P8: Martingale Core-Satellite Portfolio

Hypothesis: A single stable config cannot hit 50/90/110% ann. A portfolio of martingale configs can keep `r2-H` as core and add small, risk-capped martingale boost configs only when rolling gates allow.

Allowed sleeves:

1. Core: `r2-H-best-cd12h`, 60-85% allocation.
2. Boost A: high-return partial `600/1200/2200` candidate, 5-20% allocation.
3. Boost B: direction-aware short sleeve from P1, 5-20% allocation.
4. Cash: unused budget when gates fail.

Forbidden sleeves:

1. Pure trend, pure breakout, pure funding, pure pair-neutral.
2. Any sleeve without martingale cycle execution.
3. Any segment-specific date rule.

**Files:**
- Create: `scripts/glm_r3_core_satellite_martingale_search.py`

Risk overlay:

| Field | Values |
|---|---|
| Core allocation | `60`, `70`, `80`, `85` |
| Boost total allocation | `10`, `15`, `20`, `30` |
| Boost max DD stop | `6%`, `8%`, `10%`, `12%` of portfolio |
| Boost enable rule | rolling 30d core DD < `8/10/12%`, breadth_above_ema50 > `0.35/0.45`, previous 14d boost positive |
| Max active boost symbols | `1`, `2`, `3` |

- [ ] **Step 1: Run core-satellite combiner**

```bash
python3 scripts/glm_r3_core_satellite_martingale_search.py \
  --out docs/superpowers/artifacts/glm-martingale-core-round3/r3-core-satellite-grid.json \
  --workers 12
```

- [ ] **Step 2: Promotion threshold**

Promote if any profile gate is hit. If not, save nearest frontier:

| Frontier class | Save if |
|---|---|
| Conservative near | `ann >=40`, `DD <=12`, `pos >=4` |
| Balanced near | `ann >=60`, `DD <=22`, `pos >=4` |
| Aggressive near | `ann >=80`, `DD <=32`, `pos >=3`, `2024-2026 >0` |

- [ ] **Step 3: Commit**

```bash
git add scripts/glm_r3_core_satellite_martingale_search.py docs/superpowers/artifacts/glm-martingale-core-round3 docs/superpowers/reports/2026-07-02-glm-martingale-round3-search-ledger.md
git commit -m "docs: 修复思路 记录Round3马丁核心卫星组合搜索"
```

## 15. Final Validation For Any Promoted Candidate

For every promoted candidate:

```bash
target/release/portfolio_budget_replay \
  --config <candidate-config.json> \
  --budget 5000 \
  --start-ms 1672531200000 \
  --end-ms 1780271999999 \
  --market-data data/market_data_full.db \
  --funding-data data/funding_rates.db \
  --profile aggressive \
  --portfolio-id <candidate-id> \
  --exchange-min-notional 5
```

Then run all five segments with exact windows from Round 2.

Candidate JSON must include:

```json
{
  "candidate_id": "r3-promoted-example",
  "family": "direction_aware_safety_order",
  "martingale_core": true,
  "research_only": false,
  "live_parity_status": "passed",
  "budget_quote": 5000,
  "full": {
    "annualized_return_pct": 0.0,
    "max_drawdown_pct": 0.0,
    "total_return_pct": 0.0,
    "trade_count": 0,
    "principal_breached": false
  },
  "segments": {
    "h1_2023": {"return_pct": 0.0, "annualized_return_pct": 0.0, "max_drawdown_pct": 0.0},
    "h2_2023": {"return_pct": 0.0, "annualized_return_pct": 0.0, "max_drawdown_pct": 0.0},
    "2024": {"return_pct": 0.0, "annualized_return_pct": 0.0, "max_drawdown_pct": 0.0},
    "2025": {"return_pct": 0.0, "annualized_return_pct": 0.0, "max_drawdown_pct": 0.0},
    "2026_ytd": {"return_pct": 0.0, "annualized_return_pct": 0.0, "max_drawdown_pct": 0.0}
  },
  "acceptance": {
    "conservative": false,
    "balanced": false,
    "aggressive": false,
    "reason": "first failed gate and value, for example DD 32.4 exceeds aggressive limit 30"
  }
}
```

The zeros above are schema examples only; completed files must contain replayed values.

## 16. Stop Rules

Stop a branch when:

1. It repeats a prior non-repeat reason.
2. Smoke sample is worse than `r2-H-best-cd12h` on ann, DD, positive segments, and 2024-2026 aggregate.
3. Required data cannot be sourced with checksums.
4. It cannot be made live-parity without unsafe exchange behavior.

Do not stop all Round 3 work until every P1-P8 direction is either run, blocked with evidence, or rejected with a non-repeat key.

## 17. Handoff Back To ChatGPT

When GLM pauses or completes, write a date-stamped handoff file. If the handoff is produced on 2026-07-02, use:

```text
docs/superpowers/reports/2026-07-02-glm-round3-handoff-to-chatgpt.md
```

Required sections:

1. Branch and latest commit.
2. P0 live-parity audit status and any patches.
3. Candidate counts and wall time per direction.
4. Best result per direction, full + five segments.
5. Comparison against `r2-H-best-cd12h`.
6. Any target-passing candidates.
7. Near frontier table for Conservative/Balanced/Aggressive.
8. Registry line count and ledger path.
9. Rejected-family table with non-repeat key.
10. Open blockers needing user approval, especially paid historical sentiment data or live deployment.

Commit handoff:

```bash
git add docs/superpowers/reports/2026-07-02-glm-round3-handoff-to-chatgpt.md docs/superpowers/artifacts/glm-martingale-core-round3
git commit -m "docs: 修复思路 Round3马丁搜索交接"
git push -u origin glm-martingale-core-round3
```
