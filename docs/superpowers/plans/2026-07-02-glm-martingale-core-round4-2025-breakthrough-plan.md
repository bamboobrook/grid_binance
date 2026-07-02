# GLM Martingale Core Round 4 2025 Breakthrough Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 在 Round 3 `r3-P1-best-cd11` 的基础上，继续以马丁/DCA/grid 为核心，优先解决 live-parity 阻塞和 2025 震荡熊市负段，寻找满足 Conservative/Balanced/Aggressive 三档收益/回撤目标的小资金多币种组合。

**Architecture:** Round 4 不再重复 TP/cooldown/spacing 的普通参数扫。先做 P0 trading-engine parity，随后用 cycle attribution 找出 2025 亏损来源，再探索 range-regime 马丁、pump-fade short 马丁、stale-cycle active exit、rebound/wick-confirmed safety orders、premium/index 数据门、动态马丁配置库和风险预算分配。所有方向必须写入 Round 4 registry 与 ledger，失败必须留下 non-repeat key。

**Tech Stack:** Rust `backtest-engine` / `trading-engine` / `portfolio_budget_replay`, Python research runners in `scripts/`, SQLite `data/market_data_full.db`, `data/funding_rates.db`, optional derived SQLite for Binance premium/index/mark data, Git worktree evidence workflow.

---

## 0. Hard Constraints

Round 4 必须继续遵守：

1. 主收益必须来自马丁/DCA/grid cycle；不能用纯趋势、纯突破、纯 funding、纯 pair-neutral、纯统计套利替代。
2. 指标只允许辅助马丁：开仓许可、安全单确认、active-cycle exit、止盈止损、重入、权重、暂停、quarantine。
3. 最终候选必须 `<5000U` 本金预算、多币种、可用当前代码复跑、具备 trading-engine live-parity。
4. 不启动 Binance 实盘、不发布到 `flyingkid`、不写生产 DB。实盘启动必须等用户再次批准。
5. 三档目标保持不变：

| Profile | Annualized return | Max DD | Segment stability |
|---|---:|---:|---:|
| Conservative | `>50%` | `<=10%` | `>=4/5` positive |
| Balanced | `>90%` | `<=20%` | `>=4/5` positive |
| Aggressive | `>110%` | `<=30%` | `>=3/5` positive, prefer `>=4/5` |

## 1. Round 3 Baseline

GLM 启动前先读：

```bash
sed -n '1,240p' docs/superpowers/reports/2026-07-02-glm-round3-handoff-to-chatgpt.md
sed -n '1,220p' docs/superpowers/reports/2026-07-02-glm-round3-live-parity-audit.md
sed -n '1,220p' docs/superpowers/artifacts/glm-martingale-core-round3/exploration-registry.jsonl
sed -n '1,220p' docs/superpowers/artifacts/glm-martingale-core-round3/promising/r3-P1-best-cd11.json
```

Current best:

| Candidate | ann | DD | pos/5 | 2024-2026 agg | h1 contribution | Segment returns |
|---|---:|---:|---:|---:|---:|---|
| `r3-P1-best-cd11` | `34.5%` | `17.8%` | `4/5` | `+29.2%` | `43.7%` | h1 `+25.9`, h2 `+4.2`, 2024 `+32.1`, 2025 `-10.1`, 2026 `+7.2` |

Known blockers:

1. No target-passing candidate yet.
2. `r3-P1-best-cd11` uses backtest-only features: Partial TP, Breakeven, Conditional SO, Equity-Reclaim.
3. 2025 remains negative around `-10%`.
4. P3 TP/cooldown fine search found cd11h optimal; ordinary TP/cooldown tuning should not be repeated.
5. P4 custom ladder found fixed `150bps` still best.
6. P6 breadth proxy worsened 2025.
7. P8 core-satellite raised ann only to `35.0%` but worsened DD to `22.5%`.

## 2. External Research Signals For Round 4

Use these as mechanism inspiration, not as proof:

1. 3Commas DCA multiple take-profit docs allow up to 4 TP targets and mention managing averaging orders with trailing features. This supports testing 4-stage partial TP and active management of remaining safety orders. Source: `https://help.3commas.io/en/articles/8805166-dca-bot-multiple-take-profit`
2. 3Commas condition-based averaging order docs require both technical indicator conditions and minimum deviation; they also distinguish averaging from base order and from last executed entry order. Source: `https://help.3commas.io/en/articles/9663694-dca-bot-averaging-orders-by-technical-indicators`
3. 3Commas stop-loss breakeven docs support moving SL after profit stages, matching the current Partial TP + BE mechanism. Source: `https://help.3commas.io/en/articles/9464682-dca-bot-stop-loss-breakeven`
4. Binance Premium Index Kline and Index Price Kline endpoints are official replayable market-data sources that can be used to build a premium/mark/index regime proxy if historical coverage is available. Sources: `https://developers.binance.com/docs/derivatives/usds-margined-futures/market-data/rest-api/Premium-Index-Kline-Data`, `https://developers.binance.com/docs/derivatives/usds-margined-futures/market-data/rest-api/Index-Price-Kline-Candlestick-Data`
5. Binance Data Vision provides official public market-data file access. GLM must verify exact paths/checksums before using derived data. Source: `https://data.binance.vision/`
6. Binance public data GitHub states Data Vision files are downloadable daily/monthly and support public market data access. Source: `https://github.com/binance/binance-public-data`

## 3. Round 4 Non-Repeat Matrix

Do not repeat these as-is:

| Family | Non-repeat key | Allowed Round 4 mutation |
|---|---|---|
| TP/cooldown ordinary fine search | P3 found cd11h optimal and 2025 still `-10%` | Only test if combined with new range sleeve, active exit, or 4-stage TP |
| Custom ladder spacing | P4 found fixed150 best | Only retest with rebound/wick-confirmed SO or range-regime sleeve |
| BTC breadth proxy | P6 worsened 2025 | Replace with local cycle attribution or cross-sectional cycle health, not BTC-only breadth |
| Core-satellite static blend | P8 worsened DD | Only dynamic risk-budget allocation with hard boost DD caps |
| Direction-aware SO generic conditions | P1 best already uses long `rsi<45`, short `none` | New SO must add rebound/wick/volume confirmation or asymmetric active exits |
| OI/long-short/taker data | Historical local data absent | Use premium/index/mark data only after checksum coverage passes |
| Backtest-only candidate promotion | P0 found live parity absent | Any final candidate must pass trading-engine parity first |

## 4. Round 4 Registry And Ledger

Create new evidence directories:

```bash
mkdir -p docs/superpowers/artifacts/glm-martingale-core-round4/promising
mkdir -p docs/superpowers/artifacts/glm-martingale-core-round4/rejected
mkdir -p docs/superpowers/artifacts/glm-martingale-core-round4/run-manifests
touch docs/superpowers/reports/2026-07-02-glm-martingale-round4-search-ledger.md
touch docs/superpowers/artifacts/glm-martingale-core-round4/exploration-registry.jsonl
```

Every run, including failures and blocked data checks, appends one JSONL record:

```json
{
  "exploration_id": "r4-P2-range-sleeve-001",
  "started_at_utc": "runner writes current UTC timestamp, for example 2026-07-02T20:30:00Z",
  "family": "range_regime_martingale_sleeve",
  "hypothesis": "A low-ADX Bollinger range sleeve can earn in 2025 choppy bear while core trend martingale stays paused.",
  "new_vs_prior": "Prior searches focused trend-gated core; this adds a separate range-regime martingale cycle family.",
  "martingale_core": true,
  "research_only": false,
  "live_parity_status": "blocked_until_P0_passes",
  "input_signature": "range=adx<18|bb_width_pct=0.8-4|tp=250/600/1200|max_legs=4",
  "command": "runner copies full command line with arguments",
  "status": "running",
  "result_path": "docs/superpowers/artifacts/glm-martingale-core-round4/rejected/r4-P2-range-sleeve-001.json",
  "decision": "pending"
}
```

Ledger entry format:

```markdown
## r4-P2-range-sleeve-001

- Hypothesis:
- New vs prior:
- Command:
- Candidate count:
- Wall time:
- Best full result:
- Segment table:
- Gate result:
- Reject/promote reason:
- Non-repeat key:
```

No registry line and ledger entry means the result cannot be cited.

## 5. Worktree And Branch

Use a new branch/worktree:

```bash
git status --short --branch
git worktree add .worktrees/glm-martingale-core-round4 -b glm-martingale-core-round4
cd .worktrees/glm-martingale-core-round4
git status --short --branch
```

Expected:

```text
## glm-martingale-core-round4
```

Before long jobs:

```bash
ps -eo pid,etime,cmd | rg 'portfolio_budget_replay|search_small_capital_martingale|glm_r2|glm_r3|glm_r4' || true
sha256sum data/market_data_full.db data/funding_rates.db
```

Record any running process before reusing or stopping it.

## 6. P0: Trading-Engine Live-Parity Implementation

This is mandatory before any Round 2/3/4 candidate can be called live-reproducible.

### Task P0.1: Implement Partial TP Stage State

**Files:**
- Modify: `crates/shared-domain/src/martingale.rs`
- Modify: `apps/trading-engine/src/martingale_runtime.rs`
- Modify: `apps/trading-engine/src/main.rs`
- Test: `apps/trading-engine/tests/execution_sync.rs`
- Test: `apps/trading-engine/tests/order_sync.rs`

- [ ] **Step 1: Add failing tests**

Required test names:

```text
partial_tp_stage_close_uses_fraction_of_current_position
partial_tp_stage_persists_across_reconcile_ticks
partial_tp_final_stage_closes_remaining_position
```

Run:

```bash
cargo test -p trading-engine partial_tp_stage_close_uses_fraction_of_current_position -- --nocapture
```

Expected before implementation: fail because trading-engine does not evaluate `MartingaleTakeProfitModel::Partial`.

- [ ] **Step 2: Implement minimal state**

Rules:

1. Track `partial_tp_stage` per active martingale strategy/cycle.
2. Compute TP price from current weighted average entry and current stage bps.
3. Place reduce-only close for `stage_fraction * current_position_qty`.
4. Advance stage only after confirmed fill, not after order placement.
5. Final stage closes all remaining position.

- [ ] **Step 3: Run targeted tests**

```bash
cargo test -p trading-engine partial_tp -- --nocapture
```

Expected: all partial TP tests pass.

### Task P0.2: Implement Breakeven Stop After Partial TP

**Files:**
- Modify: `apps/trading-engine/src/martingale_runtime.rs`
- Modify: `apps/trading-engine/src/stop_loss.rs`
- Test: `apps/trading-engine/tests/execution_sync.rs`

- [ ] **Step 1: Add failing tests**

Required test names:

```text
breakeven_stop_activates_after_configured_stage_fill
breakeven_stop_uses_average_entry_plus_buffer_for_long
breakeven_stop_uses_average_entry_minus_buffer_for_short
```

Run:

```bash
cargo test -p trading-engine breakeven_stop_activates_after_configured_stage_fill -- --nocapture
```

Expected before implementation: fail.

- [ ] **Step 2: Implement**

Rules:

1. `breakeven_stop_active` turns true only after configured stage index fill.
2. Long BE stop = `avg_entry * (1 + buffer_bps / 10000)`.
3. Short BE stop = `avg_entry * (1 - buffer_bps / 10000)`.
4. Existing strategy drawdown stop remains fallback before BE is active.

- [ ] **Step 3: Run tests**

```bash
cargo test -p trading-engine breakeven_stop -- --nocapture
```

Expected: all BE stop tests pass.

### Task P0.3: Implement Conditional Safety Orders

**Files:**
- Modify: `apps/trading-engine/src/martingale_runtime.rs`
- Test: `apps/trading-engine/tests/execution_sync.rs`

- [ ] **Step 1: Add failing tests**

Required test names:

```text
safety_order_condition_false_blocks_next_leg
safety_order_condition_true_allows_next_leg_after_deviation
safety_order_condition_is_ignored_when_empty
```

Run:

```bash
cargo test -p trading-engine safety_order_condition_false_blocks_next_leg -- --nocapture
```

Expected before implementation: fail.

- [ ] **Step 2: Implement**

Rules:

1. Safety leg still requires price deviation.
2. If `safety_order_condition` is present, indicator expression must evaluate true at current bar.
3. Empty or missing condition preserves previous behavior.
4. Evaluation must use same indicator expression semantics as entry triggers.

- [ ] **Step 3: Run tests**

```bash
cargo test -p trading-engine safety_order_condition -- --nocapture
```

Expected: all targeted tests pass.

### Task P0.4: Implement Equity-Reclaim Re-Entry

**Files:**
- Modify: `apps/trading-engine/src/main.rs`
- Modify: `apps/trading-engine/src/martingale_runtime.rs`
- Test: `apps/trading-engine/tests/*`

- [ ] **Step 1: Add failing tests**

Required test names:

```text
portfolio_reentry_reclaim_keeps_cooldown_until_equity_threshold
portfolio_reentry_reclaim_ends_cooldown_after_threshold
portfolio_reentry_reclaim_disabled_uses_calendar_cooldown
```

- [ ] **Step 2: Implement**

Rules:

1. Store equity at stop and equity peak at stop.
2. Compute reclaim target = `equity_at_stop + reclaim_fraction * (peak_at_stop - equity_at_stop)`.
3. End cooldown early only if current equity >= reclaim target.
4. If reclaim fraction absent, keep calendar cooldown behavior.

- [ ] **Step 3: Run trading-engine suite**

```bash
cargo test -p trading-engine martingale -- --nocapture
cargo test -p trading-engine partial_tp -- --nocapture
cargo test -p trading-engine safety_order_condition -- --nocapture
cargo test -p trading-engine portfolio_reentry -- --nocapture
```

Expected: all targeted tests pass.

- [ ] **Step 4: Commit P0**

```bash
git add crates/shared-domain apps/trading-engine docs/superpowers/reports/2026-07-02-glm-martingale-round4-search-ledger.md
git commit -m "feat: 修复思路 补齐马丁Round4实盘等价机制"
```

## 7. P1: 2025 Cycle Attribution Forensics

Before more blind search, identify exactly how 2025 loses.

**Files:**
- Create: `scripts/glm_r4_cycle_attribution.py`
- Output: `docs/superpowers/artifacts/glm-martingale-core-round4/r4-cycle-attribution.json`
- Report: `docs/superpowers/reports/2026-07-02-glm-round4-2025-cycle-attribution.md`

Metrics per symbol/direction/config:

1. Closed cycles count.
2. Win/loss count.
3. Average realized PnL per cycle.
4. Max adverse excursion per cycle.
5. Max favorable excursion per cycle.
6. Cycle duration buckets: `<6h`, `6-24h`, `1-3d`, `>3d`.
7. Loss reason: SL, BE stop, final TP missed, stale open, portfolio stop.
8. Contribution by month within 2025.

- [ ] **Step 1: Run attribution for current best**

```bash
python3 scripts/glm_r4_cycle_attribution.py \
  --config docs/superpowers/artifacts/glm-martingale-core-round3/promising/r3-P1-best-cd11.json \
  --start-ms 1735689600000 \
  --end-ms 1767225599999 \
  --out docs/superpowers/artifacts/glm-martingale-core-round4/r4-cycle-attribution-2025.json
```

- [ ] **Step 2: Write report**

Report must include:

```markdown
| symbol | direction | return contribution | worst cycle | avg duration | loss count | likely fix |
```

Likely fixes must map to Round 4 directions:

| Finding | Route |
|---|---|
| long legs lose in low ADX ranges | P2 range sleeve or long pause |
| short legs fail after pump reversals | P3 pump-fade or P4 active exits |
| losses are stale cycles | P4 max age/no-progress exit |
| safety orders cluster before further adverse moves | P5 rebound/wick SO |
| premium/funding spikes precede losses | P6 premium gate |

- [ ] **Step 3: Commit**

```bash
git add scripts/glm_r4_cycle_attribution.py docs/superpowers/artifacts/glm-martingale-core-round4 docs/superpowers/reports/2026-07-02-glm-round4-2025-cycle-attribution.md docs/superpowers/reports/2026-07-02-glm-martingale-round4-search-ledger.md
git commit -m "docs: 修复思路 记录Round4马丁2025亏损归因"
```

## 8. P2: Range-Regime Martingale Sleeve

Hypothesis: 2025 is a choppy bear where trend-gated longs lose and shorts under-capture. A separate low-ADX/range-regime martingale sleeve with small TP and limited legs may harvest oscillations without replacing the core.

**Files:**
- Create: `scripts/glm_r4_range_regime_sleeve_search.py`
- Output: `docs/superpowers/artifacts/glm-martingale-core-round4/r4-range-regime-sleeve-grid.json`

Range sleeve rules:

1. Runs only when `adx(14) < threshold`.
2. Uses BB/RSI mean-reversion entry:
   - long range: `close < bb_lower(20,2)` or `rsi(14) < 35`
   - short range: `close > bb_upper(20,2)` or `rsi(14) > 65`
3. Max legs `3-5`, low multiplier `1.2-1.8`, fixed spacing `80-180bps`.
4. TP is small partial ladder `250/600/1200` or `300/700/1400`.
5. Hard per-sleeve DD stop `4-8%` of total budget.
6. Allocated as a sleeve beside `r3-P1` core, not replacing it.

Search grid:

| Field | Values |
|---|---|
| Range ADX threshold | `14`, `18`, `22`, `26` |
| BB stddev | `1.8`, `2.0`, `2.2`, `2.5` |
| RSI long/short | `30/70`, `35/65`, `40/60` |
| max legs | `3`, `4`, `5` |
| multiplier | `1.2`, `1.4`, `1.6`, `1.8` |
| spacing bps | `80`, `120`, `150`, `180` |
| sleeve allocation | `5%`, `10%`, `15%`, `20%` |

Segment-first pruning:

1. Run 2025 first.
2. Keep only rows with 2025 range sleeve return `>0` and sleeve DD `<=10%`.
3. Combine survivors with `r3-P1` core and validate all five segments.

- [ ] **Step 1: Run search**

```bash
python3 scripts/glm_r4_range_regime_sleeve_search.py \
  --out docs/superpowers/artifacts/glm-martingale-core-round4/r4-range-regime-sleeve-grid.json \
  --workers 24
```

- [ ] **Step 2: Promote if**

Any candidate satisfies:

1. 2025 full portfolio segment `>=0`, full ann `>=35%`, DD `<=20%`, pos `5/5`; or
2. full ann `>=45%`, DD `<=22%`, pos `>=4`, 2024-2026 positive; or
3. any original C/B/A target.

- [ ] **Step 3: Commit**

```bash
git add scripts/glm_r4_range_regime_sleeve_search.py docs/superpowers/artifacts/glm-martingale-core-round4 docs/superpowers/reports/2026-07-02-glm-martingale-round4-search-ledger.md
git commit -m "docs: 修复思路 记录Round4震荡区间马丁搜索"
```

## 9. P3: Pump-Fade Short Martingale Sleeve

Hypothesis: In choppy bear markets, shorting after upward exhaustion may work better than trend-following short entries. This remains martingale because entries and safety orders are DCA/grid cycles, not pure trend.

**Files:**
- Create: `scripts/glm_r4_pump_fade_short_search.py`
- Output: `docs/superpowers/artifacts/glm-martingale-core-round4/r4-pump-fade-short-grid.json`

Entry rules for short sleeve:

| Signal | Values |
|---|---|
| 24h price pump | `+6%`, `+9%`, `+12%`, `+18%` |
| RSI threshold | `65`, `70`, `75`, `80` |
| upper wick ratio | `0.25`, `0.35`, `0.50` |
| close below prior 3-bar low | on/off |
| BTC background | `BTC below ema50`, `BTC below ema200`, `none` |

Martingale params:

| Field | Values |
|---|---|
| first order | `20`, `25`, `30` USDT |
| multiplier | `1.4`, `1.8`, `2.2` |
| max legs | `3`, `4`, `5`, `6` |
| spacing | `120`, `180`, `250`, `350` bps |
| TP ladder | `300/700/1400`, `500/1000/1800` |
| BE after stage | `0`, `1` |

Symbols:

```text
AAVEUSDT,SOLUSDT,DOTUSDT,NEARUSDT,AVAXUSDT,OPUSDT,ARBUSDT,ADAUSDT,DOGEUSDT,LINKUSDT
```

- [ ] **Step 1: Run 2025-first search**

```bash
python3 scripts/glm_r4_pump_fade_short_search.py \
  --out docs/superpowers/artifacts/glm-martingale-core-round4/r4-pump-fade-short-grid.json \
  --workers 24
```

- [ ] **Step 2: Promote if**

1. Standalone 2025 ann `>30%`, DD `<=15%`; or
2. Combined with core flips 2025 positive while full DD `<=22%`; or
3. Combined full ann `>=45%`, DD `<=25%`, pos `>=4`.

- [ ] **Step 3: Commit**

```bash
git add scripts/glm_r4_pump_fade_short_search.py docs/superpowers/artifacts/glm-martingale-core-round4 docs/superpowers/reports/2026-07-02-glm-martingale-round4-search-ledger.md
git commit -m "docs: 修复思路 记录Round4冲高回落做空马丁搜索"
```

## 10. P4: Active-Cycle Stale Exit And Regime-Break Exit

Hypothesis: 2025 losses may come from stale cycles that neither reach TP nor fail quickly. Active exits can reduce dead capital and DD, allowing more profitable cycle turnover.

**Files:**
- Modify: `crates/shared-domain/src/martingale.rs`
- Modify: `apps/backtest-engine/src/martingale/kline_engine.rs`
- Create: `scripts/glm_r4_active_exit_search.py`

New optional risk fields:

```rust
pub max_cycle_age_hours: Option<f64>;
pub no_progress_exit_hours: Option<f64>;
pub no_progress_mfe_bps: Option<u32>;
pub regime_break_exit: Option<String>;
pub mfe_giveback_exit_pct: Option<f64>;
```

Rules:

1. `max_cycle_age_hours`: close cycle if age exceeds threshold.
2. `no_progress_exit`: close if cycle has not reached `no_progress_mfe_bps` within hours.
3. `regime_break_exit`: close when expression invalidates the direction.
4. `mfe_giveback_exit_pct`: after MFE occurs, close if giveback exceeds threshold.

Search grid:

| Field | Values |
|---|---|
| max cycle age | `24h`, `48h`, `72h`, `120h`, disabled |
| no progress hours | `12h`, `24h`, `48h`, disabled |
| no progress MFE | `100`, `200`, `400` bps |
| MFE giveback | `40%`, `60%`, `80%`, disabled |
| regime break | EMA50 break, BTC EMA50 break, disabled |

- [ ] **Step 1: Add failing tests**

```bash
cargo test -p backtest-engine stale_cycle_exit_closes_after_max_age -- --nocapture
```

Expected before implementation: fail.

- [ ] **Step 2: Implement backtest feature**

Only implement trading-engine parity if this search improves frontier.

- [ ] **Step 3: Run search**

```bash
python3 scripts/glm_r4_active_exit_search.py \
  --out docs/superpowers/artifacts/glm-martingale-core-round4/r4-active-exit-grid.json \
  --workers 20
```

- [ ] **Step 4: Promote if**

Promote if active exits improve at least two:

1. 2025 segment return by `+5pp`.
2. full DD lower by `>=3pp`.
3. full ann higher by `>=5pp`.
4. positive segments becomes `5/5`.

- [ ] **Step 5: Commit**

```bash
git add crates/shared-domain apps/backtest-engine scripts/glm_r4_active_exit_search.py docs/superpowers/artifacts/glm-martingale-core-round4 docs/superpowers/reports/2026-07-02-glm-martingale-round4-search-ledger.md
git commit -m "feat: 修复思路 增加Round4马丁滞留周期退出搜索"
```

## 11. P5: Rebound/Wick/Volume Confirmed Safety Orders

Hypothesis: Conditional SO improved the frontier, but it only checks an indicator. Price-action confirmation can reduce adding safety orders before continuation moves.

**Files:**
- Modify: `crates/shared-domain/src/martingale.rs`
- Modify: `apps/backtest-engine/src/martingale/kline_engine.rs`
- Create: `scripts/glm_r4_reversal_confirmed_so_search.py`

New optional fields:

```rust
pub safety_order_rebound_bps: Option<u32>;
pub safety_order_wick_ratio_min: Option<f64>;
pub safety_order_volume_spike_min: Option<f64>;
pub safety_order_max_per_signal: Option<u32>;
```

Signals:

| Long SO confirmation | Short SO confirmation |
|---|---|
| price rebounds `20-150bps` from local low | price pulls back `20-150bps` from local high |
| lower wick ratio `>=0.25/0.35/0.50` | upper wick ratio `>=0.25/0.35/0.50` |
| volume > `1.5/2.0/3.0x` rolling 60m | volume > `1.5/2.0/3.0x` rolling 60m |
| max one safety leg per signal | max one safety leg per signal |

- [ ] **Step 1: Add failing tests**

```bash
cargo test -p backtest-engine rebound_safety_order_waits_for_price_recovery -- --nocapture
cargo test -p backtest-engine wick_confirmed_safety_order_requires_wick_ratio -- --nocapture
```

Expected before implementation: fail.

- [ ] **Step 2: Implement research feature**

Use kline OHLCV already in local data. If volume is not available in the DB schema, mark volume-spike branch blocked and still test rebound/wick.

- [ ] **Step 3: Run search**

```bash
python3 scripts/glm_r4_reversal_confirmed_so_search.py \
  --out docs/superpowers/artifacts/glm-martingale-core-round4/r4-reversal-confirmed-so-grid.json \
  --workers 20
```

- [ ] **Step 4: Commit**

```bash
git add crates/shared-domain apps/backtest-engine scripts/glm_r4_reversal_confirmed_so_search.py docs/superpowers/artifacts/glm-martingale-core-round4 docs/superpowers/reports/2026-07-02-glm-martingale-round4-search-ledger.md
git commit -m "feat: 修复思路 增加Round4反转确认安全单搜索"
```

## 12. P6: Premium / Index / Mark Data Gate

Hypothesis: OI/long-short/taker history is missing, but premium/index/mark data may be downloadable from official sources and can proxy crowded perp conditions.

**Files:**
- Create: `scripts/glm_r4_market_data_probe.py`
- Create if available: `scripts/glm_r4_premium_gate_search.py`
- Output: `docs/superpowers/artifacts/glm-martingale-core-round4/r4-premium-data-probe.json`

Data probe steps:

1. Check Binance REST Premium Index Kline for one symbol/month.
2. Check Binance Data Vision URL paths for futures premium/index/mark data.
3. Save source URL, coverage, row count, min/max timestamp, sha256.
4. Do not commit large DB files; commit checksum report and script.

- [ ] **Step 1: Probe data availability**

```bash
python3 scripts/glm_r4_market_data_probe.py \
  --symbols BNBUSDT,SOLUSDT \
  --start-ms 1704067200000 \
  --end-ms 1706745599999 \
  --out docs/superpowers/artifacts/glm-martingale-core-round4/r4-premium-data-probe.json
```

- [ ] **Step 2: If coverage passes, run gate search**

Premium gate ideas:

| Feature | Long action | Short action |
|---|---|---|
| premium z-score high | reduce long new cycles | allow pump-fade short |
| premium z-score low | allow recovery long | reduce short |
| premium spike + high RSI | block long SO | allow short SO |
| mark-index divergence normalizes | allow re-entry | allow TP extension |

Run:

```bash
python3 scripts/glm_r4_premium_gate_search.py \
  --out docs/superpowers/artifacts/glm-martingale-core-round4/r4-premium-gate-grid.json \
  --workers 16
```

- [ ] **Step 3: Commit**

```bash
git add scripts/glm_r4_market_data_probe.py scripts/glm_r4_premium_gate_search.py docs/superpowers/artifacts/glm-martingale-core-round4 docs/superpowers/reports/2026-07-02-glm-martingale-round4-search-ledger.md
git commit -m "docs: 修复思路 记录Round4溢价数据门控搜索"
```

## 13. P7: Four-Stage Partial TP And Safety-Order Cancel Policy

Hypothesis: 3Commas supports up to four DCA TP targets; Round 2/3 used three-stage TP. Four-stage TP may bank smaller early profits, reduce BE stop churn, and allow tail winners without increasing DD.

**Files:**
- Create: `scripts/glm_r4_four_stage_tp_search.py`

Grid:

| Field | Values |
|---|---|
| TP ladder | `400/800/1600/2800`, `500/1000/1800/3200`, `600/1200/2200/3600` |
| Original split | `20/25/25/30`, `25/25/25/25`, `30/25/20/25`, `35/25/20/20` |
| BE after stage | `0`, `1`, `2` |
| BE buffer | `50`, `100`, `150` |
| Cancel unused SO after TP stage | disabled, after TP1, after TP2 |
| Core cooldown | `10h`, `11h`, `12h` |

- [ ] **Step 1: Implement search without changing engine if existing `Partial` supports arbitrary stage count**

If existing backtest `Partial` accepts more than 3 stages, no engine change is needed.

- [ ] **Step 2: Run**

```bash
python3 scripts/glm_r4_four_stage_tp_search.py \
  --out docs/superpowers/artifacts/glm-martingale-core-round4/r4-four-stage-tp-grid.json \
  --workers 20
```

- [ ] **Step 3: Promote if**

1. ann `>=40%`, DD `<=18%`, pos `>=4`; or
2. 2025 improves by `>=3pp` without reducing full ann; or
3. any original target passes.

- [ ] **Step 4: Commit**

```bash
git add scripts/glm_r4_four_stage_tp_search.py docs/superpowers/artifacts/glm-martingale-core-round4 docs/superpowers/reports/2026-07-02-glm-martingale-round4-search-ledger.md
git commit -m "docs: 修复思路 记录Round4四段止盈马丁搜索"
```

## 14. P8: Dynamic Martingale Config Library And Risk Budget Allocator

Hypothesis: P8 static core-satellite worsened DD. A dynamic allocator may add boost configs only during favorable live-observable states and keep cash otherwise.

**Files:**
- Create: `scripts/glm_r4_dynamic_config_allocator.py`

Candidate library:

1. Core: `r3-P1-best-cd11`.
2. Range sleeve best from P2 if positive in 2025.
3. Pump-fade short best from P3 if positive in 2025.
4. Four-stage TP best from P7 if ann/DD improves.
5. Cash state.

Allocator inputs:

| Input | Values |
|---|---|
| Lookback window | `14d`, `30d`, `60d`, `90d` |
| Metric | closed-cycle Sharpe proxy, return/DD, win rate with MAE penalty |
| Rebalance cadence | `7d`, `14d`, `30d` |
| Max boost allocation | `5%`, `10%`, `15%`, `20%` |
| Portfolio DD cap | `10%`, `15%`, `20%`, `25%` |

Rules:

1. Allocator may only use data before the rebalance timestamp.
2. Config list is fixed at simulation start.
3. If no config has positive prior score, allocate to cash.
4. Switch log must record timestamp, chosen config, score, and reason.

- [ ] **Step 1: Run allocator after P2/P3/P7 candidates exist**

```bash
python3 scripts/glm_r4_dynamic_config_allocator.py \
  --out docs/superpowers/artifacts/glm-martingale-core-round4/r4-dynamic-config-allocator.json \
  --workers 12
```

- [ ] **Step 2: Promote if**

1. ann `>=45%`, DD `<=18%`, pos `>=4`; or
2. ann `>=60%`, DD `<=22%`, pos `>=4`; or
3. any original target passes.

- [ ] **Step 3: Commit**

```bash
git add scripts/glm_r4_dynamic_config_allocator.py docs/superpowers/artifacts/glm-martingale-core-round4 docs/superpowers/reports/2026-07-02-glm-martingale-round4-search-ledger.md
git commit -m "docs: 修复思路 记录Round4动态马丁配置分配"
```

## 15. Final Validation Gates

Any promising candidate must run:

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

And all five segments:

| Segment | Start ms | End ms |
|---|---:|---:|
| h1_2023 | `1672531200000` | `1688169599999` |
| h2_2023 | `1688169600000` | `1704067199999` |
| 2024 | `1704067200000` | `1735689599999` |
| 2025 | `1735689600000` | `1767225599999` |
| 2026_ytd | `1767225600000` | `1780271999999` |

Promising artifacts go under:

```text
docs/superpowers/artifacts/glm-martingale-core-round4/promising/
```

Rejected artifacts go under:

```text
docs/superpowers/artifacts/glm-martingale-core-round4/rejected/
```

Candidate JSON must include these fields. Numeric fields must contain replayed numbers, not strings:

```json
{
  "candidate_id": "r4-P2-range-family-best",
  "family": "range_regime_martingale_sleeve",
  "martingale_core": true,
  "research_only": false,
  "live_parity_status": "passed_or_blocked_with_reason",
  "budget_quote": 5000,
  "full": {
    "annualized_return_pct": 45.2,
    "max_drawdown_pct": 18.4,
    "total_return_pct": 220.0,
    "trade_count": 12345,
    "principal_breached": false
  },
  "segments": {
    "h1_2023": {"return_pct": 12.3, "max_drawdown_pct": 8.1},
    "h2_2023": {"return_pct": 4.5, "max_drawdown_pct": 7.4},
    "2024": {"return_pct": 30.0, "max_drawdown_pct": 16.2},
    "2025": {"return_pct": 1.0, "max_drawdown_pct": 12.6},
    "2026_ytd": {"return_pct": 7.0, "max_drawdown_pct": 9.3}
  },
  "acceptance": {
    "conservative": false,
    "balanced": false,
    "aggressive": false,
    "reason": "first failed gate and value, for example ann 45.2 below conservative 50"
  }
}
```

## 16. Stop Rules

Stop a branch when:

1. It repeats a non-repeat key from Round 3 or Round 4.
2. It fails 2025-first pruning and has no new mechanism.
3. It improves ann only by increasing DD more than the ann improvement.
4. It cannot be made live-parity without unsafe exchange behavior.
5. Required data cannot be sourced with reproducible checksums.

Do not stop all Round 4 work until P0-P8 are either completed, blocked with evidence, or rejected with a non-repeat key.

## 17. Handoff Back To ChatGPT

When GLM pauses or finishes, write:

```text
docs/superpowers/reports/2026-07-02-glm-round4-handoff-to-chatgpt.md
```

Handoff sections:

1. Branch and latest commit.
2. P0 live-parity implementation status and tests.
3. 2025 cycle attribution summary.
4. Candidate counts and wall time per direction.
5. Best result per direction with full + five segment rows.
6. Comparison against `r3-P1-best-cd11`.
7. Any target-passing candidate.
8. Near-frontier table for Conservative/Balanced/Aggressive.
9. Registry line count and ledger path.
10. Rejected-family table with non-repeat keys.
11. Open blockers needing user approval, especially historical premium/OI data or real deployment.

Commit handoff:

```bash
git add docs/superpowers/reports/2026-07-02-glm-round4-handoff-to-chatgpt.md docs/superpowers/artifacts/glm-martingale-core-round4
git commit -m "docs: 修复思路 Round4马丁搜索交接"
git push -u origin glm-martingale-core-round4
```
