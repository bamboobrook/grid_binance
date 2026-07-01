# GLM Martingale Core Indicator Expansion Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 让 GLM 继续以马丁/网格为核心，扩大指标过滤、趋势判定、主动风控和多币种配置搜索，寻找保守/平衡/激进三档小资金马丁组合。

**Architecture:** 保留 martingale/grid/DGT 类执行内核，所有新增指标只允许作为方向选择、入场许可、新周期暂停、网格/TP 自适应、已有周期退出、组合仓位分配和 regime gate。搜索必须先分段筛选，再全周期复验，避免继续选出 2023H1 单段驱动的过拟合组合。

**Tech Stack:** Rust `backtest-engine` / `portfolio_budget_replay` / `search_small_capital_martingale`, Python research runners in `scripts/`, SQLite `data/market_data_full.db` and `data/funding_rates.db`, Git branches/worktrees.

---

## 0. 非协商约束

GLM 必须按以下规则执行：

1. 核心策略必须是马丁、网格、DGT 或马丁相邻机制；不能把纯趋势、纯突破、纯资金费率、纯配对价差或纯统计套利当作主策略。
2. 指标只能服务于马丁：多空方向、入场过滤、新 cycle 暂停、grid spacing、take profit、active-cycle exit、portfolio allocation、risk cooldown。
3. 不启动 Binance 实盘、不发布到 `flyingkid`、不写应用数据库展示候选，除非三档离线门禁全部通过且用户再次批准。
4. 必须小资金可运行：最终每档保证金本金预算 `< 5000 USDT`，不是杠杆后的名义仓位。
5. 必须多币种：最终组合不能退化成单币种押注；优先 8-18 个币种，资金不足时允许降到 4-7 个，但必须说明降档原因和风险。
6. 必须控制回撤：保守 DD `<= 10%`，平衡 DD `<= 20%`，激进 DD `<= 30%`。
7. 必须抗过拟合：搜索阶段就纳入 `H1-2023`、`H2-2023`、`2024`、`2025`、`2026_ytd`，不能先按 full-period 年化排序再事后否决。
8. 必须保留 live-parity 意识：所有最终候选只能使用交易引擎可复现的指标、风控、手续费、资金费率、保证金、最小下单量、tick/step 舍入和 reduceOnly 退出逻辑。

收益目标：

| 档位 | 年化收益 | 最大回撤 | 正收益分段 | 备注 |
|---|---:|---:|---:|---|
| Conservative | `> 50%` | `<= 10%` | `>= 4/5` | 不能依赖单个极端币或单段行情 |
| Balanced | `> 90%` | `<= 20%` | `>= 4/5` | 允许更高周转，但预算仍 `<5000U` |
| Aggressive | `> 110%` | `<= 30%` | `>= 3/5`，优先 `>=4/5` | 不接受 2024-2026 合计为负 |

## 1. 当前证据摘要

已知事实：

1. `docs/superpowers/reports/2026-07-01-martingale-frontier-evidence-audit.md` 已索引大量历史报告和行级证据，机器报告最终 pass 为 `0`。
2. `docs/superpowers/reports/2026-07-01-martingale-goal-completion-audit.md` 标记原目标未完成。
3. `docs/superpowers/reports/2026-07-01-martingale-grid-search-freeze-and-reopen-criteria.md` 冻结的是重复静态马丁/网格参数扫，不是禁止新增马丁核心机制。
4. 动态 breakout/trend 组合探针已失败，且用户明确要求回到马丁核心；后续不能沿纯趋势方向替代马丁。
5. 之前 P4 / original-margin / row-combo 证据显示，失败主因不是 `<5000U` 预算缩放，而是收益/回撤/分段稳定性不足。
6. 2025 正收益源曾在 short 崩盘币和 long trend 马丁腿中出现，但旧 BTC-only regime filter 捕捉不足；后续必须使用 per-symbol regime，而不是只看 BTC。

GLM 开始前先读这些文件：

```bash
sed -n '1,220p' docs/superpowers/reports/2026-07-01-martingale-frontier-evidence-audit.md
sed -n '1,220p' docs/superpowers/reports/2026-07-01-martingale-goal-completion-audit.md
sed -n '1,220p' docs/superpowers/reports/2026-07-01-martingale-grid-search-freeze-and-reopen-criteria.md
sed -n '1,220p' docs/superpowers/plans/2026-06-30-glm-execution-handoff-small-cap-robust-martingale.md
sed -n '1,220p' docs/superpowers/reports/2026-06-29-p2-2025-segment-search-findings.md
```

## 2. 执行前工作区规程

GLM 必须在独立分支或 worktree 执行，避免污染用户当前主线。

- 建议分支：`glm-martingale-core-indicator-expansion`
- 建议 worktree：`.worktrees/glm-martingale-core-indicator-expansion`
- 证据目录：`docs/superpowers/reports/` 和 `docs/superpowers/artifacts/glm-martingale-core/`
- 长任务输出目录：`/tmp/glm_martingale_core/`

启动前确认无残留长任务：

```bash
ps -C portfolio_budget_replay --no-headers | wc -l
ps -C search_small_capital_martingale --no-headers | wc -l
ps -ef | grep -E "p4_row_combo_search.py|native_small_portfolio_search.py|original_margin_pack_v7.py|run_p4_parallel_search.py|dynamic_breakout_trend_probe.py" | grep -v grep | wc -l
```

预期：三条命令输出都为 `0`。若不是 `0`，先记录 PID、命令和归属，再决定是否停止。

创建执行台账：

```bash
mkdir -p docs/superpowers/artifacts/glm-martingale-core
printf '# GLM Martingale Core Search Ledger\n\n' > docs/superpowers/reports/2026-07-01-glm-martingale-core-search-ledger.md
git add docs/superpowers/reports/2026-07-01-glm-martingale-core-search-ledger.md
git commit -m "docs: 修复思路 初始化马丁核心搜索台账"
git push -u origin glm-martingale-core-indicator-expansion
```

## 3. 扩大搜索方向

### 方向 A：Regime-Gated Martingale Direction Search

目标：仍然跑马丁，只用 regime 判断 long/short/暂停。

核心规则：

1. long 马丁只在 per-symbol bullish 或 recovery regime 运行。
2. short 马丁只在 per-symbol bearish 或 blow-off/reversal regime 运行。
3. regime 不明确时不新开 cycle；已有 cycle 按风控退出，不盲目加仓。
4. BTC regime 只能作为组合级风险背景，不能替代每个币自己的 trend filter。

指标搜索范围：

| 维度 | 候选 |
|---|---|
| trend basis | `ema(20/50)`, `ema(30/90)`, `donchian(20/55)`, `slope(ema20, 6h/24h)` |
| momentum | `rsi(14)`, `roc(24h)`, `macd histogram` |
| trend strength | `adx(14)`, `adx(21)` |
| BTC market background | BTC `ema30/ema90`, BTC 24h ROC, BTC ATR pct |
| pause state | unclear, high-vol panic, post-stop cooldown |

搜索命名：

- `regime_per_symbol_ema`
- `regime_per_symbol_donchian`
- `regime_symbol_plus_btc_guard`
- `regime_reversal_short_only`

验收：

1. 单策略先按 segment-first 过滤。
2. 每个 symbol 至少保存最佳 long、short、long_short 各 5 个候选。
3. 组合阶段不能只选 2023H1 强 trend long；2024-2026 aggregate 必须为正。

### 方向 B：Volatility/ATR-Adaptive Martingale

目标：用 ATR 控制马丁网格密度、TP 和加仓风险，降低高波动穿仓式回撤。

核心规则：

1. ATR spacing：高波动时放宽网格，低波动时允许密集网格。
2. ATR TP：TP 随波动扩大或缩小，不用固定 bps 绑死所有行情。
3. ATR pause：ATR pct 超过阈值时暂停新 cycle。
4. ADX safety skip：极端趋势中暂停新增 safety order，避免逆势越补越深。

搜索范围：

| 维度 | Conservative | Balanced | Aggressive |
|---|---:|---:|---:|
| `atr_period` | `14,21` | `7,14,21` | `7,14,21,28` |
| `atr_spacing_multiplier` | `1.2,1.6,2.0,2.5` | `0.9,1.2,1.6,2.0` | `0.6,0.9,1.2,1.6,2.0` |
| `atr_tp_multiplier` | `0.8,1.0,1.3` | `0.8,1.1,1.5` | `1.0,1.5,2.0` |
| `atr_pause_pct` | `1.5,2.0,2.5` | `2.0,2.8,3.5` | `2.5,3.5,4.5` |
| `adx_skip_threshold` | `28,35,45` | `35,45,55` | `45,55,65` |

验收：

1. 不能只看 full-period；每个候选必须输出 5 段 metrics。
2. 若 ATR 机制回测存在但 trading-engine 不支持，候选只能标记 `research_only=true`，不能进入最终三档。
3. 若 ATR 机制已经 live-parity 支持，必须附上对应测试命令和结果。

### 方向 C：Active-Cycle Portfolio Risk Controls

目标：解决旧机制只暂停新 cycle、不能处理已有 active cycle 扩亏的问题。

允许的马丁核心风控：

1. portfolio DD stop：组合权益从峰值回撤达到阈值，reduceOnly 平掉所有 active cycles，并进入 cooldown。
2. symbol equity stop：单 symbol 对组合权益贡献达到亏损阈值，停止该 symbol 新 cycle，并平掉 active cycle。
3. cycle age stop：cycle 持续超过 `max_cycle_age_hours` 且未回归，按 reduceOnly 退出。
4. cycle profit-lock：cycle 曾浮盈后回吐超过阈值，提前止盈/止损。
5. regime break stop：当前方向 regime 失效时退出已有 cycle。

搜索范围：

| 维度 | Conservative | Balanced | Aggressive |
|---|---:|---:|---:|
| portfolio DD stop | `6,8,10%` | `12,16,20%` | `20,25,30%` |
| cooldown hours | `24,48,72` | `12,24,48` | `6,12,24` |
| symbol loss stop | `2,3,4% equity` | `4,6,8% equity` | `6,8,10% equity` |
| cycle age hours | `24,48,72` | `48,96,168` | `96,168,240` |
| profit-lock giveback | `25,40,60%` | `35,50,70%` | `50,70,85%` |

验收：

1. active-cycle 风控必须在 backtest-engine 和 trading-engine 语义一致。
2. 每个 stop 触发都必须在事件日志中可见，至少包含 timestamp、symbol、reason、equity_before、equity_after。
3. DD stop 不能靠未来权益曲线判断；只能用当时已知 portfolio equity peak。

### 方向 D：Segment-First Walk-Forward Search

目标：把抗过拟合从报告阶段前移到搜索阶段。

评分函数建议：

```text
score =
  full_ann
  - 2.0 * max(0, full_dd - dd_limit)
  - 15.0 * max(0, min_positive_segments - positive_segments)
  - 0.8 * max(0, h1_2023_contribution - 0.45) * 100
  + 0.4 * aggregate_return_2024_2026
  - 0.5 * segment_ann_stddev
```

硬拒绝规则：

1. `positive_segments < min_positive_segments`。
2. `2024 + 2025 + 2026_ytd` 合计收益为负。
3. `h1_2023_contribution > 0.60`。
4. full DD 超出档位上限 1.25 倍。
5. 单币权重大于 `35%`，除非组合币种数少于 5 且有明确预算原因。

输出字段：

```json
{
  "candidate_id": "glm-mart-core-balanced-001",
  "profile": "balanced",
  "strategy_family": "regime_per_symbol_ema",
  "martingale_core": true,
  "research_only": false,
  "budget_quote": 5000,
  "full": {"annualized_return_pct": 0.0, "max_drawdown_pct": 0.0},
  "segments": {
    "h1_2023": {"return_pct": 0.0, "max_drawdown_pct": 0.0},
    "h2_2023": {"return_pct": 0.0, "max_drawdown_pct": 0.0},
    "2024": {"return_pct": 0.0, "max_drawdown_pct": 0.0},
    "2025": {"return_pct": 0.0, "max_drawdown_pct": 0.0},
    "2026_ytd": {"return_pct": 0.0, "max_drawdown_pct": 0.0}
  },
  "portfolio_symbols": ["BTCUSDT", "XRPUSDT", "BCHUSDT", "DOTUSDT"],
  "acceptance": "rejected_or_promoted_with_reason"
}
```

### 方向 E：Dynamic Multi-Symbol Martingale Portfolio Allocation

目标：组合层决定哪些马丁 bot 运行、各自权重多少，不替代马丁交易方式。

分配因子：

1. regime quality：当前 symbol 的方向一致性和趋势强度。
2. recent martingale health：过去 N 天 cycle 胜率、平均 cycle 时长、最大浮亏。
3. volatility budget：ATR pct 越高，首单或权重越低。
4. spread/min-notional feasibility：小资金下过滤不适合的交易对。
5. funding drag：资金费率成本过高时降低对应方向权重。
6. diversification：同类山寨币相关性过高时降低集中权重。

组合搜索范围：

| 维度 | 候选 |
|---|---|
| universe size | `8,12,18,24` |
| active symbol cap | `4,6,8,12` |
| max symbol weight | `20%,25%,30%,35%` |
| rebalance interval | `6h,12h,24h,72h` |
| allocation model | equal, inverse volatility, regime score, risk parity lite |
| stale cooldown | `24h,48h,96h` |

验收：

1. 最终不能只靠一个 symbol 或一类高度相关 symbol。
2. 每次 rebalance 的权重来源必须可解释。
3. 若动态分配尚未 live-parity，只能作为 research-only；最终三档必须有明确上线实现清单。

## 4. GLM 执行批次

### Task 1：建立搜索台账和证据索引

**Files:**
- Create: `docs/superpowers/reports/2026-07-01-glm-martingale-core-search-ledger.md`
- Create: `docs/superpowers/artifacts/glm-martingale-core/evidence-index.json`

- [ ] **Step 1:** 记录当前分支、commit、数据文件 hash、已读报告清单。

```bash
git rev-parse --abbrev-ref HEAD
git rev-parse HEAD
sha256sum data/market_data_full.db
sha256sum data/funding_rates.db
```

- [ ] **Step 2:** 索引已证伪路径，写入 `evidence-index.json`，字段包含 `path`、`family`、`rows`、`best_ann`、`best_dd`、`why_rejected`。
- [ ] **Step 3:** 提交台账。

```bash
git add docs/superpowers/reports/2026-07-01-glm-martingale-core-search-ledger.md docs/superpowers/artifacts/glm-martingale-core/evidence-index.json
git commit -m "docs: 修复思路 建立马丁核心扩展搜索台账"
git push
```

### Task 2：Regime-Gated Martingale 单策略搜索

**Files:**
- Create or modify: `scripts/glm_regime_gated_martingale_search.py`
- Read: `apps/backtest-engine/src/bin/search_small_capital_martingale.rs`
- Output: `docs/superpowers/artifacts/glm-martingale-core/regime-gated-single-strategy.json`

- [ ] **Step 1:** 只生成 martingale/grid config，禁止生成纯趋势订单逻辑。
- [ ] **Step 2:** 对每个 symbol/direction/filter 跑 segment-first 搜索，优先保存每段都有交易的候选。
- [ ] **Step 3:** 保存每个 profile 的 top 200 和 near-miss 200。
- [ ] **Step 4:** 写入台账并提交。

```bash
git add scripts/glm_regime_gated_martingale_search.py docs/superpowers/artifacts/glm-martingale-core/regime-gated-single-strategy.json docs/superpowers/reports/2026-07-01-glm-martingale-core-search-ledger.md
git commit -m "docs: 修复思路 记录马丁regime过滤搜索进展"
git push
```

### Task 3：ATR/ADX 自适应马丁搜索

**Files:**
- Create or modify: `scripts/glm_atr_adx_martingale_search.py`
- Read: `docs/superpowers/plans/2026-06-02-martingale-indicator-walkforward-plan.md`
- Output: `docs/superpowers/artifacts/glm-martingale-core/atr-adx-single-strategy.json`

- [ ] **Step 1:** 确认 ATR spacing、ATR TP、ADX skip 在当前 backtest/live 中的支持状态。
- [ ] **Step 2:** 若已有 live-parity 支持，纳入最终候选池；若仅 backtest 支持，标记 `research_only=true`。
- [ ] **Step 3:** 先跑小样本 smoke，每档至少 20 个有效候选，再扩大。
- [ ] **Step 4:** 保存 full + 5 段 metrics，写入台账并提交。

```bash
git add scripts/glm_atr_adx_martingale_search.py docs/superpowers/artifacts/glm-martingale-core/atr-adx-single-strategy.json docs/superpowers/reports/2026-07-01-glm-martingale-core-search-ledger.md
git commit -m "docs: 修复思路 记录马丁ATR ADX搜索进展"
git push
```

### Task 4：Active-Cycle 风控验证

**Files:**
- Create or modify: `scripts/glm_active_cycle_risk_probe.py`
- Read: `apps/backtest-engine/src/martingale/kline_engine.rs`
- Read: `apps/trading-engine/src/` if active exit changes are needed
- Output: `docs/superpowers/artifacts/glm-martingale-core/active-cycle-risk.json`

- [ ] **Step 1:** 列出当前已支持的 active-cycle exit 和 portfolio DD stop。
- [ ] **Step 2:** 对已有机制只做参数搜索；对缺失机制先写最小实现计划，再 TDD 实现。
- [ ] **Step 3:** 风控触发必须输出事件证据。
- [ ] **Step 4:** 保存每档 best DD frontier 和 near-miss frontier，写入台账并提交。

```bash
git add scripts/glm_active_cycle_risk_probe.py docs/superpowers/artifacts/glm-martingale-core/active-cycle-risk.json docs/superpowers/reports/2026-07-01-glm-martingale-core-search-ledger.md
git commit -m "docs: 修复思路 记录马丁主动风控搜索进展"
git push
```

### Task 5：动态多币种马丁组合

**Files:**
- Create or modify: `scripts/glm_dynamic_martingale_portfolio.py`
- Output: `docs/superpowers/artifacts/glm-martingale-core/dynamic-portfolio-candidates.json`
- Output: `docs/superpowers/artifacts/glm-martingale-core/dynamic-portfolio-replay/`

- [ ] **Step 1:** 从 Task 2-4 的单策略池中选入候选，只允许 martingale core。
- [ ] **Step 2:** 组合权重按 equal、inverse volatility、regime score、risk parity lite 四类分别生成。
- [ ] **Step 3:** 所有组合必须用 `portfolio_budget_replay` 复验 full + 5 段。
- [ ] **Step 4:** 如果出现达标或接近达标候选，立即执行第 5 节记录协议并提交。

```bash
git add scripts/glm_dynamic_martingale_portfolio.py docs/superpowers/artifacts/glm-martingale-core/dynamic-portfolio-candidates.json docs/superpowers/artifacts/glm-martingale-core/dynamic-portfolio-replay docs/superpowers/reports/2026-07-01-glm-martingale-core-search-ledger.md
git commit -m "docs: 修复思路 记录动态多币种马丁组合搜索"
git push
```

### Task 6：最终三档晋级包

**Files:**
- Create: `docs/superpowers/reports/2026-07-01-glm-martingale-core-final-candidates.md`
- Create: `docs/superpowers/artifacts/glm-martingale-core/final-conservative.json`
- Create: `docs/superpowers/artifacts/glm-martingale-core/final-balanced.json`
- Create: `docs/superpowers/artifacts/glm-martingale-core/final-aggressive.json`

- [ ] **Step 1:** 只有通过收益、DD、预算、多币种、分段、live-parity 六项门禁的候选才可写入 final。
- [ ] **Step 2:** 每档报告必须包含 config、full metrics、segment metrics、equity curve path、capital projection、min-notional 检查、no-lookahead 说明、live parity 说明。
- [ ] **Step 3:** 若没有达标候选，写 frontier report，不伪装成成功。
- [ ] **Step 4:** 提交并推送。

```bash
git add docs/superpowers/reports/2026-07-01-glm-martingale-core-final-candidates.md docs/superpowers/artifacts/glm-martingale-core
git commit -m "docs: 修复思路 归档马丁核心最终候选"
git push
```

## 5. 好结果记录协议

任何候选满足以下任一条件，必须立即记录并提交：

1. Conservative：`ann >= 45%` 且 `DD <= 12%`。
2. Balanced：`ann >= 80%` 且 `DD <= 24%`。
3. Aggressive：`ann >= 100%` 且 `DD <= 36%`。
4. 任一档 full gate 已过，但 segment gate 未过。
5. 任一档 segment gate 已过，但 full 年化或 DD 只差 15% 以内。

记录文件：

- `docs/superpowers/artifacts/glm-martingale-core/promising/<candidate_id>.json`
- `docs/superpowers/reports/2026-07-01-glm-martingale-core-search-ledger.md`

JSON 必须包含：

```json
{
  "candidate_id": "glm-mart-core-balanced-001",
  "recorded_at": "2026-07-01T00:00:00Z",
  "profile": "balanced",
  "strategy_family": "regime_per_symbol_ema",
  "martingale_core": true,
  "research_only": false,
  "budget_quote": 5000,
  "symbols": ["BTCUSDT", "XRPUSDT", "BCHUSDT", "DOTUSDT"],
  "full_metrics": {
    "annualized_return_pct": 0.0,
    "max_drawdown_pct": 0.0,
    "total_return_pct": 0.0,
    "max_capital_used_quote": 0.0
  },
  "segment_metrics": {},
  "config_path": "docs/superpowers/artifacts/glm-martingale-core/promising/glm-mart-core-balanced-001.json",
  "equity_curve_path": "docs/superpowers/artifacts/glm-martingale-core/promising/glm-mart-core-balanced-001-equity.json",
  "no_lookahead_check": "passed_or_failed_with_reason",
  "live_parity_check": "passed_or_research_only_with_reason",
  "acceptance_reason": "why this is promising"
}
```

提交命令：

```bash
git add docs/superpowers/artifacts/glm-martingale-core/promising docs/superpowers/reports/2026-07-01-glm-martingale-core-search-ledger.md
git commit -m "docs: 修复思路 记录马丁核心高潜候选"
git push
```

## 6. 反过拟合门禁

GLM 不得放宽以下门禁：

1. 不允许只用 full-period top 排序。
2. 不允许只保存 2023H1 表现亮眼的组合。
3. 不允许单 symbol 参数过度特化；同一 family 参数必须可跨多个 symbol 解释。
4. 不允许把亏损段藏在组合汇总里；每个候选都要有 5 段明细。
5. 不允许改动目标后宣称达标；若需要降低目标，单独写请求给用户批准。

推荐抽样验证：

```bash
target/release/portfolio_budget_replay \
  --config <candidate.json> \
  --market-db data/market_data_full.db \
  --funding-db data/funding_rates.db \
  --start-ms 1672531200000 \
  --end-ms 1780271999999 \
  --budget-quote 5000 \
  --risk-profile balanced
```

5 段时间窗：

| Segment | Start ms | End ms |
|---|---:|---:|
| `h1_2023` | `1672531200000` | `1688169599999` |
| `h2_2023` | `1688169600000` | `1704067199999` |
| `2024` | `1704067200000` | `1735689599999` |
| `2025` | `1735689600000` | `1767225599999` |
| `2026_ytd` | `1767225600000` | `1780271999999` |

## 7. 提交与远端同步规则

GLM 每完成一个独立证据批次就提交。commit message 必须包含 `问题描述`、`复现路径` 或 `修复思路` 之一。

推荐格式：

```bash
git status --short
git add <evidence files>
git commit -m "docs: 修复思路 记录马丁核心搜索进展"
git push
git status --short
```

每次长任务结束必须检查：

```bash
git status --short
ps -C portfolio_budget_replay --no-headers | wc -l
ps -C search_small_capital_martingale --no-headers | wc -l
```

期望：

1. `git status --short` 为空，或只剩正在运行任务明确会继续写入的临时文件。
2. 没有无人认领的 replay/search 进程。
3. 远端分支已经推送。

## 8. 停止条件

GLM 应在以下情况停止并写报告：

1. 三档 Conservative / Balanced / Aggressive 全部达标。
2. 任一档找到接近达标 frontier，但需要新增 live parity 机制才能上线。
3. 搜索 5 条方向后仍无 near-miss，且证据显示收益/回撤 frontier 没有改善。
4. 需要用户批准放宽目标、扩大到非马丁主策略、启动实盘或发布到 `flyingkid`。

停止报告位置：

```text
docs/superpowers/reports/2026-07-01-glm-martingale-core-stop-report.md
```

停止报告必须明确：

1. 达标候选路径。
2. 未达标 frontier。
3. 每条方向的投入、行数、最好结果和失败原因。
4. 下一步是否仍保持马丁核心。
5. 是否需要用户批准新的范围。
