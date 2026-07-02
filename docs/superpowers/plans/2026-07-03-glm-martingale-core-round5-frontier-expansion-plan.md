# GLM Martingale Core Round 5 Frontier Expansion Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 在 Round 4 `r4-combo-best` 的基础上，继续以马丁/DCA/grid cycle 为核心，穷尽尚未系统验证的马丁原生机制，寻找满足保守、平衡、激进三档收益和回撤目标的小资金多币种组合。

**Architecture:** Round 5 不再重复普通 entry gate、TP/cooldown、fixed spacing、BTC breadth、premium 单阈值这些已经失败的方向。先锁定 Round 4 证据和失败账本，再用“周期级微观归因”驱动新机制：last-executed safety order、盈亏后首单动态缩放、波动率目标化、custom price ladder 的 dip/breakout 双形态、同币双向 hedge-grid 马丁、扩展币种库和实盘 algo-order 约束。所有探索都必须写入 Round 5 registry 和 ledger，失败必须留下 non-repeat key。

**Tech Stack:** Rust `backtest-engine` / `trading-engine` / `portfolio_budget_replay`, Python research runners in `scripts/`, SQLite `data/market_data_full.db`, `data/funding_rates.db`, `data/premium_index.db`, Git branch `glm-martingale-core-round5`.

---

## 0. 不可偏离的策略边界

Round 5 继续执行用户要求：

1. 收益来源必须是马丁/DCA/grid cycle；趋势、突破、premium、funding、pair-neutral 只能作为马丁开仓、补仓、退出、权重和暂停的辅助信号。
2. 不允许把核心改成纯趋势、纯突破、纯 funding、纯 pair-neutral、纯统计套利。
3. 最终候选必须 `<5000U`，多币种，五段验证，当前仓库可复跑，并具备 trading-engine 实盘路径。
4. 每个失败探索都要写入 registry 和 ledger，下轮禁止重复同一 non-repeat key。
5. 不启动 Binance 实盘、不接入真实资金、不把候选自动发布给用户账户。

三档目标保持不变：

| Profile | Annualized return | Max DD | Segment stability |
|---|---:|---:|---:|
| Conservative | `>50%` | `<=10%` | `>=4/5` positive, 2024-2026 aggregate positive |
| Balanced | `>90%` | `<=20%` | `>=4/5` positive, 2024-2026 aggregate positive |
| Aggressive | `>110%` | `<=30%` | `>=3/5` positive, prefer `>=4/5` |

固定五段窗口：

| Segment | Start ms | End ms |
|---|---:|---:|
| h1_2023 | `1672531200000` | `1688169599999` |
| h2_2023 | `1688169600000` | `1704067199999` |
| 2024 | `1704067200000` | `1735689599999` |
| 2025 | `1735689600000` | `1767225599999` |
| 2026_ytd | `1767225600000` | `1780271999999` |

## 1. Round 4 Baseline And Gap

GLM 先读这些文件，不要从记忆继续：

```bash
sed -n '1,260p' docs/superpowers/reports/2026-07-02-glm-4round-final-report.md
sed -n '1,220p' docs/superpowers/reports/2026-07-02-glm-round4-2025-cycle-attribution.md
sed -n '1,240p' docs/superpowers/reports/2026-07-02-glm-round4-final-parity.md
sed -n '1,260p' docs/superpowers/artifacts/glm-martingale-core-round4/exploration-registry.jsonl
sed -n '1,260p' docs/superpowers/artifacts/glm-martingale-core-round4/promising/r4-combo-best.json
```

Current best:

| Candidate | ann | DD | pos/5 | Budget | Symbols | 2025 |
|---|---:|---:|---:|---:|---|---:|
| `r4-combo-best` | `34.7%` | `17.7%` | `4/5` | `5000U`, max used about `1000U` | BNB/TRX/BCH long + AAVE/SOL/DOT short | `-8.70%` |

Round 4 best structure:

1. Long side: BNB/TRX/BCH strict gate, Partial TP `800/1600/2600`, multiplier `2.8`, max legs `8`, cooldown `11h`.
2. Short side: AAVE/SOL/DOT pump-fade, `roc(720)>18 AND rsi>65`, Partial TP `800/1600/2600`, multiplier `1.8`, max legs `5`, cooldown `11h`.
3. Live parity claimed for Conditional SO, Multi-stage Partial TP approximation, Breakeven, Equity-Reclaim.

Current gap:

| Target | Required | Best | Gap |
|---|---:|---:|---:|
| Conservative return | `>50%` | `34.7%` | `15.3pp` |
| Conservative DD | `<=10%` | `17.7%` | `7.7pp` |
| Balanced return | `>90%` | `34.7%` | `55.3pp` |
| Aggressive return | `>110%` | `34.7%` | `75.3pp` |
| 2025 segment | positive preferred | `-8.70%` | `8.70pp` to zero |

## 2. 2026-07-03 External Search Signals

这些外部资料只提供机制启发，不构成目标通过证据：

| Source | Useful signal for Round 5 | Constraint |
|---|---|---|
| 3Commas condition-based averaging orders: https://help.3commas.io/en/articles/9663694-dca-bot-averaging-orders-by-technical-indicators | Averaging orders can require both technical indicator confirmation and minimum deviation. This maps to conditional safety orders, not standalone signals. | Must remain a martingale safety-order gate. |
| 3Commas averaging order settings: https://help.3commas.io/en/articles/11983699-dca-bot-averaging-order-settings-explained | Current DCA bots expose standard DCA and Custom Price Ladder modes. | Use as ladder/sizing inspiration only. |
| 3Commas custom price ladder: https://help.3commas.io/en/articles/11984169-dca-bot-using-custom-price-ladder | Custom ladder can target dips, breakouts, or both, so Round 5 may test average-down and tightly capped average-up legs inside one DCA cycle. | If profit mainly comes from continuation legs and not DCA recovery cycles, mark as martingale-core violation. |
| 3Commas reinvesting/risk-reduction: https://3commas.io/blog/automated-reinvesting-risk-reduction-upgrades-dca-bots | Realized-profit reinvest and loss-side risk reduction are public DCA features. | Use only lagged closed-cycle results; no future segment labels. |
| 3Commas DCA feature page: https://3commas.io/dca-bots | Multi-pair, reinvest/risk reduction, multiple TP with trailing, stop loss with trailing/breakeven are established bot controls. | These controls must be replayed locally and made live-parity before promotion. |
| Binance New Algo Order: https://developers.binance.com/docs/derivatives/usds-margined-futures/trade/rest-api/New-Algo-Order | USD-M Futures TP/SL/trailing conditional orders use `/fapi/v1/algoOrder`. | Any final live candidate must account for algo-order limits and cancellation. |
| Binance derivatives change log: https://developers.binance.com/docs/derivatives/change-log | USD-M conditional orders migrate to Algo Service effective 2025-12-09. | Old conditional-order assumptions cannot be accepted for live parity. |
| Binance exchangeInfo: https://developers.binance.com/docs/derivatives/usds-margined-futures/market-data/rest-api/Exchange-Information | Current symbol filters and trading rules come from `GET /fapi/v1/exchangeInfo`. | Replace the approximate `exchange-min-notional 5` promotion check with exact filters before live-ready status. |
| Binance common definition: https://developers.binance.com/docs/derivatives/usds-margined-futures/common-definition | Algo-order and normal-order count filters matter for multi-stage TP/SL. | Reject live promotion if order-count usage cannot fit filters. |
| Binance Premium/Mark/Index Klines: https://developers.binance.com/docs/derivatives/usds-margined-futures/market-data/rest-api/Premium-Index-Kline-Data, https://developers.binance.com/docs/derivatives/usds-margined-futures/market-data/rest-api/Mark-Price-Kline-Candlestick-Data, https://developers.binance.com/docs/derivatives/usds-margined-futures/market-data/rest-api/Index-Price-Kline-Candlestick-Data | Round 4 premium data exists locally and can support richer premium states than a single BTC threshold. | Premium remains a gate, not a separate return source. |
| Binance Data Vision: https://data.binance.vision/ and https://github.com/binance/binance-public-data | Official bulk files can extend free historical OHLCV coverage. | Commit checksums and manifests; do not commit large DB files. |
| Dynamic Grid Trading paper: https://arxiv.org/abs/2506.11921 | Traditional grid can have weak expected value; dynamic reset/re-anchor can improve grid behavior. | Prior local DGT failed the user gates; Round 5 may only test re-anchor as a martingale cycle modifier. |

## 3. Round 1-4 Non-Repeat Matrix

Do not repeat these searches as-is:

| Family | Evidence | Non-repeat key | Allowed mutation |
|---|---|---|---|
| Pure regime gate scan | Round 1: `0/3276` passed segment gates | `repeat-regime-gate-grid` | Use only if gate is generated from cycle-level micro attribution and validated out-of-sample. |
| Portfolio DD stop alone | Round 1 lowered DD but killed return | `portfolio-stop-alone` | Use only as live circuit breaker, not as expected return engine. |
| High budget variants | Round 1 worsened segment stability | `increase-budget-to-solve-return` | Budget stays `<5000U`. |
| Plain tight SL scan | DD control led to negative ann | `tight-sl-only` | Use only paired with dynamic deal-size/risk reduction. |
| Funding window | Round 2 found funding too small | `funding-yield-as-bridge` | Funding can appear only as gate/cost, not as return target source. |
| Simple symbol health substitutions | Round 2 base6 remained best | `simple-symbol-swap-base6` | Expanded universe must use coverage/liquidity prefilters and OOS validation. |
| Inventory caps | Round 2 caps did not bind | `static-cap-skew-nonbinding` | Use dynamic first-order/max-leg scaling instead. |
| Fixed spacing alternatives / DGT spacing | Round 2 and Round 3 found fixed150 best | `plain-spacing-rescan` | Test last-executed basis, volatility target, or custom ladder shape only. |
| Direction-aware SO ordinary conditions | Round 3 found long `rsi<45`, short `none` best | `ordinary-so-condition-rescan` | New SO must change basis, sizing, trigger sequence, or use cycle health. |
| Breadth/BTC proxy | Round 3 worsened 2025 | `btc-breadth-proxy` | Use cycle micro-regime or cross-symbol health derived only from past closed cycles. |
| Static core-satellite | Round 3 raised ann to `35.0%` but DD `22.5%` | `static-core-satellite` | Dynamic allocator must allocate to cash when prior score is weak. |
| Long gate relaxation | Round 4 worsened 2025/2026 | `long-gate-relaxation` | Do not remove strict gate alone. |
| Low-ADX range sleeve | Round 4 barely fired and rejected | `low-adx-range-sleeve` | Range logic may return only if it uses cycle-derived high-volatility chop definition, not plain ADX threshold. |
| Four-stage TP | Round 4 no improvement | `four-stage-tp-rescan` | Do not rescan stage count; trailing after partial TP is allowed as a distinct exit mode. |
| Active stale exit | Round 4 no improvement | `max-age-no-progress-exit` | Do not repeat max-age/no-progress alone. |
| Rebound SO only | Round 4 no improvement | `rebound-so-only` | Rebound can combine with last-executed basis or dynamic sizing only. |
| Premium single threshold | Round 4 weak signal | `single-premium-threshold` | Use premium state features jointly with price/pump cycle features. |
| Pump-fade `roc(720)>18+rsi>65` | Round 4 marginal improvement only | `single-roc-pump-fade` | New pump-fade must change safety-order basis, sizing, or symbol pool. |

## 4. Round 5 Evidence Layout

Create these paths:

```bash
mkdir -p docs/superpowers/artifacts/glm-martingale-core-round5/promising
mkdir -p docs/superpowers/artifacts/glm-martingale-core-round5/rejected
mkdir -p docs/superpowers/artifacts/glm-martingale-core-round5/run-manifests
touch docs/superpowers/reports/2026-07-03-glm-martingale-round5-search-ledger.md
touch docs/superpowers/artifacts/glm-martingale-core-round5/exploration-registry.jsonl
```

Every exploration writes a registry record before running and updates it after completion. Use one JSON object per line:

```json
{"exploration_id":"r5-B-last-executed-so-001","started_at_utc":"2026-07-03T00:00:00Z","family":"last_executed_safety_order_basis","hypothesis":"Using last executed leg as the next safety-order reference reduces clustered adds in choppy 2025 while preserving martingale recovery cycles.","new_vs_prior":"Rounds 2-4 tested fixed/custom distances from the original anchor, not last-executed deviation basis.","martingale_core":true,"research_only":false,"input_signature":"basis=last_executed;step=120-250;scale=1.0-1.4;core=r4-combo-best","command":"python3 scripts/glm_r5_last_executed_so_search.py --out docs/superpowers/artifacts/glm-martingale-core-round5/r5-last-executed-so-grid.json --workers 20","status":"running","result_path":"docs/superpowers/artifacts/glm-martingale-core-round5/r5-last-executed-so-grid.json","decision":"pending","non_repeat_key":"last-executed-safety-order-basis"}
```

Ledger entry format:

```markdown
## r5-B-last-executed-so-001

- Hypothesis:
- New vs prior:
- Command:
- Candidate count:
- Wall time:
- Best full result:
- Segment table:
- Gate result:
- Decision:
- Non-repeat key:
```

No registry line and ledger section means the result cannot be used in a handoff.

## 5. Worktree And Starting Commands

Start Round 5 from the current pushed Round 4 branch:

```bash
git status --short --branch
git fetch origin
git worktree add .worktrees/glm-martingale-core-round5 -b glm-martingale-core-round5 origin/glm-martingale-core-round4
cd .worktrees/glm-martingale-core-round5
git status --short --branch
```

Build replay binary before long jobs:

```bash
cargo build --release -p backtest-engine
test -x target/release/portfolio_budget_replay
```

Record data fingerprints:

```bash
sha256sum data/market_data_full.db data/funding_rates.db data/premium_index.db > docs/superpowers/artifacts/glm-martingale-core-round5/run-manifests/r5-data-sha256.txt
sqlite3 data/market_data_full.db "SELECT COUNT(*) AS rows, COUNT(DISTINCT symbol) AS symbols, MIN(open_time), MAX(open_time) FROM klines;" > docs/superpowers/artifacts/glm-martingale-core-round5/run-manifests/r5-market-data-coverage.txt
sqlite3 data/premium_index.db "SELECT COUNT(*) AS rows, COUNT(DISTINCT symbol) AS symbols, MIN(open_time), MAX(open_time) FROM premium_index;" > docs/superpowers/artifacts/glm-martingale-core-round5/run-manifests/r5-premium-data-coverage.txt
```

Commit the initialized evidence files:

```bash
git add docs/superpowers/artifacts/glm-martingale-core-round5 docs/superpowers/reports/2026-07-03-glm-martingale-round5-search-ledger.md
git commit -m "docs: 修复思路 初始化Round5马丁搜索账本"
```

## 6. Promotion And Stop Gates

Original target promotion:

| Gate | Conservative | Balanced | Aggressive |
|---|---:|---:|---:|
| annualized return | `>50%` | `>90%` | `>110%` |
| max DD | `<=10%` | `<=20%` | `<=30%` |
| positive segments | `>=4/5` | `>=4/5` | `>=3/5` |
| 2024-2026 aggregate | positive | positive | positive |
| budget | `<5000U` | `<5000U` | `<5000U` |

Near-frontier promotion for continued search:

| Label | Gate |
|---|---|
| `NF-2025-flip` | 2025 `>=0`, full ann `>=30%`, DD `<=20%`, pos `>=4/5` |
| `NF-conservative-near` | ann `>=45%`, DD `<=12%`, pos `>=4/5` |
| `NF-balanced-near` | ann `>=60%`, DD `<=22%`, pos `>=4/5` |
| `NF-aggressive-near` | ann `>=80%`, DD `<=32%`, pos `>=3/5`, 2024-2026 positive |
| `NF-frontier-improve` | ann improves over `34.7%` by `>=5pp`, DD `<=18%`, and 2025 `>=-5%` |

Stop a direction when:

1. It matches a non-repeat key above.
2. 2025-first smoke is worse than `r4-combo-best` on both return and DD.
3. The only improvement comes from exceeding `<5000U`.
4. Full-period improvement depends on one segment contributing more than `60%` of total positive PnL.
5. It cannot be implemented with current Binance USD-M order/filter rules.
6. It has no registry line, no command, or no segment table.

## 7. Task A: Cycle Micro-Regime Attribution

**Goal:** Stop blind parameter sweeps. Extract per-cycle features from Round 4 best and identify which observable states produce profitable or losing martingale cycles.

**Files:**
- Create: `scripts/glm_r5_cycle_microregime.py`
- Create: `docs/superpowers/artifacts/glm-martingale-core-round5/r5-cycle-microregime.json`
- Create: `docs/superpowers/reports/2026-07-03-glm-round5-cycle-microregime.md`
- Modify: `docs/superpowers/reports/2026-07-03-glm-martingale-round5-search-ledger.md`

Required cycle fields:

| Field | Source |
|---|---|
| symbol, direction, cycle id | replay events |
| entry timestamp and exit timestamp | replay events |
| realized PnL, fees, funding | replay result/events |
| MAE, MFE, duration hours | OHLCV replay during active cycle |
| entry ATR percent, ADX, RSI, EMA50 distance, EMA200 distance, ROC720 | indicator runtime or Python recompute |
| premium state | `data/premium_index.db` |
| prior closed-cycle return over 7d/30d by symbol and direction | lagged cycle table |
| portfolio concurrent active cycles | replay events |

- [ ] **Step A1: Create cycle table script**

The script must accept:

```bash
python3 scripts/glm_r5_cycle_microregime.py \
  --config docs/superpowers/artifacts/glm-martingale-core-round4/promising/r4-combo-best.json \
  --market-data data/market_data_full.db \
  --funding-data data/funding_rates.db \
  --premium-data data/premium_index.db \
  --start-ms 1672531200000 \
  --end-ms 1780271999999 \
  --out docs/superpowers/artifacts/glm-martingale-core-round5/r5-cycle-microregime.json \
  --report docs/superpowers/reports/2026-07-03-glm-round5-cycle-microregime.md
```

- [ ] **Step A2: Run and segment-check cycle feature stability**

The report must include:

```markdown
| feature bucket | cycles | full avg pnl | h1_2023 | h2_2023 | 2024 | 2025 | 2026_ytd | decision |
```

Decision values:

| Value | Meaning |
|---|---|
| `promote-gate-candidate` | Bucket improves at least 4 segments and has enough cycles in every tested segment. |
| `reject-overfit` | Bucket only works in one segment or one symbol. |
| `reject-thin-sample` | Fewer than 30 cycles full period or fewer than 5 cycles in at least 3 segments. |
| `route-to-task` | Bucket is promising but needs Task B/C/D/E/F implementation. |

- [ ] **Step A3: Commit**

```bash
git add scripts/glm_r5_cycle_microregime.py docs/superpowers/artifacts/glm-martingale-core-round5 docs/superpowers/reports/2026-07-03-glm-round5-cycle-microregime.md docs/superpowers/reports/2026-07-03-glm-martingale-round5-search-ledger.md
git commit -m "docs: 修复思路 记录Round5马丁周期微观归因"
```

## 8. Task B: Last-Executed Safety Order Basis

**Goal:** Test a DCA bot mode not covered by Rounds 2-4: next safety order deviation is measured from the last executed leg instead of the original base order. This remains martingale because it only changes where averaging orders fire.

**Files:**
- Modify: `crates/shared-domain/src/martingale.rs`
- Modify: `apps/backtest-engine/src/martingale/rules.rs`
- Modify: `apps/backtest-engine/src/martingale/kline_engine.rs`
- Create: `scripts/glm_r5_last_executed_so_search.py`
- Test: `apps/backtest-engine/src/martingale/rules.rs`
- Output: `docs/superpowers/artifacts/glm-martingale-core-round5/r5-last-executed-so-grid.json`

New config field:

```rust
pub enum MartingaleSafetyOrderBasis {
    BaseOrder,
    LastExecutedOrder,
}
```

Add to `MartingaleRiskLimits`:

```rust
#[serde(default)]
pub safety_order_basis: Option<MartingaleSafetyOrderBasis>;
```

Rules:

1. `None` and `BaseOrder` preserve current behavior.
2. `LastExecutedOrder` computes the next trigger from the previous filled leg price.
3. For long, next trigger = previous leg price × `(1 - step_bps / 10000)`.
4. For short, next trigger = previous leg price × `(1 + step_bps / 10000)`.
5. Existing conditional SO and rebound SO still apply after trigger.

- [ ] **Step B1: Add failing tests**

```bash
cargo test -p backtest-engine last_executed_basis_long_uses_previous_leg_price -- --nocapture
cargo test -p backtest-engine last_executed_basis_short_uses_previous_leg_price -- --nocapture
```

Expected before implementation: the tests fail because `safety_order_basis` is not available.

- [ ] **Step B2: Implement minimal backtest support**

Run:

```bash
cargo test -p backtest-engine last_executed_basis -- --nocapture
cargo test -p backtest-engine martingale::rules -- --nocapture
```

Expected after implementation: targeted tests pass and existing base-order behavior remains unchanged.

- [ ] **Step B3: Search grid**

```bash
python3 scripts/glm_r5_last_executed_so_search.py \
  --base-config docs/superpowers/artifacts/glm-martingale-core-round4/promising/r4-combo-best.json \
  --out docs/superpowers/artifacts/glm-martingale-core-round5/r5-last-executed-so-grid.json \
  --workers 20
```

Grid:

| Field | Values |
|---|---|
| basis | `base_order`, `last_executed_order` |
| long step bps | `100`, `120`, `150`, `180`, `220` |
| short step bps | `120`, `150`, `180`, `250`, `350` |
| long multiplier | `2.2`, `2.5`, `2.8`, `3.1` |
| short multiplier | `1.4`, `1.6`, `1.8`, `2.1` |
| max legs long/short | `6/4`, `7/5`, `8/5`, `8/6` |
| combine rebound | `none`, `20bps`, `40bps` |

Promote if:

1. `NF-frontier-improve`, or
2. 2025 improves by `>=5pp` while DD does not increase, or
3. any original target passes.

- [ ] **Step B4: Commit**

```bash
git add crates/shared-domain apps/backtest-engine scripts/glm_r5_last_executed_so_search.py docs/superpowers/artifacts/glm-martingale-core-round5 docs/superpowers/reports/2026-07-03-glm-martingale-round5-search-ledger.md
git commit -m "feat: 修复思路 增加Round5按上次成交价计算安全单"
```

## 9. Task C: Profit-Reinvest And Loss Risk-Reduction Deal Sizing

**Goal:** Test DCA-native dynamic deal sizing. After realized profit, allow small reinvestment into future base orders. After realized loss or drawdown, reduce first order and/or max legs for a cooldown window. This is still martingale because every active trade is a normal DCA/grid cycle.

**Files:**
- Modify: `crates/shared-domain/src/martingale.rs`
- Modify: `apps/backtest-engine/src/martingale/kline_engine.rs`
- Create: `scripts/glm_r5_profit_risk_scaled_deals.py`
- Test: `apps/backtest-engine/src/martingale/kline_engine.rs`
- Output: `docs/superpowers/artifacts/glm-martingale-core-round5/r5-profit-risk-scaled-deals.json`

New risk fields:

```rust
#[serde(default)]
pub realized_profit_reinvest_pct: Option<f64>;
#[serde(default)]
pub realized_profit_reinvest_cap_quote: Option<Decimal>;
#[serde(default)]
pub loss_streak_risk_reduction_pct: Option<f64>;
#[serde(default)]
pub loss_streak_trigger_count: Option<u32>;
#[serde(default)]
pub loss_streak_recovery_win_count: Option<u32>;
#[serde(default)]
pub dynamic_max_legs_floor: Option<u32>;
```

Rules:

1. Use only closed cycles before the new cycle timestamp.
2. Reinvest amount cannot exceed realized profit reserve.
3. Risk reduction reduces future `first_order_quote` and may reduce `max_legs`, never increases leverage.
4. Global budget remains `<5000U`; rejected budget events must be counted.
5. The script must report whether return comes from reinvestment or reduced losses.

- [ ] **Step C1: Add failing tests**

```bash
cargo test -p backtest-engine profit_reinvest_increases_next_base_order_from_closed_profit -- --nocapture
cargo test -p backtest-engine loss_streak_reduction_lowers_next_base_order -- --nocapture
cargo test -p backtest-engine loss_streak_reduction_uses_only_past_cycles -- --nocapture
```

- [ ] **Step C2: Implement backtest support**

Run:

```bash
cargo test -p backtest-engine profit_reinvest -- --nocapture
cargo test -p backtest-engine loss_streak_reduction -- --nocapture
```

- [ ] **Step C3: Search grid**

```bash
python3 scripts/glm_r5_profit_risk_scaled_deals.py \
  --base-config docs/superpowers/artifacts/glm-martingale-core-round4/promising/r4-combo-best.json \
  --out docs/superpowers/artifacts/glm-martingale-core-round5/r5-profit-risk-scaled-deals.json \
  --workers 16
```

Grid:

| Field | Values |
|---|---|
| reinvest pct | `0`, `10`, `20`, `30`, `50` |
| reinvest cap | `50U`, `100U`, `200U`, `400U` |
| loss trigger count | `1`, `2`, `3` |
| risk reduction pct | `20`, `35`, `50`, `65` |
| recovery win count | `1`, `2`, `3` |
| max legs floor long | `4`, `5`, `6` |
| max legs floor short | `3`, `4`, `5` |
| cooldown after loss | `11h`, `18h`, `24h`, `36h` |

Promote if:

1. ann `>=45%` and DD `<=18%`, or
2. 2025 `>=0` and full ann `>=30%`, or
3. Conservative original target passes.

- [ ] **Step C4: Commit**

```bash
git add crates/shared-domain apps/backtest-engine scripts/glm_r5_profit_risk_scaled_deals.py docs/superpowers/artifacts/glm-martingale-core-round5 docs/superpowers/reports/2026-07-03-glm-martingale-round5-search-ledger.md
git commit -m "feat: 修复思路 增加Round5马丁盈亏动态首单缩放"
```

## 10. Task D: Volatility-Targeted Martingale Exposure

**Goal:** Instead of only pausing high ATR states, scale first order, max legs, and spacing so each cycle targets similar risk. This differs from prior ATR/ADX scans because the DCA cycle remains active but exposure changes continuously.

**Files:**
- Modify: `crates/shared-domain/src/martingale.rs`
- Modify: `apps/backtest-engine/src/martingale/kline_engine.rs`
- Create: `scripts/glm_r5_vol_targeted_martingale.py`
- Test: `apps/backtest-engine/src/martingale/kline_engine.rs`
- Output: `docs/superpowers/artifacts/glm-martingale-core-round5/r5-vol-targeted-martingale.json`

New risk fields:

```rust
#[serde(default)]
pub vol_target_atr_pct: Option<f64>;
#[serde(default)]
pub vol_target_min_scale: Option<f64>;
#[serde(default)]
pub vol_target_max_scale: Option<f64>;
#[serde(default)]
pub vol_target_scale_max_legs: Option<bool>;
#[serde(default)]
pub vol_target_scale_spacing: Option<bool>;
```

Rules:

1. Scale uses current ATR percent from already available bars.
2. Scale is clamped between min and max.
3. If `scale_max_legs` is true, high volatility lowers max legs but never below floor from config.
4. If `scale_spacing` is true, high volatility widens spacing and low volatility tightens spacing inside configured bounds.

- [ ] **Step D1: Add failing tests**

```bash
cargo test -p backtest-engine vol_target_lowers_first_order_when_atr_exceeds_target -- --nocapture
cargo test -p backtest-engine vol_target_widens_spacing_when_enabled -- --nocapture
```

- [ ] **Step D2: Implement and run tests**

```bash
cargo test -p backtest-engine vol_target -- --nocapture
```

- [ ] **Step D3: Search grid**

```bash
python3 scripts/glm_r5_vol_targeted_martingale.py \
  --base-config docs/superpowers/artifacts/glm-martingale-core-round4/promising/r4-combo-best.json \
  --out docs/superpowers/artifacts/glm-martingale-core-round5/r5-vol-targeted-martingale.json \
  --workers 16
```

Grid:

| Field | Values |
|---|---|
| ATR period | `14`, `24`, `48` |
| target ATR pct | `1.0`, `1.5`, `2.0`, `2.5`, `3.0` |
| min scale | `0.35`, `0.50`, `0.70` |
| max scale | `1.0`, `1.25`, `1.50` |
| scale max legs | `false`, `true` |
| scale spacing | `false`, `true` |
| high-vol hard pause | disabled, `ATR% > 5`, `ATR% > 7` |

Promote if DD improves by `>=4pp` while ann stays `>=30%`, or if any near-frontier gate passes.

- [ ] **Step D4: Commit**

```bash
git add crates/shared-domain apps/backtest-engine scripts/glm_r5_vol_targeted_martingale.py docs/superpowers/artifacts/glm-martingale-core-round5 docs/superpowers/reports/2026-07-03-glm-martingale-round5-search-ledger.md
git commit -m "feat: 修复思路 增加Round5波动率目标马丁仓位"
```

## 11. Task E: Custom Price Ladder With Dip And Breakout Legs

**Goal:** Test a tightly bounded custom ladder inspired by DCA custom price ladder: a cycle can average down on adverse movement and optionally add one small continuation leg after favorable confirmation. This is a martingale probe only if the DCA recovery legs remain the primary mechanism.

**Files:**
- Create: `scripts/glm_r5_custom_ladder_dip_breakout.py`
- Output: `docs/superpowers/artifacts/glm-martingale-core-round5/r5-custom-ladder-dip-breakout.json`

Implementation approach:

1. Use existing `MartingaleSpacingModel::CustomSequence` and `MartingaleSizingModel::CustomSequence` when possible.
2. If favorable continuation leg needs engine support, add it under a clearly named field `favorable_add_sequence`.
3. Cap favorable continuation notional to `<=25%` of planned adverse safety-order notional.
4. Mark candidate rejected if more than `40%` of realized PnL comes from favorable continuation adds.

Search grid:

| Field | Values |
|---|---|
| adverse steps long | `[120,240,420,700,1050]`, `[150,300,600,900,1300]`, `[200,450,800,1250,1800]` |
| adverse steps short | `[150,350,650,1000]`, `[220,500,900,1400]`, `[300,700,1200,1900]` |
| notionals long | `[25,45,80,140,220]`, `[30,55,95,160,260]`, `[35,70,135,240,420]` |
| notionals short | `[20,32,52,85]`, `[25,42,70,115]`, `[30,50,85,140]` |
| favorable add | disabled, `+100bps`, `+150bps`, `+250bps` |
| favorable add condition | `ema50 slope positive`, `roc(120)>2`, `rsi 50-65` |
| TP ladder | `600/1200/2200`, `800/1600/2600`, trailing after TP1 |

- [ ] **Step E1: Run no-engine custom adverse ladder first**

```bash
python3 scripts/glm_r5_custom_ladder_dip_breakout.py \
  --mode adverse-only \
  --base-config docs/superpowers/artifacts/glm-martingale-core-round4/promising/r4-combo-best.json \
  --out docs/superpowers/artifacts/glm-martingale-core-round5/r5-custom-ladder-adverse-only.json \
  --workers 16
```

- [ ] **Step E2: Run bounded favorable-add probe**

```bash
python3 scripts/glm_r5_custom_ladder_dip_breakout.py \
  --mode bounded-favorable-add \
  --base-config docs/superpowers/artifacts/glm-martingale-core-round4/promising/r4-combo-best.json \
  --out docs/superpowers/artifacts/glm-martingale-core-round5/r5-custom-ladder-dip-breakout.json \
  --workers 16
```

Promote only if:

1. `martingale_core_pnl_share >= 60%`, and
2. any near-frontier gate passes, and
3. 2025 does not worsen versus `-8.70%`.

- [ ] **Step E3: Commit**

```bash
git add scripts/glm_r5_custom_ladder_dip_breakout.py docs/superpowers/artifacts/glm-martingale-core-round5 docs/superpowers/reports/2026-07-03-glm-martingale-round5-search-ledger.md
git commit -m "docs: 修复思路 记录Round5自定义价格梯马丁搜索"
```

## 12. Task F: Same-Symbol Hedged Martingale Grid

**Goal:** Test whether the missed 2025 chop can be reduced by running long and short martingale grids on the same high-liquidity symbols under hedge-mode semantics. This is not pair spread trading: every leg remains a `MartingaleStrategyConfig` order on its own symbol/direction.

**Files:**
- Create: `scripts/glm_r5_same_symbol_hedged_grid.py`
- Output: `docs/superpowers/artifacts/glm-martingale-core-round5/r5-same-symbol-hedged-grid.json`
- Report: `docs/superpowers/reports/2026-07-03-glm-round5-same-symbol-hedged-grid.md`

Candidate symbols:

```text
BTCUSDT,ETHUSDT,BNBUSDT,SOLUSDT,LINKUSDT,ADAUSDT,DOGEUSDT,TRXUSDT,BCHUSDT,AAVEUSDT
```

Rules:

1. Same symbol may have one long martingale strategy and one short martingale strategy.
2. Hedge mode is required for live promotion.
3. Per-symbol total planned margin must fit exact Binance filters and `<5000U` total budget.
4. Long and short cannot both open new cycles in the same hour unless both signals are range-reversion extremes.
5. Net notional cap rejects portfolios where same-symbol long+short planned exposure exceeds `35%` of total budget.

Search grid:

| Field | Values |
|---|---|
| symbol group size | `3`, `4`, `5` |
| long entry | `rsi<35`, `bb_lower`, `ema50 up + rsi<45` |
| short entry | `rsi>65`, `bb_upper`, `roc(720)>12+rsi>65` |
| long/short first order | `20/20`, `25/20`, `30/25` |
| multipliers | `1.4`, `1.6`, `1.8`, `2.2` |
| max legs | `3`, `4`, `5` |
| cooldown | `8h`, `11h`, `16h`, `24h` |

- [ ] **Step F1: Run same-symbol hedge-grid search**

```bash
python3 scripts/glm_r5_same_symbol_hedged_grid.py \
  --market-data data/market_data_full.db \
  --funding-data data/funding_rates.db \
  --out docs/superpowers/artifacts/glm-martingale-core-round5/r5-same-symbol-hedged-grid.json \
  --report docs/superpowers/reports/2026-07-03-glm-round5-same-symbol-hedged-grid.md \
  --workers 20
```

Promote if:

1. standalone same-symbol hedge-grid achieves ann `>=35%`, DD `<=15%`, pos `>=4/5`, or
2. combined with `r4-combo-best` passes `NF-2025-flip` or `NF-balanced-near`.

- [ ] **Step F2: Commit**

```bash
git add scripts/glm_r5_same_symbol_hedged_grid.py docs/superpowers/artifacts/glm-martingale-core-round5 docs/superpowers/reports/2026-07-03-glm-round5-same-symbol-hedged-grid.md docs/superpowers/reports/2026-07-03-glm-martingale-round5-search-ledger.md
git commit -m "docs: 修复思路 记录Round5同币双向马丁网格搜索"
```

## 13. Task G: Expanded Universe With Anti-Survivor Rules

**Goal:** Rounds 2-4 stayed near base6. Round 5 may expand symbols, but selection must be based on coverage/liquidity and then validated out-of-sample to avoid picking hindsight winners.

**Files:**
- Create: `scripts/glm_r5_symbol_universe_expansion.py`
- Output: `docs/superpowers/artifacts/glm-martingale-core-round5/r5-symbol-universe-expansion.json`
- Report: `docs/superpowers/reports/2026-07-03-glm-round5-symbol-universe-expansion.md`

Symbol prefilter:

```sql
SELECT symbol,
       COUNT(*) AS rows,
       MIN(open_time) AS min_ts,
       MAX(open_time) AS max_ts,
       AVG(volume) AS avg_volume
FROM klines
WHERE market_type = 'usd_m_futures'
  AND timeframe = '1h'
GROUP BY symbol
HAVING min_ts <= 1672531200000
   AND max_ts >= 1780271999999
   AND rows >= 25000
ORDER BY avg_volume DESC;
```

Validation rules:

1. Preselect the top liquid/complete symbols before reading candidate PnL.
2. Split candidate discovery into `2023-2024` and validation into `2025-2026_ytd`.
3. Any selected symbol must have at least two positive validation segments or improve portfolio DD.
4. If a symbol is only profitable in h1_2023, it is rejected even if full ann improves.

- [ ] **Step G1: Build universe report**

```bash
python3 scripts/glm_r5_symbol_universe_expansion.py \
  --market-data data/market_data_full.db \
  --base-config docs/superpowers/artifacts/glm-martingale-core-round4/promising/r4-combo-best.json \
  --out docs/superpowers/artifacts/glm-martingale-core-round5/r5-symbol-universe-expansion.json \
  --report docs/superpowers/reports/2026-07-03-glm-round5-symbol-universe-expansion.md \
  --workers 20
```

Search only these structures:

| Structure | Meaning |
|---|---|
| `long-bull-replacement` | Replace one of BNB/TRX/BCH with a prefiltered long symbol. |
| `pump-fade-short-replacement` | Replace one of AAVE/SOL/DOT with a prefiltered short symbol. |
| `two-new-low-weight` | Add two new symbols at `5%` weight each and reduce core weights proportionally. |
| `symbol-quarantine` | Remove a symbol after two consecutive closed-cycle loss clusters using only past data. |

Promote if expanded universe passes any near-frontier gate and has no segment with DD above `25%`.

- [ ] **Step G2: Commit**

```bash
git add scripts/glm_r5_symbol_universe_expansion.py docs/superpowers/artifacts/glm-martingale-core-round5 docs/superpowers/reports/2026-07-03-glm-round5-symbol-universe-expansion.md docs/superpowers/reports/2026-07-03-glm-martingale-round5-search-ledger.md
git commit -m "docs: 修复思路 记录Round5马丁扩展币种反过拟合搜索"
```

## 14. Task H: Live-Parity Hardening Before Any Final Candidate

**Goal:** Round 4 live parity still notes conservative approximations around partial TP. Round 5 must not promote a candidate as live-ready until Binance USD-M filters and algo-order behavior are modeled.

**Files:**
- Modify: `apps/trading-engine/src/main.rs`
- Modify: `apps/trading-engine/src/martingale_runtime.rs`
- Modify: `apps/trading-engine/src/exchange.rs` or the local exchange adapter path used by this repo
- Create: `docs/superpowers/reports/2026-07-03-glm-round5-live-parity-hardening.md`
- Test: `apps/trading-engine/tests/martingale_runtime.rs`
- Test: `apps/trading-engine/tests/execution_sync.rs`

Required checks:

1. Partial TP must submit reduce-only quantity for the configured fraction, not full close, unless the report explicitly marks the candidate as conservative non-parity.
2. Breakeven and stop/take-profit orders must account for `/fapi/v1/algoOrder` behavior.
3. Open normal orders and algo orders must fit `MAX_NUM_ORDERS` and `MAX_NUM_ALGO_ORDERS`.
4. Notional, tick size, step size, and min quantity must come from `exchangeInfo` snapshots or committed fixtures.
5. On final TP or SL, stale opposite conditional exits must be canceled or proven impossible.

- [ ] **Step H1: Add parity tests**

```bash
cargo test -p trading-engine martingale_partial_tp_places_fractional_reduce_only_close -- --nocapture
cargo test -p trading-engine martingale_algo_order_count_fits_exchange_filters -- --nocapture
cargo test -p trading-engine martingale_final_exit_cancels_stale_algo_orders -- --nocapture
```

- [ ] **Step H2: Implement or document exact blocker**

If implementation is completed, run:

```bash
cargo test -p trading-engine martingale -- --nocapture
cargo test -p trading-engine execution_sync -- --nocapture
```

If blocked, write the blocker to:

```text
docs/superpowers/reports/2026-07-03-glm-round5-live-parity-hardening.md
```

The blocker report must include the candidate affected, exact exchange rule, current code path, and required patch path.

- [ ] **Step H3: Commit**

```bash
git add apps/trading-engine docs/superpowers/reports/2026-07-03-glm-round5-live-parity-hardening.md docs/superpowers/reports/2026-07-03-glm-martingale-round5-search-ledger.md
git commit -m "feat: 修复思路 强化Round5马丁实盘等价校验"
```

## 15. Task I: Dynamic Martingale Allocator Only After Competitive Sleeves Exist

**Goal:** Revisit allocator only if Tasks B-G produce at least two competitive martingale configs. Static allocator failed in Round 3/4; Round 5 allocator must be lagged and may allocate to cash.

**Files:**
- Create: `scripts/glm_r5_dynamic_martingale_allocator.py`
- Output: `docs/superpowers/artifacts/glm-martingale-core-round5/r5-dynamic-martingale-allocator.json`

Candidate library:

1. `r4-combo-best`.
2. Best from Task B if promoted.
3. Best from Task C if promoted.
4. Best from Task D if promoted.
5. Best from Task F or G if promoted.
6. Cash.

Allocator rules:

1. Uses only prior closed-cycle returns and prior drawdowns.
2. Rebalances every `7d`, `14d`, or `30d`.
3. Max allocation to a non-core sleeve is `20%`.
4. If no config has positive lagged score, allocate to cash.
5. Switch log records timestamp, selected config, lagged score, and allocation.

- [ ] **Step I1: Check eligible configs**

Run only if at least two new configs meet `NF-frontier-improve`, `NF-2025-flip`, or `NF-conservative-near`.

```bash
python3 scripts/glm_r5_dynamic_martingale_allocator.py \
  --library docs/superpowers/artifacts/glm-martingale-core-round5/promising \
  --base-config docs/superpowers/artifacts/glm-martingale-core-round4/promising/r4-combo-best.json \
  --out docs/superpowers/artifacts/glm-martingale-core-round5/r5-dynamic-martingale-allocator.json \
  --workers 12
```

Promote if:

1. ann `>=50%`, DD `<=18%`, pos `>=4/5`, or
2. any original target passes.

- [ ] **Step I2: Commit**

```bash
git add scripts/glm_r5_dynamic_martingale_allocator.py docs/superpowers/artifacts/glm-martingale-core-round5 docs/superpowers/reports/2026-07-03-glm-martingale-round5-search-ledger.md
git commit -m "docs: 修复思路 记录Round5动态马丁组合分配"
```

## 16. Final Candidate Replay Commands

Every promising candidate must run full period:

```bash
target/release/portfolio_budget_replay \
  --config docs/superpowers/artifacts/glm-martingale-core-round5/promising/<candidate-config>.json \
  --budget 5000 \
  --start-ms 1672531200000 \
  --end-ms 1780271999999 \
  --market-data data/market_data_full.db \
  --funding-data data/funding_rates.db \
  --profile aggressive \
  --portfolio-id r5-final-validation \
  --exchange-min-notional 5
```

And each segment:

```bash
target/release/portfolio_budget_replay --config docs/superpowers/artifacts/glm-martingale-core-round5/promising/<candidate-config>.json --budget 5000 --start-ms 1672531200000 --end-ms 1688169599999 --market-data data/market_data_full.db --funding-data data/funding_rates.db --profile aggressive --portfolio-id r5-h1-2023 --exchange-min-notional 5
target/release/portfolio_budget_replay --config docs/superpowers/artifacts/glm-martingale-core-round5/promising/<candidate-config>.json --budget 5000 --start-ms 1688169600000 --end-ms 1704067199999 --market-data data/market_data_full.db --funding-data data/funding_rates.db --profile aggressive --portfolio-id r5-h2-2023 --exchange-min-notional 5
target/release/portfolio_budget_replay --config docs/superpowers/artifacts/glm-martingale-core-round5/promising/<candidate-config>.json --budget 5000 --start-ms 1704067200000 --end-ms 1735689599999 --market-data data/market_data_full.db --funding-data data/funding_rates.db --profile aggressive --portfolio-id r5-2024 --exchange-min-notional 5
target/release/portfolio_budget_replay --config docs/superpowers/artifacts/glm-martingale-core-round5/promising/<candidate-config>.json --budget 5000 --start-ms 1735689600000 --end-ms 1767225599999 --market-data data/market_data_full.db --funding-data data/funding_rates.db --profile aggressive --portfolio-id r5-2025 --exchange-min-notional 5
target/release/portfolio_budget_replay --config docs/superpowers/artifacts/glm-martingale-core-round5/promising/<candidate-config>.json --budget 5000 --start-ms 1767225600000 --end-ms 1780271999999 --market-data data/market_data_full.db --funding-data data/funding_rates.db --profile aggressive --portfolio-id r5-2026-ytd --exchange-min-notional 5
```

Candidate summary JSON must contain measured numbers:

```json
{
  "candidate_id": "r5-B-last-executed-so-best",
  "family": "last_executed_safety_order_basis",
  "martingale_core": true,
  "research_only": false,
  "live_parity_status": "passed",
  "budget_quote": 5000,
  "full": {
    "annualized_return_pct": 42.1,
    "max_drawdown_pct": 16.8,
    "total_return_pct": 180.0,
    "positive_segments": 4,
    "principal_breached": false
  },
  "segments": {
    "h1_2023": {"return_pct": 20.0, "annualized_return_pct": 45.0, "max_drawdown_pct": 12.0},
    "h2_2023": {"return_pct": 4.0, "annualized_return_pct": 8.0, "max_drawdown_pct": 10.0},
    "2024": {"return_pct": 28.0, "annualized_return_pct": 28.0, "max_drawdown_pct": 16.0},
    "2025": {"return_pct": -2.0, "annualized_return_pct": -2.0, "max_drawdown_pct": 18.0},
    "2026_ytd": {"return_pct": 8.0, "annualized_return_pct": 20.0, "max_drawdown_pct": 9.0}
  },
  "acceptance": {
    "conservative": false,
    "balanced": false,
    "aggressive": false,
    "near_frontier": ["NF-frontier-improve"],
    "first_failed_gate": "annualized return below 50 for conservative"
  }
}
```

## 17. Handoff Back To ChatGPT

When GLM finishes or pauses, write:

```text
docs/superpowers/reports/2026-07-03-glm-round5-handoff-to-chatgpt.md
```

Handoff sections:

1. Branch and latest commit.
2. Registry line count and ledger path.
3. Data coverage and sha256 paths.
4. Round 4 baseline reproduction result.
5. Task A cycle micro-regime findings.
6. Task B-G result tables with candidate count, wall time, best full result, five segments, decision, non-repeat key.
7. Task H live-parity status.
8. Any target-passing candidate.
9. Near-frontier shortlist with exact config paths.
10. Rejected-family table that future agents must not repeat.
11. Open blockers and whether they require user approval.

Final commit and push:

```bash
git add docs/superpowers/reports/2026-07-03-glm-round5-handoff-to-chatgpt.md docs/superpowers/artifacts/glm-martingale-core-round5 docs/superpowers/reports/2026-07-03-glm-martingale-round5-search-ledger.md
git commit -m "docs: 修复思路 Round5马丁搜索交接"
git push -u origin glm-martingale-core-round5
```

## 18. Operating Rule For Long Searches

GLM 可以接受长时间回测，但每个长跑必须先完成：

1. 2025-first smoke with at least `50` candidates.
2. One full-period replay of the current best structure to prove runner output is comparable to `r4-combo-best`.
3. Registry `running` line.
4. Manifest with command, git commit, data sha256, candidate grid size, worker count.
5. Segment-first pruning before expanding to the full grid.

Round 5 不允许因为目标困难而整体停止。只能按本计划的 stop rules 关闭单个方向；所有关闭都必须留下 evidence、decision 和 non-repeat key。
