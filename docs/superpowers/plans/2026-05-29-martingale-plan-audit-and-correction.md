# Martingale Backtest Plan Audit And Correction

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:receiving-code-review first, then use superpowers:writing-plans or superpowers:executing-plans only after this audit is accepted. This document is an audit/correction plan, not an implementation patch.

**Goal:** 判断当前马丁回测改动到底是实现跑偏，还是原始计划导致跑偏，并给出下一轮 GLM/DeepSeek 必须遵守的纠偏方向。

**Architecture:** 保留已经正确的“全量曲线/全量 K 线/真实成本”基础修复；把后续优化从“单策略先达标”收紧为“组合优先达标”。所有深搜必须先通过曲线完整性、候选池多样性、组合器有效性三个门禁。

**Tech Stack:** Rust `backtest-engine`, Rust `backtest-worker`, Next.js backtest UI, PostgreSQL `backtest_tasks`, local SQLite market data `/home/bumblebee/Project/discord_c2im/pipeline/data/market_data.db`.

---

## 1. Audit Conclusion

当前问题不是“谁接手”的问题，也不是全部实现都错了。结论分三层：

1. **基础真实性修复方向是对的。**
   - `combine_equity_curves()` 改成 timestamp union + forward-fill，方向正确。
   - `portfolio_candidates_from_outputs()` 倾向使用 `output.equity_curve` 全量曲线，方向正确。
   - `run_candidate_kline_screening()` 最终精测改用 `bars_for_candidate()`，方向正确。
   - 风险档位改为保守 `10%`、平衡 `20%`、激进 `30%`，符合当前要求。

2. **后续搜索方向仍不够“组合优先”。**
   - 计划虽然写了 portfolio-first，但实现和门禁仍容易把注意力放在“单策略收益/回撤是否好”。
   - 用户最终要求是：单策略可以高回撤、低收益或不完美，只要组合后满足回撤并尽量年化 `50%+`。
   - 因此候选池、权重优化、排序目标都必须以组合结果为准，而不是以单策略 Top10 为准。

3. **当前计划需要更新，但不需要推倒重来。**
   - `2026-05-29-martingale-7-balanced-profit-search-plan.md` 适合作为“先跑 7 币种平衡验证”的阶段计划。
   - `2026-05-29-martingale-portfolio-first-glm-plan.md` 方向更接近最终目标，但仍要加硬性门禁和测试，防止 GLM/DeepSeek 跑偏。

---

## 2. Current Code Changes Review

### 2.1 Keep These Changes

- `apps/backtest-engine/src/portfolio_search.rs`
  - 保留组合曲线按时间戳合并，不再按数组下标截断。
  - 保留单成员组合按组合本金缩放曲线，避免单策略 raw margin 曲线污染组合本金。

- `apps/backtest-worker/src/main.rs`
  - 保留粗筛用 `screening_candidate_evaluation()`、最终精测用 `run_candidate_kline_screening()` 全量 bars 的分层。
  - 保留组合输入优先使用 `CandidateOutput.equity_curve` 全量曲线。
  - 保留 portfolio/drawdown preview 从 `500` 增加到 `5000`，但必须确认这只用于展示，不用于组合计算。
  - 保留 `portfolio_drawdown_limit_for_task()` 的三档硬限制：`10/20/30`。

- `apps/web/components/backtest/backtest-wizard.tsx`
  - 保留默认最大回撤：保守 `10`、平衡 `20`、激进 `30`。

### 2.2 Must Not Treat These Changes As Complete

当前变更不能直接宣称“已经 100% 符合最终目标”，原因：

1. 计划要求的关键测试名在当前代码中未完全出现：
   - `weighted_portfolio_aligns_member_equity_by_timestamp_not_index`
   - `portfolio_candidates_prefer_full_output_curve_over_summary_preview`
   - `final_kline_refinement_uses_full_symbol_history_not_screening_sample`
   - `portfolio_pool_admits_growth_candidates_above_single_strategy_drawdown_limit`
   - `portfolio_v2_uses_low_weight_growth_leader_with_stabilizers_to_hit_drawdown_limit`

2. 现有测试覆盖了一部分相似行为，但命名和断言不够硬，后续 agent 容易误以为“有测试就够了”。

3. `portfolio_pool_quality_eligible()` 仍然可能过早过滤掉组合中有用的高收益高回撤策略。下一轮必须用测试证明：高收益高回撤候选能进入池，并可低权重进入组合。

4. 组合器虽然已有 `barbell` 和 `stochastic`，但还需要证明它真的能做到：
   - 高收益/高回撤策略低权重；
   - 低回撤/低相关策略做稳定器；
   - 组合硬回撤满足风险档位；
   - 排名优先按组合年化、组合回撤、组合收益/回撤比，而不是单策略分数。

---

## 3. Required Plan Update

请更新 `docs/superpowers/plans/2026-05-29-martingale-portfolio-first-glm-plan.md`，加入以下硬门禁。GLM/DeepSeek 未完成门禁前，不允许创建长时间深搜任务。

### Gate 1: Full-Curve Integrity

必须新增或确认以下测试，并逐条通过：

```bash
cargo test -p backtest-engine weighted_portfolio_aligns_member_equity_by_timestamp_not_index -- --nocapture
cargo test -p backtest-worker portfolio_candidates_prefer_full_output_curve_over_summary_preview -- --nocapture
cargo test -p backtest-worker final_kline_refinement_uses_full_symbol_history_not_screening_sample -- --nocapture
```

验收标准：

- 组合曲线按 timestamp union 合并，缺失点 forward-fill。
- 最终组合计算使用全量 `output.equity_curve`，不是 `summary.equity_curve` 采样预览。
- 最终候选精测使用全量 1m bars，不是 screening sample。

### Gate 2: Portfolio-First Candidate Admission

必须新增测试：

```bash
cargo test -p backtest-worker portfolio_pool_admits_growth_candidates_above_single_strategy_drawdown_limit -- --nocapture
```

验收标准：

- 正收益、高年化、高回撤候选允许进入组合候选池。
- 低回撤稳定器允许进入组合候选池。
- 负收益候选、极低收益高频磨损候选不得进入。
- 单策略最大回撤可以超过组合风险限制，但必须有池级上限，例如 balanced/aggressive 不超过 `65%`、conservative 不超过 `45%`。

### Gate 3: Portfolio Optimizer Must Prove Barbell Allocation

必须新增测试：

```bash
cargo test -p backtest-engine portfolio_v2_uses_low_weight_growth_leader_with_stabilizers_to_hit_drawdown_limit -- --nocapture
```

验收标准：

- 构造一个单策略回撤超过 `20%` 但年化高的 growth candidate。
- 构造多个低回撤稳定器。
- 组合器必须能把 growth candidate 以低权重纳入组合。
- 最终组合最大回撤 `<=20%`，且年化明显高于只选稳定器的组合。

### Gate 4: Ranking Must Be Portfolio-First

组合排序必须以组合结果为准：

1. 先过滤组合最大回撤 `<= risk limit`。
2. 优先最大化组合年化收益。
3. 再比较组合年化/最大回撤比。
4. 再比较组合最大回撤更低者。
5. 不得因为某个成员单策略回撤高就直接排除，除非组合后超出硬回撤。

### Gate 5: Smoke Before Deep Search

只能先跑一个 7 币种 balanced mixed smoke：

```text
BTCUSDT, ETHUSDT, BNBUSDT, SOLUSDT, XRPUSDT, DOGEUSDT, ADAUSDT
risk_profile = balanced
max_drawdown = 20
market = usd_m_futures
direction_mode = mixed_best
fee_bps = 4.5
slippage_bps = 2.0
```

smoke 完成后必须检查：

- Top3 组合是否存在。
- 组合曲线最大时间 gap 不得是几百天。
- 组合成员数、币种数、单币种权重是否符合要求。
- 候选池是否包含 long、short、long_short/mixed 的有效候选。
- 如果年化低于 `35%`，先检查候选池质量，不要直接盲目扩大深搜。

---

## 4. Correct Next Execution Order For GLM

1. **不要先跑深搜。**
2. 先补齐 Gate 1-4 的测试与实现。
3. 跑 `cargo check -p backtest-engine -p backtest-worker`。
4. 跑前端契约测试：
   ```bash
   node --test tests/verification/backtest_console_contract.test.mjs
   ```
5. 部署 worker。
6. 只创建 7 币种 balanced mixed smoke。
7. 验证曲线完整性和候选池质量。
8. 若 smoke 合格，再多 seed 中预算。
9. 若中预算仍达不到目标，再扩大搜索空间。
10. 只有 7 币种 balanced 被验证后，才扩展到 18 币种或保守/激进。

---

## 5. One-Paragraph Instruction For GLM

请先不要跑深搜，先按 `/home/bumblebee/Project/grid_binance/docs/superpowers/plans/2026-05-29-martingale-plan-audit-and-correction.md` 更新并执行 `/home/bumblebee/Project/grid_binance/docs/superpowers/plans/2026-05-29-martingale-portfolio-first-glm-plan.md`：补齐 Gate 1-4 的测试与实现，确保最终精测用全量 1m K 线、组合曲线用全量 output equity curve 且按 timestamp 对齐、候选池允许高收益高回撤策略作为低权重进攻资产进入组合、组合器以“组合年化最大且组合回撤满足平衡 20%”为目标，而不是继续筛单策略 Top10。所有测试和 `cargo check` 通过后，只跑 7 币种 balanced mixed smoke，检查曲线 gap、候选池质量、Top3 组合成员与权重；smoke 合格后再进行多 seed/扩参深搜。
