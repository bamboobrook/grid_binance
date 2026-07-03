# GLM Martingale Core Round 6 DD Compression Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 在 Round 5 首次突破 `50%` 年化的 ANKR/XRP/DOGE 马丁组合基础上，继续以马丁/DCA/grid cycle 为收益核心，优先压缩 DD、修复 2025 负段，并补齐关键实盘等价缺口。

**Architecture:** Round 6 不再重复普通 multiplier 降档、普通 symbol swap、普通 entry gate、低 ADX range、custom ladder、same-symbol hedge 和单阈值 premium gate。先对 `r5-G-best-ANKRUSDT` 的 `32.1%` DD 做 drawdown-window 归因，再测试只属于马丁内部的风险层：portfolio DD state machine、symbol/direction quarantine、partial-TP 后 trailing profit lock、安全单取消/冻结策略、候选族风险预算混合、以及 trading-engine parity。所有探索都必须写入 Round 6 registry 与 ledger，失败留下 non-repeat key。

**Tech Stack:** Rust `backtest-engine` / `trading-engine` / `portfolio_budget_replay`, Python runners in `scripts/`, SQLite `data/market_data_full.db`, `data/funding_rates.db`, `data/premium_index.db`, Git branch `glm-martingale-core-round6`.

---

## 0. Hard Constraints

继续执行用户约束：

1. 主收益必须来自马丁/DCA/grid cycle；不能替换成纯趋势、纯突破、纯 funding、纯 pair-neutral、纯统计套利。
2. 指标只能作为马丁的开仓许可、安全单条件、止盈止损、暂停、权重、quarantine 或重入判断。
3. 最终候选必须 `<5000U`、多币种、五段验证、反过拟合、可由当前仓库复跑，并具备 trading-engine live parity 或明确 blocker。
4. 每一次失败探索必须写入 registry 和 ledger，下轮不重复。
5. 不启动 Binance 实盘、不写真实账户、不自动发布策略。

目标门槛：

| Profile | Annualized return | Max DD | Segment stability |
|---|---:|---:|---:|
| Conservative | `>50%` | `<=10%` | `>=4/5` positive, 2024-2026 aggregate positive |
| Balanced | `>90%` | `<=20%` | `>=4/5` positive, 2024-2026 aggregate positive |
| Aggressive | `>110%` | `<=30%` | `>=3/5` positive, prefer `>=4/5` |

固定验证窗口：

| Segment | Start ms | End ms |
|---|---:|---:|
| h1_2023 | `1672531200000` | `1688169599999` |
| h2_2023 | `1688169600000` | `1704067199999` |
| 2024 | `1704067200000` | `1735689599999` |
| 2025 | `1735689600000` | `1767225599999` |
| 2026_ytd | `1767225600000` | `1780271999999` |

## 1. Round 5 Facts To Read First

GLM 先读这些文件：

```bash
sed -n '1,260p' docs/superpowers/reports/2026-07-03-glm-round5-handoff-to-chatgpt.md
sed -n '1,260p' docs/superpowers/reports/2026-07-03-glm-martingale-round5-search-ledger.md
sed -n '1,260p' docs/superpowers/artifacts/glm-martingale-core-round5/exploration-registry.jsonl
sed -n '1,260p' docs/superpowers/artifacts/glm-martingale-core-round5/promising/r5-G-best-ANKRUSDT.json
sed -n '1,220p' docs/superpowers/reports/2026-07-03-glm-round5-live-parity-hardening.md
sed -n '1,220p' docs/superpowers/reports/2026-07-03-glm-round5-cycle-microregime.md
```

Round 5 当前最优：

| Candidate | ann | DD | pos/5 | 2025 | Config |
|---|---:|---:|---:|---:|---|
| `r5-G-best-ANKRUSDT` | `59.5%` | `32.1%` | `4/5` | `-17.5%` | BNB/TRX/ANKR long + AAVE/SOL/DOT short, last-exec SO, vol-target, long mult `3.3`, cd `11h` |
| `replaceL2_XRPUSDT` | `50.6%` | `25.4%` | `4/5` | `-17.5%` | lower DD than ANKR, still fails Conservative DD |
| `replaceL2_DOGEUSDT` | `50.4%` | `25.6%` | `4/5` | `-17.5%` | similar to XRP |
| `r5-fine-combo-best` | `49.93%` | `26.3%` | `4/5` | `-15.9%` | BCH instead of ANKR |
| low-DD fine-combo band | `31-38%` | `17.7-20.0%` | `3-4/5` | `-10%` to `-15%` | multiplier around `3.0-3.1`, TP `600/700` |

Main blockers:

1. Conservative return passed for the first time, but DD is `32.1%`, far above `10%`.
2. Balanced and Aggressive return targets remain far away.
3. 2025 worsened as return increased: `-8.7%` at R4, `-17.5%` at ANKR high-ann.
4. R5 features `last-exec SO`, `loss-streak risk reduction`, and `vol-target` are backtest-only until trading-engine parity is implemented.
5. Local branch currently contains a script-only commit `scripts/glm_r5_ankr_lowdd_search.py`; Round 6 must either run it and record results or supersede it with a deterministic Round 6 search. Do not leave script-only evidence.

## 2. 2026-07-03 External Search Signals

Use these as mechanism inspiration only:

| Source | Useful Round 6 signal | Constraint |
|---|---|---|
| 3Commas DCA main settings: https://help.3commas.io/en/articles/3108940-dca-bot-interface-and-main-settings | Current DCA bots expose Futures long and short presets, strategy gallery, and configurable DCA bot settings. | Product docs are not proof of target pass. |
| 3Commas Stop Loss Breakeven: https://help.3commas.io/en/articles/9464682-dca-bot-stop-loss-breakeven | Stop-loss can move to breakeven after multiple TP targets. Round 6 can extend this to trailing profit lock after partial TP. | Must remain a TP/SL rule inside a martingale cycle. |
| 3Commas DCA bot page: https://3commas.io/dca-bots | Public DCA tooling emphasizes TA-based DCA, historical backtesting, and multi-exchange execution. | Use only for control ideas; local replay decides. |
| OKX Futures DCA/Martingale: https://www.okx.com/en-eu/help/iv-futures-dca-martingale | Futures DCA is explicitly Martingale-based and requires stop-loss/risk control. | OKX docs do not validate Binance live parity. |
| Binance New Algo Order: https://developers.binance.com/docs/derivatives/usds-margined-futures/trade/rest-api/New-Algo-Order | USD-M TP/SL/trailing conditional orders use `/fapi/v1/algoOrder`. | Final live-ready candidate must fit algo-order semantics. |
| Binance derivatives change log: https://developers.binance.com/docs/derivatives/change-log | USD-M conditional orders migrated to Algo Service effective 2025-12-09. | Old conditional-order paths are not live-ready. |
| Binance exchangeInfo: https://developers.binance.com/docs/derivatives/usds-margined-futures/market-data/rest-api/Exchange-Information | Exact symbol filters and order limits come from `exchangeInfo`. | Replace fixed `exchange-min-notional 5` before live promotion. |
| Binance common definitions: https://developers.binance.com/docs/derivatives/usds-margined-futures/common-definition | `MAX_NUM_ORDERS` counts normal and algo orders; algo-order filters constrain TP/SL/trailing designs. | Multi-stage TP/SL must pass order-count audit. |
| Dynamic Grid Trading paper: https://arxiv.org/abs/2506.11921 | DGT uses dynamic reset/re-anchor to improve grid behavior. | Prior DGT failed user gates; Round 6 may test re-anchor only as a DCA cycle modifier, not a standalone strategy. |

## 3. Round 1-5 Non-Repeat Matrix

Do not repeat these as-is:

| Family | Evidence | Non-repeat key | Allowed mutation |
|---|---|---|---|
| Plain regime gates | R1/R3/R4 failed or worsened 2025 | `plain-regime-gate` | Only use drawdown-window attribution tied to a specific loss source. |
| Portfolio DD stop alone | R1/R2 reduced DD by killing return | `portfolio-stop-alone` | Round 6 may test staged DD state machine that throttles, locks profit, and re-enters by lagged rules. |
| Tight SL only | Prior tight SL scans made ann negative | `tight-sl-only` | SL can be part of profit-lock or symbol quarantine. |
| High budget | Prior high budget worsened stability | `increase-budget` | Budget remains `<5000U`. |
| Ordinary TP/cooldown scan | R2/R3/R4 found cd11 and 3-stage TP best | `ordinary-tp-cooldown` | Test trailing after partial TP, not another plain stage count scan. |
| Custom ladder | R3/R5 found custom ladder near zero or worse | `custom-ladder-rescan` | Only re-anchor as DCA cycle modifier after drawdown proof. |
| Low-ADX range sleeve | R4 barely fired | `low-adx-range-sleeve` | Do not repeat. |
| Rebound SO only | R4 no improvement | `rebound-so-only` | Can combine with drawdown state or safety cancellation. |
| Active max-age/no-progress exit | R4 no improvement | `max-age-no-progress-exit` | Profit-side MFE trailing lock is distinct and allowed. |
| Single premium threshold | R4 weak signal | `single-premium-threshold` | Premium can be a secondary gate in drawdown-window attribution only. |
| Same-symbol hedge grid | R5 ann `0-4.5%` | `same-symbol-hedge-grid` | Do not repeat. |
| Custom price ladder under last-exec SO | R5 all near `0%` | `custom-ladder-last-exec` | Do not repeat. |
| Loss-streak risk reduction | R5 no effect on high-mult path | `loss-streak-risk-reduction` | Use realized drawdown state machine, not simple consecutive loss count. |
| Simple ANKR/XRP/DOGE replacement | R5 already found top symbols and DD range | `simple-l2-replacement` | Round 6 may test quarantine, blended allocation, or DD state around these candidates. |
| Static allocator where ANKR dominates BCH | R5 rejected | `static-ankr-dominates-bch` | Risk-budget allocator may compare ANKR vs XRP/DOGE/R4/cash by DD, not by ann only. |

## 4. Round 6 Evidence Layout

Create Round 6 files:

```bash
mkdir -p docs/superpowers/artifacts/glm-martingale-core-round6/promising
mkdir -p docs/superpowers/artifacts/glm-martingale-core-round6/rejected
mkdir -p docs/superpowers/artifacts/glm-martingale-core-round6/run-manifests
touch docs/superpowers/reports/2026-07-03-glm-martingale-round6-search-ledger.md
touch docs/superpowers/artifacts/glm-martingale-core-round6/exploration-registry.jsonl
```

Every exploration appends a JSONL record before and after execution:

```json
{"exploration_id":"r6-B-drawdown-state-machine-001","started_at_utc":"2026-07-03T12:30:00Z","family":"portfolio_drawdown_state_machine","hypothesis":"Throttle new martingale cycles and safety legs by portfolio underwater state to keep ANKR-like return while compressing DD.","new_vs_prior":"Prior portfolio stops were binary; this uses staged throttles, profit lock, and lagged recovery rules.","martingale_core":true,"research_only":false,"input_signature":"base=r5-G-best-ANKRUSDT;dd_states=6/10/14/18/22;actions=scale/cooldown/so-freeze","command":"python3 scripts/glm_r6_drawdown_state_machine_search.py --base-config docs/superpowers/artifacts/glm-martingale-core-round5/promising/r5-G-best-ANKRUSDT.json --out docs/superpowers/artifacts/glm-martingale-core-round6/r6-drawdown-state-machine.json --workers 20","status":"running","result_path":"docs/superpowers/artifacts/glm-martingale-core-round6/r6-drawdown-state-machine.json","decision":"pending","non_repeat_key":"staged-dd-state-machine"}
```

Ledger section format:

```markdown
## r6-B-drawdown-state-machine-001

- Hypothesis:
- New vs prior:
- Command:
- Candidate count:
- Wall time:
- Best full result:
- Five segment table:
- 2025 result:
- Gate result:
- Decision:
- Non-repeat key:
```

## 5. Branch And Baseline Commands

Start from Round 5:

```bash
git status --short --branch
git fetch origin
git worktree add .worktrees/glm-martingale-core-round6 -b glm-martingale-core-round6 HEAD
cd .worktrees/glm-martingale-core-round6
git status --short --branch
```

Build:

```bash
cargo build --release -p backtest-engine
test -x target/release/portfolio_budget_replay
```

Record data and code fingerprints:

```bash
git rev-parse HEAD > docs/superpowers/artifacts/glm-martingale-core-round6/run-manifests/r6-start-commit.txt
sha256sum data/market_data_full.db data/funding_rates.db data/premium_index.db > docs/superpowers/artifacts/glm-martingale-core-round6/run-manifests/r6-data-sha256.txt
sqlite3 data/market_data_full.db "SELECT COUNT(*) AS rows, COUNT(DISTINCT symbol) AS symbols, MIN(open_time), MAX(open_time) FROM klines;" > docs/superpowers/artifacts/glm-martingale-core-round6/run-manifests/r6-market-coverage.txt
```

Reproduce baselines before any new search:

```bash
target/release/portfolio_budget_replay --config docs/superpowers/artifacts/glm-martingale-core-round5/promising/r5-G-best-ANKRUSDT.json --budget 5000 --start-ms 1672531200000 --end-ms 1780271999999 --market-data data/market_data_full.db --funding-data data/funding_rates.db --profile aggressive --portfolio-id r6-repro-ankr --exchange-min-notional 5
target/release/portfolio_budget_replay --config docs/superpowers/artifacts/glm-martingale-core-round5/promising/r5-fine-combo-best.json --budget 5000 --start-ms 1672531200000 --end-ms 1780271999999 --market-data data/market_data_full.db --funding-data data/funding_rates.db --profile aggressive --portfolio-id r6-repro-fine --exchange-min-notional 5
target/release/portfolio_budget_replay --config docs/superpowers/artifacts/glm-martingale-core-round4/promising/r4-combo-best.json --budget 5000 --start-ms 1672531200000 --end-ms 1780271999999 --market-data data/market_data_full.db --funding-data data/funding_rates.db --profile aggressive --portfolio-id r6-repro-r4 --exchange-min-notional 5
```

Commit initialization:

```bash
git add docs/superpowers/artifacts/glm-martingale-core-round6 docs/superpowers/reports/2026-07-03-glm-martingale-round6-search-ledger.md
git commit -m "docs: 修复思路 初始化Round6马丁回撤压缩账本"
```

## 6. Promotion Gates

Original target pass:

| Gate | Conservative | Balanced | Aggressive |
|---|---:|---:|---:|
| ann | `>50%` | `>90%` | `>110%` |
| DD | `<=10%` | `<=20%` | `<=30%` |
| positive segments | `>=4/5` | `>=4/5` | `>=3/5` |
| 2024-2026 aggregate | positive | positive | positive |

Near-frontier pass:

| Label | Gate |
|---|---|
| `NF-conservative-dd` | ann `>=50%`, DD `<=18%`, pos `>=4/5` |
| `NF-conservative-close` | ann `>=45%`, DD `<=12%`, pos `>=4/5` |
| `NF-balanced-dd` | ann `>=70%`, DD `<=22%`, pos `>=4/5` |
| `NF-aggressive-dd` | ann `>=80%`, DD `<=30%`, pos `>=3/5`, 2024-2026 positive |
| `NF-2025-repair` | 2025 `>=-5%`, full ann `>=40%`, DD `<=25%`, pos `>=4/5` |
| `NF-frontier` | improves over `r5-G-best-ANKRUSDT` in at least two of ann, DD, 2025, pos/5 without worsening the other two by more than `2pp` |

Reject a candidate if:

1. It depends on one segment for more than `60%` of total positive return.
2. It breaches `<5000U`.
3. It lowers DD by simply holding cash so ann drops below `30%`, unless it is a diagnostic row.
4. It cannot be implemented in trading-engine and lacks a blocker report.
5. It repeats a non-repeat key.

## 7. Task A: Drawdown-Window Attribution

**Goal:** Identify exact windows, symbols, directions, legs, and cycle states that create `r5-G-best-ANKRUSDT` DD `32.1%`. Round 5 microregime was broad; this task is drawdown-window specific.

**Files:**
- Create: `scripts/glm_r6_drawdown_window_attribution.py`
- Create: `docs/superpowers/artifacts/glm-martingale-core-round6/r6-drawdown-window-attribution.json`
- Create: `docs/superpowers/reports/2026-07-03-glm-round6-drawdown-window-attribution.md`
- Modify: `docs/superpowers/reports/2026-07-03-glm-martingale-round6-search-ledger.md`

Required output fields:

| Field | Meaning |
|---|---|
| peak timestamp, trough timestamp, recovery timestamp | portfolio DD window |
| symbol and direction contribution | realized plus unrealized PnL by strategy |
| safety leg count at trough | how deep each cycle was |
| skipped/blocked safety count | whether controls blocked or allowed averaging |
| TP stage before trough | whether partial profit was banked |
| ATR%, ADX, RSI, ROC720, premium | observable state at entry and at trough |
| candidate fix route | Task B/C/D/E/F/G |

- [ ] **Step A1: Run attribution**

```bash
python3 scripts/glm_r6_drawdown_window_attribution.py \
  --config docs/superpowers/artifacts/glm-martingale-core-round5/promising/r5-G-best-ANKRUSDT.json \
  --market-data data/market_data_full.db \
  --funding-data data/funding_rates.db \
  --premium-data data/premium_index.db \
  --start-ms 1672531200000 \
  --end-ms 1780271999999 \
  --out docs/superpowers/artifacts/glm-martingale-core-round6/r6-drawdown-window-attribution.json \
  --report docs/superpowers/reports/2026-07-03-glm-round6-drawdown-window-attribution.md
```

- [ ] **Step A2: Route findings**

Report table:

```markdown
| dd window | peak->trough DD | top loss strategy | top loss mechanism | observable trigger | route | reject-if |
```

Route map:

| Finding | Route |
|---|---|
| losses cluster after portfolio DD already exceeds threshold | Task B DD state machine |
| one symbol/direction dominates trough | Task C symbol quarantine |
| cycle had TP1 then gave back to stop | Task D partial-TP trailing lock |
| safety legs added during worsening portfolio DD | Task E safety-order freeze/cancel |
| ANKR high-return only needs smaller allocation | Task F risk-budget blend |
| loss comes from exchange/order mechanics | Task G live parity audit |

- [ ] **Step A3: Commit**

```bash
git add scripts/glm_r6_drawdown_window_attribution.py docs/superpowers/artifacts/glm-martingale-core-round6 docs/superpowers/reports/2026-07-03-glm-round6-drawdown-window-attribution.md docs/superpowers/reports/2026-07-03-glm-martingale-round6-search-ledger.md
git commit -m "docs: 修复思路 记录Round6 ANKR回撤窗口归因"
```

## 8. Task B: Portfolio Drawdown State Machine

**Goal:** Replace binary portfolio stop with staged throttling while preserving active martingale cycles. This is not `portfolio-stop-alone`: actions scale entry size, freeze safety legs, extend cooldown, and require lagged recovery before returning to high-risk mode.

**Files:**
- Modify: `crates/shared-domain/src/martingale.rs`
- Modify: `apps/backtest-engine/src/martingale/kline_engine.rs`
- Create: `scripts/glm_r6_drawdown_state_machine_search.py`
- Test: `apps/backtest-engine/src/martingale/kline_engine.rs`
- Output: `docs/superpowers/artifacts/glm-martingale-core-round6/r6-drawdown-state-machine.json`

Add config:

```rust
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct MartingaleDrawdownStateRule {
    pub trigger_drawdown_pct: f64,
    pub first_order_scale: Option<f64>,
    pub safety_order_scale: Option<f64>,
    pub max_new_cycles_per_symbol: Option<u32>,
    pub cooldown_seconds: Option<u64>,
    pub freeze_safety_orders: Option<bool>,
}
```

Add to `MartingaleRiskLimits`:

```rust
#[serde(default)]
pub drawdown_state_rules: Vec<MartingaleDrawdownStateRule>;
#[serde(default)]
pub drawdown_state_recovery_pct: Option<f64>;
```

Rules:

1. State uses portfolio peak-to-current equity known at the current bar.
2. Higher DD state overrides lower DD state.
3. Recovery requires equity to reclaim `drawdown_state_recovery_pct` of peak-to-trough loss.
4. Existing open cycles may TP or SL normally.
5. New safety orders can be scaled or frozen only after price trigger and before placement.

- [ ] **Step B1: Add failing tests**

```bash
cargo test -p backtest-engine dd_state_scales_new_entry_after_threshold -- --nocapture
cargo test -p backtest-engine dd_state_freezes_safety_orders_after_threshold -- --nocapture
cargo test -p backtest-engine dd_state_recovers_only_after_reclaim_fraction -- --nocapture
```

- [ ] **Step B2: Implement and run tests**

```bash
cargo test -p backtest-engine dd_state -- --nocapture
cargo test -p backtest-engine martingale::kline_engine -- --nocapture
```

- [ ] **Step B3: Run search**

```bash
python3 scripts/glm_r6_drawdown_state_machine_search.py \
  --base-config docs/superpowers/artifacts/glm-martingale-core-round5/promising/r5-G-best-ANKRUSDT.json \
  --out docs/superpowers/artifacts/glm-martingale-core-round6/r6-drawdown-state-machine.json \
  --workers 20
```

Grid:

| Field | Values |
|---|---|
| state thresholds | `6/10/14`, `8/12/16`, `10/15/20`, `12/18/24` |
| first-order scale by state | `0.8/0.6/0.4`, `0.7/0.5/0.3`, `1.0/0.5/0.25` |
| safety scale by state | `1.0/0.7/0.4`, `0.8/0.5/0.0`, `1.0/0.0/0.0` |
| cooldown multiplier | `1x`, `1.5x`, `2x`, `3x` |
| recovery reclaim | `0.25`, `0.50`, `0.75` |
| base candidates | ANKR, XRP, DOGE, fine-combo |

Promote if `NF-conservative-dd`, `NF-conservative-close`, or any original target passes.

- [ ] **Step B4: Commit**

```bash
git add crates/shared-domain apps/backtest-engine scripts/glm_r6_drawdown_state_machine_search.py docs/superpowers/artifacts/glm-martingale-core-round6 docs/superpowers/reports/2026-07-03-glm-martingale-round6-search-ledger.md
git commit -m "feat: 修复思路 增加Round6马丁组合回撤状态机"
```

## 9. Task C: Symbol And Direction Quarantine

**Goal:** If drawdown attribution shows one symbol/direction causes troughs, pause that sleeve using only past realized/unrealized loss evidence. This is not simple symbol swap; it is lagged quarantine plus recovery.

**Files:**
- Modify: `crates/shared-domain/src/martingale.rs`
- Modify: `apps/backtest-engine/src/martingale/kline_engine.rs`
- Create: `scripts/glm_r6_symbol_quarantine_search.py`
- Test: `apps/backtest-engine/src/martingale/kline_engine.rs`
- Output: `docs/superpowers/artifacts/glm-martingale-core-round6/r6-symbol-quarantine.json`

Add config:

```rust
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct MartingaleQuarantineRule {
    pub scope: String,
    pub loss_drawdown_pct: f64,
    pub stop_count_window_hours: f64,
    pub stop_count_trigger: u32,
    pub pause_hours: f64,
    pub recovery_expression: Option<String>,
}
```

Add to `MartingaleRiskLimits`:

```rust
#[serde(default)]
pub quarantine_rules: Vec<MartingaleQuarantineRule>;
```

Scopes:

| Scope | Meaning |
|---|---|
| `symbol` | Pause all strategies on one symbol. |
| `symbol_direction` | Pause only the losing direction on one symbol. |
| `direction` | Pause all longs or all shorts. |

- [ ] **Step C1: Add failing tests**

```bash
cargo test -p backtest-engine quarantine_pauses_symbol_after_stop_cluster -- --nocapture
cargo test -p backtest-engine quarantine_recovers_after_pause_and_expression -- --nocapture
```

- [ ] **Step C2: Implement and run tests**

```bash
cargo test -p backtest-engine quarantine -- --nocapture
```

- [ ] **Step C3: Run search**

```bash
python3 scripts/glm_r6_symbol_quarantine_search.py \
  --base-config docs/superpowers/artifacts/glm-martingale-core-round5/promising/r5-G-best-ANKRUSDT.json \
  --out docs/superpowers/artifacts/glm-martingale-core-round6/r6-symbol-quarantine.json \
  --workers 20
```

Grid:

| Field | Values |
|---|---|
| scope | `symbol`, `symbol_direction`, `direction` |
| DD trigger | `4%`, `6%`, `8%`, `10%`, `12%` |
| stop count window | `24h`, `72h`, `168h` |
| stop count trigger | `1`, `2`, `3` |
| pause hours | `24`, `72`, `168`, `336` |
| recovery expression | disabled, `close > ema(50)`, `rsi(14) between 45 and 60`, `BTCUSDT.close > BTCUSDT.ema(50)` |
| candidates | ANKR, XRP, DOGE, fine-combo |

Promote if DD improves by `>=8pp` and ann remains `>=45%`, or if any near-frontier gate passes.

- [ ] **Step C4: Commit**

```bash
git add crates/shared-domain apps/backtest-engine scripts/glm_r6_symbol_quarantine_search.py docs/superpowers/artifacts/glm-martingale-core-round6 docs/superpowers/reports/2026-07-03-glm-martingale-round6-search-ledger.md
git commit -m "feat: 修复思路 增加Round6马丁币种方向隔离"
```

## 10. Task D: Partial-TP Trailing Profit Lock

**Goal:** Many high-mult candidates likely give back gains after partial TP. Add a trailing stop only after TP stage or MFE threshold, so it remains a martingale exit rule. This differs from Round 4 active exit because it protects favorable excursion, not stale cycles.

**Files:**
- Modify: `crates/shared-domain/src/martingale.rs`
- Modify: `apps/backtest-engine/src/martingale/kline_engine.rs`
- Create: `scripts/glm_r6_partial_tp_trailing_lock_search.py`
- Test: `apps/backtest-engine/src/martingale/kline_engine.rs`
- Output: `docs/superpowers/artifacts/glm-martingale-core-round6/r6-partial-tp-trailing-lock.json`

Add to `MartingaleTakeProfitModel::Partial` or risk limits:

```rust
#[serde(default)]
pub trailing_lock_after_stage: Option<u32>;
#[serde(default)]
pub trailing_lock_activation_bps: Option<u32>;
#[serde(default)]
pub trailing_lock_callback_bps: Option<u32>;
#[serde(default)]
pub trailing_lock_floor_bps: Option<u32>;
```

Rules:

1. Disabled by default.
2. Arm after configured partial TP stage fires or after MFE reaches activation bps.
3. Long trailing stop follows high watermark; short follows low watermark.
4. Stop cannot be worse than breakeven plus `trailing_lock_floor_bps`.
5. Final close is a strategy stop event with reason `partial_tp_trailing_lock`.

- [ ] **Step D1: Add failing tests**

```bash
cargo test -p backtest-engine partial_tp_trailing_lock_arms_after_stage -- --nocapture
cargo test -p backtest-engine partial_tp_trailing_lock_closes_on_giveback -- --nocapture
cargo test -p backtest-engine partial_tp_trailing_lock_respects_breakeven_floor -- --nocapture
```

- [ ] **Step D2: Implement and run tests**

```bash
cargo test -p backtest-engine partial_tp_trailing_lock -- --nocapture
```

- [ ] **Step D3: Run search**

```bash
python3 scripts/glm_r6_partial_tp_trailing_lock_search.py \
  --base-config docs/superpowers/artifacts/glm-martingale-core-round5/promising/r5-G-best-ANKRUSDT.json \
  --out docs/superpowers/artifacts/glm-martingale-core-round6/r6-partial-tp-trailing-lock.json \
  --workers 20
```

Grid:

| Field | Values |
|---|---|
| arm after stage | `0`, `1`, `2` |
| activation bps | `600`, `800`, `1000`, `1400`, `1800` |
| callback bps | `150`, `250`, `400`, `600`, `900` |
| floor bps | `0`, `50`, `100`, `200` |
| TP ladder | `600/1200/2200`, `700/1400/2400`, `800/1600/2600` |
| candidates | ANKR, XRP, DOGE, fine-combo |

Promote if DD improves by `>=6pp` with ann `>=45%`, or if any near-frontier gate passes.

- [ ] **Step D4: Commit**

```bash
git add crates/shared-domain apps/backtest-engine scripts/glm_r6_partial_tp_trailing_lock_search.py docs/superpowers/artifacts/glm-martingale-core-round6 docs/superpowers/reports/2026-07-03-glm-martingale-round6-search-ledger.md
git commit -m "feat: 修复思路 增加Round6马丁分段止盈后追踪锁盈"
```

## 11. Task E: Safety-Order Freeze And Cancel After Profit Or DD State

**Goal:** Stop adding new averaging legs after a cycle has banked profit or the portfolio is already underwater. This is distinct from rebound SO and custom ladder: the trigger price is still normal martingale, but order eligibility changes after TP/DD state.

**Files:**
- Modify: `crates/shared-domain/src/martingale.rs`
- Modify: `apps/backtest-engine/src/martingale/kline_engine.rs`
- Create: `scripts/glm_r6_safety_freeze_cancel_search.py`
- Test: `apps/backtest-engine/src/martingale/kline_engine.rs`
- Output: `docs/superpowers/artifacts/glm-martingale-core-round6/r6-safety-freeze-cancel.json`

Add fields:

```rust
#[serde(default)]
pub freeze_safety_after_partial_tp_stage: Option<u32>;
#[serde(default)]
pub freeze_safety_when_portfolio_dd_pct: Option<f64>;
#[serde(default)]
pub cancel_unfilled_safety_after_partial_tp: Option<bool>;
```

Rules:

1. Freeze applies only to future unfilled safety legs.
2. Existing filled legs stay in the cycle and can TP/SL.
3. Cancel policy is represented in backtest as not placing any later safety leg after the stage fires.
4. Trading-engine parity must cancel open orders when this is promoted.

- [ ] **Step E1: Add failing tests**

```bash
cargo test -p backtest-engine freeze_safety_after_partial_tp_blocks_later_legs -- --nocapture
cargo test -p backtest-engine freeze_safety_when_portfolio_dd_blocks_later_legs -- --nocapture
```

- [ ] **Step E2: Implement and run tests**

```bash
cargo test -p backtest-engine freeze_safety -- --nocapture
```

- [ ] **Step E3: Run search**

```bash
python3 scripts/glm_r6_safety_freeze_cancel_search.py \
  --base-config docs/superpowers/artifacts/glm-martingale-core-round5/promising/r5-G-best-ANKRUSDT.json \
  --out docs/superpowers/artifacts/glm-martingale-core-round6/r6-safety-freeze-cancel.json \
  --workers 20
```

Grid:

| Field | Values |
|---|---|
| freeze after stage | disabled, `0`, `1`, `2` |
| freeze at portfolio DD | disabled, `8%`, `12%`, `16%`, `20%` |
| cancel after partial TP | disabled, enabled |
| combine with trailing lock | disabled, best Task D candidate |
| candidates | ANKR, XRP, DOGE, fine-combo |

Promote if 2025 improves by `>=8pp` or DD improves by `>=6pp` while ann remains `>=40%`.

- [ ] **Step E4: Commit**

```bash
git add crates/shared-domain apps/backtest-engine scripts/glm_r6_safety_freeze_cancel_search.py docs/superpowers/artifacts/glm-martingale-core-round6 docs/superpowers/reports/2026-07-03-glm-martingale-round6-search-ledger.md
git commit -m "feat: 修复思路 增加Round6马丁安全单冻结取消策略"
```

## 12. Task F: Risk-Budget Blend Of ANKR/XRP/DOGE/R4/Cash

**Goal:** Quantify whether high-return ANKR and lower-DD XRP/DOGE/R4 can combine non-linearly. Round 5 allocator rejected BCH vs ANKR by ann dominance; Round 6 allocator optimizes DD and segment stability, not ann only.

**Files:**
- Create: `scripts/glm_r6_risk_budget_blend.py`
- Output: `docs/superpowers/artifacts/glm-martingale-core-round6/r6-risk-budget-blend.json`
- Report: `docs/superpowers/reports/2026-07-03-glm-round6-risk-budget-blend.md`

Candidate library:

| Label | Source |
|---|---|
| `ANKR` | `r5-G-best-ANKRUSDT.json` |
| `XRP` | extract `replaceL2_XRPUSDT` from `r5-symbol-universe-expansion.json` |
| `DOGE` | extract `replaceL2_DOGEUSDT` from `r5-symbol-universe-expansion.json` |
| `R4` | `r4-combo-best.json` |
| `fine` | `r5-fine-combo-best.json` |
| `cash` | no strategy |

Rules:

1. Use event-level portfolio curves when available; otherwise rerun each candidate and merge curves by timestamp.
2. Allocation must sum to `<=100%`; unused is cash.
3. No single active symbol can exceed `35%` planned weighted margin.
4. Dynamic switching must use only prior 30d or 60d realized curve metrics.
5. Static blend is valid only if it improves DD without dropping ann below `40%`.

- [ ] **Step F1: Build static blend frontier**

```bash
python3 scripts/glm_r6_risk_budget_blend.py \
  --mode static-frontier \
  --out docs/superpowers/artifacts/glm-martingale-core-round6/r6-risk-budget-blend-static.json \
  --report docs/superpowers/reports/2026-07-03-glm-round6-risk-budget-blend.md \
  --workers 12
```

Grid:

| Allocation component | Values |
|---|---|
| ANKR | `0%`, `20%`, `40%`, `60%`, `80%`, `100%` |
| XRP | `0%`, `20%`, `40%`, `60%` |
| DOGE | `0%`, `20%`, `40%`, `60%` |
| R4/fine | `0%`, `20%`, `40%`, `60%` |
| cash | residual |

- [ ] **Step F2: Run drawdown-aware dynamic blend only if static frontier is promising**

```bash
python3 scripts/glm_r6_risk_budget_blend.py \
  --mode rolling-dd-budget \
  --out docs/superpowers/artifacts/glm-martingale-core-round6/r6-risk-budget-blend-dynamic.json \
  --report docs/superpowers/reports/2026-07-03-glm-round6-risk-budget-blend.md \
  --workers 12
```

Dynamic rules:

| Field | Values |
|---|---|
| lookback | `14d`, `30d`, `60d` |
| rebalance cadence | `7d`, `14d`, `30d` |
| DD budget | `10%`, `15%`, `20%`, `25%` |
| max ANKR allocation | `20%`, `40%`, `60%`, `80%` |
| switch to cash after DD | `6%`, `10%`, `14%` |

Promote if `NF-conservative-close`, `NF-conservative-dd`, or any original target passes.

- [ ] **Step F3: Commit**

```bash
git add scripts/glm_r6_risk_budget_blend.py docs/superpowers/artifacts/glm-martingale-core-round6 docs/superpowers/reports/2026-07-03-glm-round6-risk-budget-blend.md docs/superpowers/reports/2026-07-03-glm-martingale-round6-search-ledger.md
git commit -m "docs: 修复思路 记录Round6马丁风险预算组合"
```

## 13. Task G: Trading-Engine Parity For R5/R6 Promoted Features

**Goal:** Do not let a backtest-only candidate become live-ready. Implement parity for `last-exec SO` and `vol-target` first, then any R6 promoted feature.

**Files:**
- Modify: `apps/trading-engine/src/martingale_runtime.rs`
- Modify: `apps/trading-engine/src/main.rs`
- Modify: `apps/trading-engine/tests/martingale_runtime.rs`
- Create: `docs/superpowers/reports/2026-07-03-glm-round6-live-parity-report.md`

Required tests:

```bash
cargo test -p trading-engine martingale_last_executed_basis_uses_previous_fill_price -- --nocapture
cargo test -p trading-engine martingale_vol_target_scales_first_order_from_atr_context -- --nocapture
cargo test -p trading-engine martingale_partial_tp_fractional_reduce_only_close_is_supported -- --nocapture
cargo test -p trading-engine martingale_algo_order_count_budget_is_reported -- --nocapture
```

Implementation rules:

1. `last-exec SO` in trading-engine must compute next trigger from last filled leg price.
2. `vol-target` must use the same ATR period and clamp as backtest.
3. Partial TP must support fractional reduce-only close quantity; if not implemented, final report marks all partial TP candidates as conservative approximation, not exact parity.
4. Algo-order count report must estimate normal orders and algo orders per symbol using Binance filters.
5. If live exchangeInfo cannot be fetched, use committed fixture and mark source.

- [ ] **Step G1: Implement mandatory R5 parity**

```bash
cargo test -p trading-engine martingale_last_executed_basis -- --nocapture
cargo test -p trading-engine martingale_vol_target -- --nocapture
```

- [ ] **Step G2: Write report**

```text
docs/superpowers/reports/2026-07-03-glm-round6-live-parity-report.md
```

Report table:

```markdown
| Feature | backtest status | trading-engine status | test evidence | live blocker |
```

- [ ] **Step G3: Commit**

```bash
git add apps/trading-engine docs/superpowers/reports/2026-07-03-glm-round6-live-parity-report.md docs/superpowers/reports/2026-07-03-glm-martingale-round6-search-ledger.md
git commit -m "feat: 修复思路 补齐Round6马丁关键实盘等价"
```

## 14. Task H: Run Or Supersede `glm_r5_ankr_lowdd_search.py`

**Goal:** The local branch has `scripts/glm_r5_ankr_lowdd_search.py` committed but no result artifact. Round 6 must close that evidence gap.

**Files:**
- Existing: `scripts/glm_r5_ankr_lowdd_search.py`
- Output: `docs/superpowers/artifacts/glm-martingale-core-round6/r6-ankr-lowdd-closeout.json`
- Modify: `docs/superpowers/reports/2026-07-03-glm-martingale-round6-search-ledger.md`

- [ ] **Step H1: Run the script if not superseded by Task B/C/F**

```bash
python3 scripts/glm_r5_ankr_lowdd_search.py \
  --out docs/superpowers/artifacts/glm-martingale-core-round6/r6-ankr-lowdd-closeout.json \
  --workers 20
```

- [ ] **Step H2: Record closeout**

Ledger must state one of:

| Decision | Required evidence |
|---|---|
| `completed` | result path, candidate count, top 20, target hits count |
| `superseded-by-r6-B` | Task B result path and why it covers all lower-DD controls |
| `superseded-by-r6-C` | Task C result path and why it covers symbol/DD controls |
| `rejected-script-only` | explicit note that script had no result and was replaced by deterministic Round 6 search |

- [ ] **Step H3: Commit**

```bash
git add scripts/glm_r5_ankr_lowdd_search.py docs/superpowers/artifacts/glm-martingale-core-round6 docs/superpowers/reports/2026-07-03-glm-martingale-round6-search-ledger.md
git commit -m "docs: 修复思路 关闭Round5 ANKR低DD脚本证据缺口"
```

## 15. Final Validation Commands

Every promising candidate must run full period:

```bash
target/release/portfolio_budget_replay --config docs/superpowers/artifacts/glm-martingale-core-round6/promising/<candidate>.json --budget 5000 --start-ms 1672531200000 --end-ms 1780271999999 --market-data data/market_data_full.db --funding-data data/funding_rates.db --profile aggressive --portfolio-id r6-final-full --exchange-min-notional 5
```

And all five segments:

```bash
target/release/portfolio_budget_replay --config docs/superpowers/artifacts/glm-martingale-core-round6/promising/<candidate>.json --budget 5000 --start-ms 1672531200000 --end-ms 1688169599999 --market-data data/market_data_full.db --funding-data data/funding_rates.db --profile aggressive --portfolio-id r6-h1-2023 --exchange-min-notional 5
target/release/portfolio_budget_replay --config docs/superpowers/artifacts/glm-martingale-core-round6/promising/<candidate>.json --budget 5000 --start-ms 1688169600000 --end-ms 1704067199999 --market-data data/market_data_full.db --funding-data data/funding_rates.db --profile aggressive --portfolio-id r6-h2-2023 --exchange-min-notional 5
target/release/portfolio_budget_replay --config docs/superpowers/artifacts/glm-martingale-core-round6/promising/<candidate>.json --budget 5000 --start-ms 1704067200000 --end-ms 1735689599999 --market-data data/market_data_full.db --funding-data data/funding_rates.db --profile aggressive --portfolio-id r6-2024 --exchange-min-notional 5
target/release/portfolio_budget_replay --config docs/superpowers/artifacts/glm-martingale-core-round6/promising/<candidate>.json --budget 5000 --start-ms 1735689600000 --end-ms 1767225599999 --market-data data/market_data_full.db --funding-data data/funding_rates.db --profile aggressive --portfolio-id r6-2025 --exchange-min-notional 5
target/release/portfolio_budget_replay --config docs/superpowers/artifacts/glm-martingale-core-round6/promising/<candidate>.json --budget 5000 --start-ms 1767225600000 --end-ms 1780271999999 --market-data data/market_data_full.db --funding-data data/funding_rates.db --profile aggressive --portfolio-id r6-2026-ytd --exchange-min-notional 5
```

Candidate JSON summary:

```json
{
  "candidate_id": "r6-B-dd-state-best",
  "family": "portfolio_drawdown_state_machine",
  "martingale_core": true,
  "research_only": false,
  "live_parity_status": "passed",
  "budget_quote": 5000,
  "full": {
    "annualized_return_pct": 52.0,
    "max_drawdown_pct": 17.5,
    "total_return_pct": 210.0,
    "positive_segments": 4,
    "principal_breached": false
  },
  "segments": {
    "h1_2023": {"return_pct": 18.0, "annualized_return_pct": 40.0, "max_drawdown_pct": 9.0},
    "h2_2023": {"return_pct": 4.0, "annualized_return_pct": 8.0, "max_drawdown_pct": 8.0},
    "2024": {"return_pct": 35.0, "annualized_return_pct": 35.0, "max_drawdown_pct": 16.0},
    "2025": {"return_pct": -4.0, "annualized_return_pct": -4.0, "max_drawdown_pct": 18.0},
    "2026_ytd": {"return_pct": 7.0, "annualized_return_pct": 18.0, "max_drawdown_pct": 8.0}
  },
  "acceptance": {
    "conservative": false,
    "balanced": false,
    "aggressive": false,
    "near_frontier": ["NF-conservative-dd"],
    "first_failed_gate": "DD above 10 for conservative"
  }
}
```

## 16. Handoff Back To ChatGPT

When GLM finishes or pauses, write:

```text
docs/superpowers/reports/2026-07-03-glm-round6-handoff-to-chatgpt.md
```

Handoff sections:

1. Branch and latest commit.
2. Registry line count and ledger path.
3. Baseline reproduction for R5 ANKR, R5 fine, R4 combo.
4. Drawdown-window attribution summary.
5. Task B-F result table with candidate count, wall time, best full result, five segments, decision, non-repeat key.
6. Task G live-parity status and tests.
7. Task H closeout for `glm_r5_ankr_lowdd_search.py`.
8. Any original target pass.
9. Near-frontier shortlist with config paths.
10. Rejected families that future agents must not repeat.
11. Open blockers needing user approval.

Commit and push:

```bash
git add docs/superpowers/reports/2026-07-03-glm-round6-handoff-to-chatgpt.md docs/superpowers/artifacts/glm-martingale-core-round6 docs/superpowers/reports/2026-07-03-glm-martingale-round6-search-ledger.md
git commit -m "docs: 修复思路 Round6马丁回撤压缩搜索交接"
git push -u origin glm-martingale-core-round6
```

## 17. Long-Run Operating Rule

Long searches are allowed, but every long job must have:

1. Registry `running` line.
2. Manifest with git commit, data sha256, candidate grid size, worker count, command.
3. 2025-first smoke before full grid unless the task is explicitly full-frontier.
4. Full five-segment validation for promoted candidates.
5. Failure row with non-repeat key.

Round 6 may close individual directions, but it must not stop the overall search until Tasks A-H are either completed, superseded with evidence, or rejected with a non-repeat key.
