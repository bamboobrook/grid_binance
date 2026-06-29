# 2026-06-29 P4 cycle-exit 机制交接给 ChatGPT

> **状态**:P4 代码已在 worktree 完成(7 task TDD + 每 task review clean)。待你接手:**① 验证 P4 是否突破(跑搜索 + 全周期分段)→ ② final whole-branch review → ③ merge 回 main**。

## 1. 背景
执行你的计划 `docs/superpowers/plans/2026-06-28-chatgpt-verdict-after-glm-optimization.md`(P1-P7)。P2(2025 单段搜索)已完成 → P4(cycle 级退出机制)代码已完成 → 待验证。
- P2 报告:`docs/superpowers/reports/2026-06-29-p2-2025-segment-search-findings.md`
- P4 spec:`docs/superpowers/specs/2026-06-29-p4-cycle-exit-mechanisms-design.md`
- P4 plan:`docs/superpowers/plans/2026-06-29-p4-cycle-exit-mechanisms.md`

## 2. P2 结论(已完成)
- 2025 正收益源**存在**:short 做空崩盘币(BCH/DOT/APT/ETC/NEAR/COMP)2025 +200~434%。
- 但**单一 short 全周期必死**:2023 大牛市做空亏,full_ann -17~-19,DD 40~80%。
- long_only trend **抗过拟合**(H1 贡献仅 15~27%)但 ann 低(组合 ~16/DD37,且 2026 亏)。
- long_and_short 跨不过 **2023 H1 转折期**(ema 滞后 → short 被套)。
- 结论:**需 cycle 级退出机制(P4)**。

## 3. P4 实现状态(代码完成,review clean)
- **worktree**:`.claude/worktrees/p4-cycle-exit`(branch `worktree-p4-cycle-exit`)
- **base**:`main` `07145c4`(含 GLM P0.1 structured config,已 commit;P0.1 之前是未提交 working tree,我 commit 作 P4 base)
- **8 commits**(`07145c4` → `6d26dcc`):
  - `67ec226` T1 config schema(RegimeBreakStop SL variant + max_cycle_age_hours risk limit)
  - `6ac3a87` T2 backtest max_cycle_age(StrategyRuntime.cycle_started_at_ms + triggered_stop age 分支)
  - `89775d1` T3 backtest regime_break(triggered_stop RegimeBreakStop arm,双条件 AND)
  - `ccc0d90` T4 trading-engine max_cycle_age(CycleState.started_at_ms + martingale_exit_signal age + reconcile 从 leg-0 fill created_at 推导起点)
  - `320f50a` T5 trading-engine regime_break + wire real persisted indicator_ctx(含 `martingale_net_drawdown_pct` 重构)
  - `6d065e9` T6 live_parity_check allow RegimeBreakStop + wire search/publish(P7)
  - `783efe0` T7 search grid 扩展(regime_break + max_cycle_age params)
  - `6d26dcc` glue script P4 schema 支持
- **测试**:backtest 210 pass + trading-engine 186 pass + shared-domain 11 pass + live_parity 8 pass。
- **review**:每 task spec ✅ + quality approved;deferred Minor 见 §7。

## 4. P4 机制设计(关键决策)
- **max_cycle_age_hours**:`MartingaleRiskLimits` 字段(strategy 级,parity gate 不查 risk_limits → 免改 gate);cycle 超 N 小时未 TP → market 平整个 cycle。用 `Option<f64>` 直接透传(不走 resolve_threshold,默认 None=禁用)。
- **regime_break_stop**:`MartingaleStopLossModel::RegimeBreakStop{ema_period, drawdown_pct_bps}`;long: `close<ema` **AND** `drawdown≥thr` → 平仓;short 反向。**双条件 AND**(P2 探针证明单条件 `close>ema` 在牛市频繁误触发)。
- 三处 parity:backtest `triggered_stop` ↔ trading-engine `martingale_exit_signal` ↔ `live_parity_check`。
- 退出走 `ExitDecision::StrategyStop` 通道(不改 `exit_rules` 优先级)。
- 实现路径:路径(2)新 SL 枚举变体(避免扩展 expression AND,parity 简单)。

## 5. 待办(你接手,3 步)

### 步骤 1:验证 P4 是否突破
```bash
cd /home/bumblebee/Project/grid_binance/.claude/worktrees/p4-cycle-exit
PATH=$HOME/.cargo/bin:$PATH cargo build -p backtest-engine --bin search_small_capital_martingale --bin portfolio_budget_replay --release

./target/release/search_small_capital_martingale \
  --budgets 3000 \
  --symbols BCHUSDT,DOTUSDT,APTUSDT,ETCUSDT,NEARUSDT,COMPUSDT,GALAUSDT,ICPUSDT \
  --direction-modes short_only,long_and_short \
  --entry-filters rsi_moderate,bb_moderate,trend_rsi,none \
  --regime-break ema50,ema100,none \
  --max-cycle-age 48,120,none \
  --start-ms 1735689600000 --end-ms 1767225599999 \
  --market-data /home/bumblebee/Project/grid_binance/data/market_data_full.db \
  --funding-data /home/bumblebee/Project/grid_binance/data/funding_rates.db \
  --output /tmp/2025_p4_3000.json --top-n 50 --grid small --max-params-per-symbol-budget 30

python3 scripts/validate_2025_single_strategy_segments.py \
  --search /tmp/2025_p4_3000.json --budget 3000 --profile balanced \
  --select-mode diverse --top 30 --out docs/superpowers/reports/2025_p4_segments.json
```
**验证问题**:regime_break+age 是否让 short 候选在 2023 H1 不再击穿 + full ann 向 balanced 90/DD20 靠近?(对比 P2 的"short 全死、full_ann -17~ -19")。约 9min 搜索 + 20min 分段验证。

### 步骤 2:final whole-branch review
review worktree branch `07145c4..6d26dcc`(用 `superpowers:requesting-code-review`,most capable model;deferred Minor 见 §7)。

### 步骤 3:finishing-a-development-branch
merge `worktree-p4-cycle-exit` 回 main(用 `superpowers:finishing-a-development-branch`)。

## 6. 关键工具/数据位置
- search/replay bin:worktree `target/release/`(已 build)
- market DB:`/home/bumblebee/Project/grid_binance/data/market_data_full.db`(gitignored,主目录)
- funding DB:`/home/bumblebee/Project/grid_binance/data/funding_rates.db`
- 胶水脚本:`scripts/validate_2025_single_strategy_segments.py`(worktree,已支持 P4 row schema:regime_break_ema_period + max_cycle_age_hours)
- SDD ledger:worktree `.superpowers/sdd/progress.md`(gitignored scratch)

## 7. deferred Minor(final review triage)
- T3:backtest RegimeBreakStop 的 EMA warmup 早返回分支(None→default)未单测(trivial/fail-safe,可选 5 行测试钉死)。
- T5:live regime 用 `tick.price` vs backtest `latest_close_by_symbol`(完成 bar close);live 稍敏感,drawdown AND 已过滤瞬时;可选严格化(live 用最近完成 bar close)。
- T6:search parity log 的 `passes=checked-violations_total` 公式在多 violation 时不准(search 总是 parity-clean,total=0,实际正确;仅信息性)。

## 8. 坑/注意事项
- 搜索 output 到 `docs/superpowers/artifacts/` 会失败(worktree 无此目录,gitignored)→ 用 `/tmp/` 或先 `mkdir -p`。
- 第一次搜索已跑完(2624 rows,parity 2624/2624 pass),证明 P4 搜索逻辑工作,仅 output 路径失败。
- `martingale_exit_signal` 签名改了(加 `now_ms`/`cycle_started_at_ms`/`indicator_ctx`),所有调用点已更新。
- brief 原 helper 直接调 `martingale_strategy_drawdown_pct` 会对 RegimeBreakStop 返回 None(它只 match StrategyDrawdownPct)→ T5 重构出 SL-无关的 `martingale_net_drawdown_pct`,已修。

## 9. 验收标准(对齐你的计划 §6)
重搜后候选给:full + segment(H1-2023/H2-2023/2024/2025/2026)return/DD、budget matrix(1000-5000)、live parity、overfit flags(H1 贡献、2024-2026 合计)。**突破标志**:short/long_and_short 加 regime_break+age 后 2023 H1 不击穿、full ann 向 90/20 靠近、segment gate 通过。若仍无解,按 §7 输出失败证明(搜索空间/trial/Pareto/是否改善)。
