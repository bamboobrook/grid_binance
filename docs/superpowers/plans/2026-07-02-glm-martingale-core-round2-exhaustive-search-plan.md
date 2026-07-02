# GLM Martingale Core Round 2 Exhaustive Search Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 在 `docs/superpowers/reports/2026-07-02-glm-handoff-to-chatgpt.md` 的基础上，继续以马丁/DCA/grid 为收益核心，系统探索尚未充分验证的外部启发机制，并把每一次探索完整记录，避免重复回测。

**Architecture:** 不改成纯趋势、纯突破、纯资金费率或纯配对套利。所有新增机制只能影响马丁 cycle 的启动、安全单、止盈、止损、重入、网格中心、币种启停或组合权重；最终候选必须通过 `<5000U`、多币种、分段稳健、回撤门禁和 live-parity 复验。

**Tech Stack:** Rust `portfolio_budget_replay` / `backtest-engine` / `trading-engine`, Python research runners in `scripts/`, SQLite `data/market_data_full.db` and `data/funding_rates.db`, optional derived SQLite for futures sentiment data, Git branch/worktree evidence workflow.

---

## 0. User Constraint Restatement

达拉崩吧的约束是硬约束：

1. 策略必须继续“用马丁跑”，指标只辅助判断多空、是否开 cycle、是否加安全单、如何止盈止损、如何调权。
2. 不接受用纯趋势、纯突破、纯 funding、纯 pair-neutral 替代马丁核心。
3. 可以接受长时间回测，但不接受没记录的重复探索。
4. 三档收益目标仍是：
   - Conservative: annualized `>50%`, max DD `<=10%`, positive segments `>=4/5`
   - Balanced: annualized `>90%`, max DD `<=20%`, positive segments `>=4/5`
   - Aggressive: annualized `>110%`, max DD `<=30%`, positive segments `>=3/5`, preferably `>=4/5`
5. 最终仍要小资金 `<5000U`、多币种组合、回撤控制、防止过拟合。

Round 2 的目标不是立刻判死刑，而是把尚未充分工程化的马丁相邻机制继续穷尽。

## 1. External Research Signals Used

本轮外部检索没有发现可直接复制的公开达标组合，但发现了几个仍可转化为“马丁核心增强”的方向：

1. Dynamic Grid Trading paper: DGT 通过动态重置 grid 位置改善传统 grid 的零期望问题，并在 BTC/ETH 分钟级回测中优于传统 grid 和 buy-and-hold。来源: `https://arxiv.org/abs/2506.11921`
2. 3Commas condition-based averaging orders: DCA 安全单可由 RSI/MACD/MA 等指标触发，并且仍要求满足最小价格偏离。来源: `https://help.3commas.io/en/articles/9663694-dca-bot-averaging-orders-by-technical-indicators`
3. 3Commas stop-loss breakeven: 多级 TP 后把 SL 移到平均入场价或上一 TP，且 TP1 后可取消未成交 averaging orders。来源: `https://help.3commas.io/en/articles/9464682-dca-bot-stop-loss-breakeven`
4. 3Commas trailing buy: 价格先触发阈值，再等待从低点反弹一定比例才买入，可转化为马丁安全单的 rebound confirmation。来源: `https://help.3commas.io/en/articles/3108937-how-trailing-buy-works`
5. Binance USD-M Futures sentiment/flow endpoints are live-obtainable for future execution:
   - Open interest statistics: `GET /futures/data/openInterestHist`
   - Global long/short account ratio: `GET /futures/data/globalLongShortAccountRatio`
   - Taker buy/sell volume ratio: `GET /futures/data/takerlongshortRatio`
   - Funding rate history: `GET /fapi/v1/fundingRate`
   Sources:
   `https://developers.binance.com/docs/derivatives/usds-margined-futures/market-data/rest-api/Open-Interest-Statistics`,
   `https://developers.binance.com/docs/derivatives/usds-margined-futures/market-data/rest-api/Long-Short-Ratio`,
   `https://developers.binance.com/docs/derivatives/usds-margined-futures/market-data/rest-api/Taker-BuySell-Volume`,
   `https://developers.binance.com/docs/derivatives/usds-margined-futures/market-data/rest-api/Get-Funding-Rate-History`
6. Binance USD-M conditional TP/SL/trailing orders now use Algo Order semantics, so any partial TP / breakeven / trailing-stop live promotion must respect the current endpoint behavior. Source: `https://developers.binance.com/docs/derivatives/usds-margined-futures/trade/rest-api/New-Algo-Order`

Interpretation for this repo: these sources do not prove the return targets are achievable, but they identify untested mechanisms that still preserve the martingale core.

## 2. Existing Frontier And Do-Not-Repeat Matrix

Before any new run, GLM must read:

```bash
sed -n '1,220p' docs/superpowers/reports/2026-07-02-glm-handoff-to-chatgpt.md
sed -n '1,260p' docs/superpowers/reports/2026-07-01-glm-martingale-core-search-ledger.md
sed -n '1,220p' docs/superpowers/reports/2026-07-01-glm-martingale-core-stop-report.md
sed -n '1,220p' docs/superpowers/reports/2026-07-01-external-martingale-grid-claim-gate-matrix.md
```

The following families are considered already exhausted unless the new run changes the mechanism, not just the numbers:

| Already explored | Do not repeat as-is | Allowed Round 2 mutation |
|---|---|---|
| Fixed spacing / multiplier spacing / ATR spacing | Wider numeric sweep of same fields | Conditional safety-order trigger or rebound confirmation before adding |
| Percent TP / ATR TP / trailing TP / mixed TP | Same single-exit TP sweep | Partial TP ladder plus breakeven stop and remaining-SO cancel |
| Tight SL / loose SL | Another `sl_bps` grid only | State-dependent SL after partial TP or regime break |
| Portfolio DD stop with calendar cooldown | More fixed cooldown values only | Equity/volatility recovery-based re-entry |
| Per-symbol EMA/ADX/RSI/BB gates | Same expression family only | Gates applied to safety orders, cycle exit, or symbol quarantine |
| Broad 6-symbol long/short portfolio | Add/remove one symbol with same rules | Dynamic active-symbol health scoring and rolling allocation |
| Research-only DGT reset | Same free DGT reset | Bounded reset only while flat or after low-exposure cycle close |
| Funding sleeve standalone | Standalone funding carry | Funding as direction/weight veto for martingale entries |
| Pair-neutral grid/probe | Pure pair-neutral as core | Only allowed if each leg remains martingale and user approves final classification |

## 3. Mandatory Exploration Registry

Round 2 must create a machine-readable registry before running any long backtest:

```bash
mkdir -p docs/superpowers/artifacts/glm-martingale-core-round2/promising
mkdir -p docs/superpowers/artifacts/glm-martingale-core-round2/rejected
mkdir -p docs/superpowers/artifacts/glm-martingale-core-round2/run-manifests
touch docs/superpowers/reports/2026-07-02-glm-martingale-round2-search-ledger.md
touch docs/superpowers/artifacts/glm-martingale-core-round2/exploration-registry.jsonl
```

Every exploration, including failures, must append one JSON line:

```json
{
  "exploration_id": "r2-A-partial-tp-be-001",
  "started_at_utc": "generated ISO-8601 UTC timestamp from the runner",
  "family": "partial_tp_breakeven_martingale",
  "hypothesis": "Partial TP banks cycle profit before full mean reversion and BE stop reduces tail DD.",
  "new_vs_prior": "Prior TP tests used single full-position exits; this tests partial exits plus stop migration.",
  "martingale_core": true,
  "research_only": true,
  "input_signature": "family=partial_tp_be|tp=0.35/0.35/0.30|be=after_tp1|cancel_so=true",
  "config_sha256": "sha256 of canonical JSON config generated before replay",
  "command": "exact command used",
  "data_sha256": {
    "market_data_full.db": "3422e929ed994829b0a66efe4f8473eec1d43a2ce991ce654648cadeacd2ff19",
    "funding_rates.db": "356e270dacce36b4703364545beeafe402f1aa55aa11caebc5708f5f3b6767a1"
  },
  "status": "running",
  "result_path": "docs/superpowers/artifacts/glm-martingale-core-round2/rejected/r2-A-partial-tp-be-001.json",
  "decision": "pending"
}
```

After completion, update the ledger with:

```markdown
## r2-A-partial-tp-be-001

- Hypothesis:
- New vs prior:
- Command:
- Candidate count:
- Wall time:
- Best full-period result:
- Segment table:
- Gate result:
- Reject/promote reason:
- Next mutation:
```

If a run has no ledger entry and registry line, the result must not be used as evidence.

## 4. Worktree And Branch Protocol

Use a separate branch/worktree:

```bash
git status --short --branch
git worktree add .worktrees/glm-martingale-core-round2 -b glm-martingale-core-round2
cd .worktrees/glm-martingale-core-round2
git status --short --branch
```

Expected:

```text
## glm-martingale-core-round2
```

Before long runs:

```bash
ps -eo pid,etime,cmd | rg 'portfolio_budget_replay|search_small_capital_martingale|glm_.*search|dgt_dynamic_grid_probe' || true
sha256sum data/market_data_full.db data/funding_rates.db
```

Record any live process in the ledger. Do not kill unknown work without identifying owner and command.

## 5. Round 2 Priority Directions

### Direction A: Partial TP Ladder + Breakeven Stop

Hypothesis: Current ann/DD cliff exists because each cycle needs to wait for full mean reversion. A partial TP ladder can bank profit earlier, reduce remaining exposure, move the stop to average entry after TP1, and cancel unused safety orders. This keeps martingale as the entry/averaging core but changes active-cycle exit.

Why it is new: prior `percent`, `atr`, `trailing`, and `mixed` TP tests exited the whole cycle as one position. They did not test partial exits plus stop migration.

Research prototype semantics:

1. Cycle opens and averages using existing martingale rules.
2. TP1 closes `30-50%` of current position at `400-900 bps` from weighted average entry.
3. After TP1, stop moves to weighted average entry plus fees, or to `TP1 - buffer`.
4. Remaining safety orders are canceled after TP1 for conservative/balanced variants.
5. TP2 closes `25-40%` at `900-1600 bps`.
6. TP3 closes remaining size with trailing or fixed `1600-2600 bps`.

Search grid:

| Field | Values |
|---|---|
| TP split | `50/30/20`, `40/35/25`, `30/30/40` |
| TP ladder bps | `400/900/1600`, `600/1200/2200`, `800/1600/2600` |
| BE trigger | `after_tp1`, `after_tp2` |
| BE buffer bps | `0`, `25`, `50`, `100` |
| Cancel unused safety orders after TP1 | `true`, `false` |
| Apply to | long only, short only, both |

Promotion gate:

1. Research prototype must find at least one candidate with `ann > 35%`, `DD <= 30%`, `positive_segments >= 3`, and `2024+2025+2026_ytd > 0`.
2. If the prototype clears that near-gate, implement backtest-engine support with tests.
3. Live promotion requires reduce-only correctness under Hedge Mode and current Binance Algo Order behavior.

### Direction B: Conditional Safety Orders + Rebound Confirmation

Hypothesis: Martingale loses too much when it adds safety orders during a one-way move. Safety orders should still be martingale orders, but execution must require both adverse price deviation and a recovery/indicator signal.

Why it is new: existing entry triggers mostly gate cycle starts, while ADX safety-skip only blocks extreme trend. This tests 3Commas-style condition-based averaging orders and trailing-buy style rebound confirmation at the safety-order level.

Mechanisms:

1. `so_condition_mode=indicator_and_deviation`: safety order triggers only when price deviation is reached and indicator expression is true.
2. `so_condition_mode=rebound`: after safety level is touched, wait for rebound from local low/high before adding.
3. `so_condition_mode=one_per_signal`: if price gaps through multiple levels, execute only one safety order per signal, not all eligible orders.
4. `so_condition_mode=batch_from_base`: comparison arm that executes all eligible levels after signal.

Search grid:

| Field | Values |
|---|---|
| Min deviation bps | `100`, `150`, `220`, `300` |
| Rebound confirmation bps | `20`, `40`, `60`, `100`, `150` |
| Indicator set | `rsi_reversal`, `bb_reentry`, `ema_slope_recovery`, `taker_flow_recovery` |
| Max safety orders per signal | `1`, `2`, `all` |
| Safety cooldown | `15m`, `1h`, `4h`, `6h` |

Implementation order:

1. Build research-only simulator around existing replay output if faster.
2. If promising, add explicit backtest-engine safety-order condition support.
3. Add trading-engine parity only after near-gate evidence.

Promotion gate:

`ann > 50%` with `DD <= 35%` in research is enough to justify engine work because this direction directly attacks DD, not just return.

### Direction C: Bounded DGT Reset Inside Martingale Rules

Hypothesis: DGT reset can adapt grid center, but prior DGT probes failed when unconstrained by budget/DD/segments. A bounded variant may help if reset is allowed only when the martingale cycle is flat, lightly exposed, or recently profitable.

Allowed reset modes:

1. `flat_only_reset`: reset grid center only after cycle close.
2. `low_exposure_reset`: reset only when active leg count `<=2` and unrealized loss is better than `-2%` of budget.
3. `profit_locked_reset`: after TP1/partial TP, reset remaining grid center around current price.
4. `volatility_band_reset`: reset when price moves outside ATR band while no active cycle exists.

Forbidden reset modes:

1. Resetting center while deep underwater and hiding unrealized loss.
2. Increasing allocated capital after reset without budget accounting.
3. Treating DGT standalone as final candidate if martingale cycle logic is absent.

Search grid:

| Field | Values |
|---|---|
| Reset interval guard | `flat_only`, `low_exposure`, `after_partial_tp` |
| ATR band | `1.5`, `2.0`, `2.5`, `3.0` |
| Center source | `ema20`, `ema50`, `last_cycle_close`, `donchian_mid20` |
| Reset cooldown | `6h`, `12h`, `24h`, `72h` |
| Max resets per symbol per month | `4`, `8`, `16` |

Promotion gate:

Candidate must beat 008-best on at least two of these three metrics: annualized return, DD, and `2024-2026` aggregate return, while keeping `positive_segments >=3`.

### Direction D: Futures Sentiment Gate For Martingale Entries

Hypothesis: 2025/2026 failure modes may be crowding/liquidation-flow driven. OI, long/short ratio, taker buy/sell volume, and funding can help decide whether a long/short martingale cycle has enough squeeze/reversion potential.

Data caveat:

Binance public endpoints expose these live, but some endpoints document limited recent history. For 2023-2026 backtests, GLM must first verify whether local historical data exists or can be sourced legally and reproducibly. If not, do not use this family as final evidence; record it as blocked by missing historical data.

Derived features:

| Feature | Long martingale use | Short martingale use |
|---|---|---|
| OI rising + price down | avoid catching knife unless rebound confirms | bearish continuation allowed |
| OI falling + price down | capitulation/reversion long allowed | reduce short aggressiveness |
| Global long/short extreme high | reduce long weight | allow short or pause if squeeze risk high |
| Taker sell dominance exhaustion | allow long after rebound | avoid late short |
| Funding very positive | reduce long or favor short | allow short if not in squeeze |
| Funding very negative | allow long if regime recovers | reduce short |

Search grid:

| Field | Values |
|---|---|
| OI lookback | `4h`, `12h`, `24h`, `72h` |
| OI z-score threshold | `1.0`, `1.5`, `2.0` |
| long/short ratio z-score | `1.0`, `1.5`, `2.5` |
| taker imbalance window | `1h`, `4h`, `12h` |
| funding threshold | `0.01%`, `0.03%`, `0.05%` per funding event |

Promotion gate:

Only promote if the historical data source is documented, checksummed, and replayable. A candidate based on future-only or recent-only data is not valid for final gates.

### Direction E: Inventory-Skewed Martingale Portfolio

Hypothesis: Instead of equal-weight independent bots, use inventory-risk logic: when portfolio is already net long, new long cycle size/spacing is reduced and short cycle allowance increases; when net short, invert the rule. The trade logic remains martingale, but the portfolio avoids one-sided exposure accumulation.

Mechanisms:

1. `net_delta_budget_pct`: approximate portfolio long minus short exposure over budget.
2. `inventory_skew_weight`: reduce new same-side first order as net exposure grows.
3. `inventory_skew_spacing`: widen same-side spacing as exposure grows.
4. `opposite_side_unlock`: permit hedging-side cycles when net exposure exceeds threshold.

Search grid:

| Field | Values |
|---|---|
| Net exposure threshold | `15%`, `25%`, `35%`, `50%` of budget |
| Same-side first-order multiplier | `0.25`, `0.50`, `0.75` |
| Same-side spacing multiplier | `1.25`, `1.50`, `2.00` |
| Opposite-side unlock weight | `5%`, `8%`, `12%` |
| Max simultaneous same-side cycles | `2`, `3`, `4`, `6` |

Promotion gate:

Must reduce 008-best DD below `24%` without dropping annualized return below `18%`, or raise annualized return above `35%` while keeping DD `<=30%`.

### Direction F: Recovery-Based Re-Entry After Portfolio Stop

Hypothesis: Calendar cooldown DD stop cuts DD but also misses recoveries. Re-entry should require equity, volatility, and regime recovery rather than fixed hours.

Mechanisms:

1. `portfolio_stop_reentry=equity_reclaim`: re-enable when equity recovers `25-75%` of stopped drawdown.
2. `portfolio_stop_reentry=vol_normalized`: re-enable when ATR pct falls below threshold.
3. `portfolio_stop_reentry=regime_confirmed`: re-enable only when per-symbol regime aligns for the affected side.
4. `staged_reentry`: resume with `25%`, then `50%`, then `100%` weights after successful closed cycles.

Search grid:

| Field | Values |
|---|---|
| DD stop | `10%`, `12%`, `16%`, `20%`, `25%`, `30%` |
| Equity reclaim fraction | `0.25`, `0.50`, `0.75` |
| ATR normalize threshold | `1.5%`, `2.0%`, `2.5%`, `3.5%` |
| Staged weight path | `25/50/100`, `33/66/100`, `50/100` |
| Max re-entry attempts per month | `1`, `2`, `4` |

Promotion gate:

Must beat fixed-cooldown DD-stop variants on both annualized return and DD. If it only improves one metric, record and reject.

### Direction G: Symbol Health Quarantine And Dynamic Active Set

Hypothesis: Multi-symbol helped DD but weak symbols drag returns in bad segments. Use rolling martingale health to quarantine symbols without curve-fitting year labels.

Health metrics:

1. Last `N` closed cycles win rate.
2. Average cycle duration.
3. Max adverse excursion per cycle.
4. Funding drag.
5. Spread/min-notional feasibility.
6. Correlation cluster exposure.

Rules:

1. A symbol with `health_score < threshold` stops new cycles for `cooldown_days`.
2. Existing cycles remain governed by active-cycle risk controls.
3. Replacement symbols come from a pre-approved universe ranked only by data available at that timestamp.
4. Never choose symbols by knowing segment outcome.

Search grid:

| Field | Values |
|---|---|
| Health window | `7d`, `14d`, `30d`, `60d` |
| Quarantine threshold | bottom `10%`, `20%`, `30%` |
| Cooldown days | `3`, `7`, `14`, `30` |
| Active symbol cap | `4`, `6`, `8`, `10`, `12` |
| Replacement mode | `same_direction_best_health`, `opposite_hedge_allowed`, `cash_if_no_candidate` |

Promotion gate:

Must improve `2025` and `2026_ytd` simultaneously without making `2024` negative. This family is specifically aimed at segment balance.

### Direction H: Time/Funding Window Gate

Hypothesis: Some martingale losses cluster around funding settlements, high-spread hours, or low-liquidity transitions. Time gates are high overfit risk, so use only coarse, exchange-stable windows.

Allowed features:

1. Hour-of-day bucket in UTC, grouped into four 6-hour blocks.
2. Funding event proximity: avoid opening new cycles `0-30m`, `0-60m`, or `0-120m` before/after funding.
3. Weekend flag.
4. Low-volume percentile from local klines.

Forbidden features:

1. Exact dates, months, or year labels.
2. Segment-specific time rules.
3. Symbol-specific handpicked holiday rules.

Promotion gate:

Only useful if it improves DD by at least `20%` relative while reducing annualized return by less than `15%` relative.

## 6. Search Order

Run in this order because each stage either attacks the ann/DD cliff directly or creates reusable infrastructure:

1. Direction A: Partial TP + breakeven stop.
2. Direction B: Conditional safety orders + rebound confirmation.
3. Direction F: Recovery-based re-entry after portfolio stop.
4. Direction G: Symbol health quarantine.
5. Direction E: Inventory-skewed portfolio.
6. Direction C: Bounded DGT reset.
7. Direction D: Futures sentiment gate, only after data availability is resolved.
8. Direction H: Time/funding window gate, last because overfit risk is highest.

Do not run all directions blindly in parallel before each direction's first 5% sample is inspected. For every direction:

1. Run a `1-5%` smoke sample.
2. Record registry and ledger.
3. If smoke produces no near-miss and repeats a known failure mode, stop that branch and record rejection.
4. If smoke improves one key frontier metric, expand to full grid.

## 7. Segment-First Validation

Every promoted row must include these exact windows:

| Segment | Start ms | End ms |
|---|---:|---:|
| h1_2023 | `1672531200000` | `1688169599999` |
| h2_2023 | `1688169600000` | `1704067199999` |
| 2024 | `1704067200000` | `1735689599999` |
| 2025 | `1735689600000` | `1767225599999` |
| 2026_ytd | `1767225600000` | `1780271999999` |

Hard rejects:

1. `2024 + 2025 + 2026_ytd < 0`.
2. `positive_segments < 3` for aggressive, or `<4` for conservative/balanced.
3. Full-period DD exceeds the profile limit by more than `1.10x` after full-grid expansion.
4. Single symbol contributes more than `45%` of realized PnL.
5. Any final candidate uses research-only behavior without a live-parity implementation plan.

Near-miss labels:

| Label | Condition |
|---|---|
| `near_conservative` | `ann >35%`, `DD <=12%`, `positive_segments >=4` |
| `near_balanced` | `ann >60%`, `DD <=24%`, `positive_segments >=4` |
| `near_aggressive` | `ann >70%`, `DD <=35%`, `positive_segments >=3`, `2024-2026 >0` |
| `frontier_improvement` | Beats 008-best on at least two of ann, DD, positive segments, 2024-2026 aggregate |

Near-misses must be saved under:

```text
docs/superpowers/artifacts/glm-martingale-core-round2/promising/
```

Rejected runs must be summarized under:

```text
docs/superpowers/artifacts/glm-martingale-core-round2/rejected/
```

## 8. Candidate Record Contract

Each candidate JSON must include:

```json
{
  "candidate_id": "r2-A-partial-tp-be-001-best",
  "family": "partial_tp_breakeven_martingale",
  "martingale_core": true,
  "research_only": true,
  "live_parity_status": "research_only_requires_engine_support",
  "budget_quote": 5000,
  "profile": "aggressive",
  "portfolio_symbols": ["BNBUSDT", "TRXUSDT", "BCHUSDT", "AAVEUSDT", "SOLUSDT", "DOTUSDT"],
  "full": {
    "annualized_return_pct": 0.0,
    "max_drawdown_pct": 0.0,
    "total_return_pct": 0.0,
    "trade_count": 0
  },
  "segments": {
    "h1_2023": {"return_pct": 0.0, "annualized_return_pct": 0.0, "max_drawdown_pct": 0.0},
    "h2_2023": {"return_pct": 0.0, "annualized_return_pct": 0.0, "max_drawdown_pct": 0.0},
    "2024": {"return_pct": 0.0, "annualized_return_pct": 0.0, "max_drawdown_pct": 0.0},
    "2025": {"return_pct": 0.0, "annualized_return_pct": 0.0, "max_drawdown_pct": 0.0},
    "2026_ytd": {"return_pct": 0.0, "annualized_return_pct": 0.0, "max_drawdown_pct": 0.0}
  },
  "acceptance": {
    "profile_gate": "one of passed, near_miss, frontier_improvement, rejected",
    "reason": "single sentence naming the first failed gate, such as DD 34.2 exceeds aggressive limit 30",
    "next_mutation": "single sentence naming the next planned mutation, or none"
  },
  "replay_command": "exact command used",
  "config": {}
}
```

The literal zeros above are contract examples. Completed candidate files must contain real replay values.

## 9. Engineering Promotion Rules

Research-only prototypes are allowed for exploration, but final candidates are not allowed to remain research-only.

Promotion sequence:

1. Research script proves a near-miss or frontier improvement.
2. Add backtest-engine feature with unit tests.
3. Add trading-engine feature with parity tests.
4. Re-run the exact candidate through `portfolio_budget_replay`.
5. Re-run all five segments.
6. Save config and result JSON.
7. Commit with a message containing `修复思路`.

Minimum tests for any engine feature:

| Feature | Required tests |
|---|---|
| Partial TP | fills reduce position size, PnL realized, remaining TP/SL size correct |
| Breakeven stop | stop migrates only after configured TP, includes fees/buffer |
| Cancel unused safety orders | only inactive safety orders cancel, active position remains protected |
| Conditional safety order | deviation alone does not execute; condition plus deviation executes |
| Rebound confirmation | local low/high tracking works for long and short |
| Recovery re-entry | calendar time alone does not re-enable after stop |
| Inventory skew | same-side exposure reduces size or widens spacing deterministically |

## 10. Commit And Remote Discipline

Every meaningful stage must be committed:

```bash
git add docs/superpowers/reports/2026-07-02-glm-martingale-round2-search-ledger.md \
        docs/superpowers/artifacts/glm-martingale-core-round2
git commit -m "docs: 修复思路 记录马丁Round2探索结果"
git push -u origin glm-martingale-core-round2
```

For code changes:

```bash
git add apps crates scripts docs
git commit -m "feat: 修复思路 增加马丁Round2机制"
git push
```

If a run is promising, commit immediately before starting the next long run. Promising means any of:

1. `near_conservative`, `near_balanced`, `near_aggressive`.
2. `frontier_improvement`.
3. A single segment blocker is materially improved, such as 2025 or 2026_ytd moving from negative to positive without breaking DD.

## 11. Stop And Escalation Criteria

Do not stop the whole Round 2 after one failed direction. Stop a specific branch only when:

1. It repeats a known failure mode from the do-not-repeat matrix.
2. Smoke sample shows worse ann, worse DD, and worse segment balance than 008-best.
3. Required historical data is unavailable or unverifiable.
4. The mechanism cannot be made live-parity without unsafe exchange behavior.

Escalate to ChatGPT/user when:

1. A research-only candidate clears a final return/DD/segment target and needs engine implementation.
2. A data source requires paid/API-key historical data.
3. A mechanism would change the strategy classification away from martingale core.
4. Two independent directions produce near-misses that should be combined, because combination search can be expensive.

## 12. First Concrete Execution Batch For GLM

Batch 1 must be small and evidence-focused:

1. Build exploration registry and ledger.
2. Implement or prototype Direction A partial TP + breakeven against the known 008-best and 004-highann structures.
3. Run only these base symbol sets first:
   - `BNB/TRX/BCH long + AAVE/SOL/DOT short`
   - `BNB/TRX long + AAVE short`
   - `BNB/TRX/BCH/ADA long + AAVE/SOL/DOT/NEAR short`
4. Use TP ladders:
   - `400/900/1600`, split `50/30/20`
   - `600/1200/2200`, split `40/35/25`
   - `800/1600/2600`, split `30/30/40`
5. Evaluate full period plus all five segments.
6. Save best and rejected summaries.
7. Commit before starting Direction B.

Expected output files:

```text
docs/superpowers/reports/2026-07-02-glm-martingale-round2-search-ledger.md
docs/superpowers/artifacts/glm-martingale-core-round2/exploration-registry.jsonl
docs/superpowers/artifacts/glm-martingale-core-round2/promising/r2-A-partial-tp-be-best.json
docs/superpowers/artifacts/glm-martingale-core-round2/rejected/r2-A-partial-tp-be-summary.json
```

## 13. Final Handoff Format Back To ChatGPT

When GLM pauses or finishes, write:

```text
docs/superpowers/reports/YYYY-MM-DD-glm-round2-handoff-to-chatgpt.md
```

The handoff must include:

1. Branch name and latest commit.
2. Exact list of directions run.
3. Count of candidates and wall time per direction.
4. Best result per direction, including five segment rows.
5. Best overall frontier compared to 008-best.
6. Registry path and number of JSONL lines.
7. Promising candidate paths.
8. Rejected-family table with non-repeat reason.
9. Open blockers requiring user approval.

No conclusion like “impossible” is acceptable unless every Round 2 direction above has either been run, blocked by verifiable missing data, or rejected with a specific non-repeat reason.
