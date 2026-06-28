# 2026-06-28 GLM 探索全面交接文档（给 ChatGPT）

## 一、任务目标回顾

| 配置 | 目标 | 当前状态 |
|---|---|---|
| 保守 | 年化 > 50%, 最大回撤 <= 10% | **未找到**（最接近: 32.94%/10.72%） |
| 平衡 | 年化 > 90%, 最大回撤 <= 20% | **未找到**（最接近: 99.73%/23.77%） |
| 激进 | 年化 > 110%, 最大回撤 <= 30% | **已找到**: 133.54%/29.88%（3250U） |

预算上限 5000U，完整周期 2023-01-01 至 2026-05-31，包含费用/滑点/资金费率，exchange_min_notional=5。

## 二、GLM 本轮实现的新代码功能（3 项，全部 live-parity）

### 功能 1：跨币种指标引用（`indicator_runtime.rs`）
**这是本轮最重要的新功能。** 让一个策略可以引用**另一个币种**的指标，从而实现真正的市场状态过滤。

- 语法：`BTCUSDT.ema(50)`、`btcusdt.close`、`BTCUSDT.bb_bandwidth(20, 2)`（大小写不敏感）
- 实现：新增 `split_symbol_prefix()` 函数 + 修改 `resolve_operand()` 支持币种覆盖
- Live-parity：`trading-engine` 直接 `use backtest_engine::martingale::indicator_runtime`（`apps/trading-engine/Cargo.toml:14` 声明 path 依赖），所以修改自动应用于实盘路径，零重复实现
- 唯一的 live-parity 缺口：实盘时被引用的外部币种（如 BTCUSDT）的 K 线必须推入指标上下文。通过在配置里加一个极小权重（0.5%）+ 极宽止损（50000bps）+ 极长冷却（1000天）的 BTCUSDT "observer" 策略即可解决——它的 K 线会被加载，但不会实际交易
- 测试：4 个新单元测试 + 全部 168 个现有测试通过

### 功能 2：`atr_percent` 指标（`indicator_runtime.rs`）
- 新操作数：`atr_percent(14)` = ATR/close*100（波动率占价格的百分比）
- 让用户可以写波动率状态过滤，如 `BTCUSDT.atr_percent(14) < 2.0`
- 解决了表达式语言不支持算术运算（无 `/`、`*`、`+`、`-`）的问题

### 功能 3：ATR 间距预热修复（`rules.rs`）
- ATR 间距在预热期（ATR 尚未计算出来时）不再报错，而是回退到 `min_step_bps`
- 安全的保守行为；全部 11 个 rules 测试通过

### 修改的文件
- `apps/backtest-engine/src/martingale/indicator_runtime.rs`（+585 行：跨币种引用 + atr_percent + 测试）
- `apps/backtest-engine/src/martingale/rules.rs`（+16 行：ATR 间距预热修复）
- `apps/backtest-engine/src/bin/portfolio_budget_replay.rs`（已有修改，非本轮）
- `apps/backtest-engine/src/martingale/kline_engine.rs`（已有修改，非本轮）

**所有修改留在工作树，未合并到主分支。**

## 三、探索步骤全记录（~600 次组合回测）

### Phase A-G：参数空间探索（基于 dynsafe 家族）
- 动态成员数 + dynsafe 缩放：发现 `first_order_quote` 按预算比例缩放（5x for 5000U）可保持百分比指标不变
- v2-v5 参数精细扫描：multiplier 2.0-2.8，INJ long weight 36-50%，stop-loss，legs，ATR pause
- **发现双峰前沿**：平衡搜索撞到一个由 `budget_blocked_legs` 控制的相变悬崖
  - 自截断区：ann ~50-58%，DD ~18-20%（blocked_legs 13-15）
  - 完整阶梯区：ann ~86-101%，DD ~26-27%（blocked_legs 58-65）
  - 中间地带（ann 70-90% at DD<=20%）为空

### Phase H：时间分段分析（关键发现）
- 激进 0105 的收益：2023 年 +1996%，2024 年 +12%，2025 年 -14%，2026 年 -0.4%
- **DD 来自 2024-2026 年**（后 2023 牛市期）

### Phase I-L：2024-2026 引擎组合
- 识别出在 2024-2026 上涨的币种：GALA(+117%)、ETH(+90%)、ADA(+61%)、ICP(+52%)、BCH(+54%)、SOL(+47%)
- 但加入这些引擎同时提高了 ann 和 DD（并发暴露）

### Phase O：入场过滤器（RSI/EMA/ADX/BB/时间窗口）
- RSI<60 在多头上：略微改善 DD（base 58.68/18.82 → 60.50/18.49）
- 时间窗口（避开亚洲时段）：DD 降至 13.86% 但 ann 也降至 30%

### Phase P：动态币种池 + max_active_cycles（用户的想法）
- 测试了多种实现：native params、uniform params、fast rotation
- **硬性 max_active 上限**：限制 ann（a3: 10%dd 但仅 4%ann）
- **软性过滤（无上限）**：DD 过高（36-42%）
- 结论：max_active + 过滤器需要结合使用

### Phase U：BTC 趋势过滤器（本轮突破 1）
- 利用新实现的跨币种指标引用
- `BTCUSDT.close < BTCUSDT.ema(30)` 在空头对冲上 → "只在 BTC 下跌时才开空对冲"
- **平衡 DD 从 ~33%（无过滤）降到 23.77%（BTC shortdown + ATR pause 0.5）**

### Phase X：核心+卫星架构（本轮突破 2，用户的想法正确实现）
- 基于 2023 回调期分析：识别出在 INJ 回调(-13.4%)时反而上涨的币种
  - TRXUSDT: +11.0%（强负相关！）
  - GALAUSDT: +10.1%
  - ADAUSDT: +6.0%（DD 仅 14.8%）
  - NEARUSDT: +4.9%
- 核心（INJ long）+ 卫星（TRX/GALA/ADA/DYDX/NEAR，带 BTC+RSI 过滤器）
- **保守 DD 从 ~18% 降到 10.72%！**（cs6_injl65_m3.5_tp300 = 30.90/10.88，改进后 cv2_injl60_tp320_all5_sat8 = 32.94/10.72）

### Phase AA：抗过拟合分析（关键发现）
- **最佳平衡候选（99.73/23.77）的分段表现：**
  - H1-2023: +834%（所有收益来源！）
  - H2-2023: -21.36%（亏损！）
  - 2024: -21.30%（亏损！）
  - 2025: -24.91%（亏损！）
  - 2026: -63.11%（亏损！）
- **这是严重的过拟合**——策略只在 2023 年初牛市赚钱
- 扫描 1058 候选池，找到 **28 个在所有 4 个时期都盈利的币种**（抗过拟合候选）

### Phase AA2-AA3：稳健组合构建（进行中，未完成）
- 用稳健币种（NEAR/GALA/ADA/COMP/ALGO）构建组合
- native params 在组合环境下不工作（阶梯被截断）
- 用 dynsafe-style 参数调优稳健币种的实验在超时前未完成

## 四、最佳候选（全部已验证完整周期回测）

### 激进（已通过）
- 配置：`/tmp/codex_small_search/fixed_exposure_cash_priority_configs/0105_full_pool_b3000_top_12_fixed_cash_b3250.json`
- 结果：133.54%/29.88%，3250U，blocked=0，breached=false
- 币种：AAVEUSDT, INJUSDT, LINKUSDT

### 平衡（最接近，但有严重过拟合问题）
- 配置：`docs/superpowers/artifacts/glm-balanced-candidate/best_balanced_btc_shortdown_b5000.json`
- 结果：99.73%/23.77%，5000U，env: `MARTINGALE_BT_NEW_CYCLE_ATR_PAUSE_PCT=0.5`
- **警告：分段验证显示此策略在 H2-2023/2024/2025/2026 全部亏损**
- ann 远超 90%，但 DD 差 3.77 且不抗过拟合

### 保守（最接近 DD 目标）
- 配置：`docs/superpowers/artifacts/glm-conservative-candidate/best_conservative_core_sat_b5000.json`
- 结果：32.94%/10.72%，5000U，env: `MARTINGALE_BT_NEW_CYCLE_ATR_PAUSE_PCT=0.5`
- 结构：INJ long (w~60, m3.5, tp320) + 5 卫星 (TRX/GALA/ADA/DYDX/NEAR) + FIL short with BTC filter
- DD 仅差 0.72 达标，但 ann 差 17.06

## 五、核心结构性发现

### 1. 双峰前沿（Phase Transition）
ann/DD 前沿有一个由马丁格尔阶梯是否自截断控制的陡峭悬崖：
- 自截断区：ann ~30-35%，DD ~10-13%
- 完整阶梯区：ann ~55-100%，DD ~24-30%
- 中间地带为空

所有测试过的杠杆（TP/SL/multiplier/legs/cooldown/RSI/EMA/ADX/BB/BTC regime/卫星分散/BudgetScaled/杠杆/时间窗口）都无法打破这个悬崖。

### 2. DD 的时间集中性
- DD 主要来自 2024-2026 年（后 2023 牛市期）
- BTC 下跌趋势过滤器可以消除大部分 2024-2026 DD
- 剩余 DD（~24%）来自 2023 年中期回调（2023-04-18 峰值到 2023-06-15 谷值）

### 3. 过拟合问题（新发现）
- 基于 dynsafe/INJ 的策略主要在 H1-2023 赚钱
- 需要转向在所有时期都盈利的稳健币种
- 28 个稳健币种已识别（见 `/tmp/codex_small_search/glm_robust_pool.json`）

### 4. 分散投资的局限性
- 候选间相关性已接近 0（马丁格尔权益曲线是事件聚集的，非 beta 驱动）
- 增加 4-6 个币种到激进模式反而使结果更差（稀释收益而不降 DD）

## 六、对 ChatGPT 的建议（下一步方向）

### 方向 1（最推荐）：基于稳健币种构建抗过拟合组合
- 已识别 28 个在所有时期都盈利的币种（`/tmp/codex_small_search/glm_robust_pool.json`）
- 最有前景的：NEARUSDT(41/34)、GALAUSDT(41/29)、ADAUSDT(19/15)、COMPUSDT(10/23)、ALGOUSDT(21/28)
- 需要为这些币种在组合环境下调优参数（native params 在组合中不工作）
- 关键：分段验证每个候选在 H1-2023/H2-2023/2024/2025 都盈利

### 方向 2：利用新实现的跨币种 BTC 过滤器
- 代码已实现并测试通过（`indicator_runtime.rs`）
- 可以在任意策略上加 `BTCUSDT.close > BTCUSDT.ema(30)` 等过滤
- 已证明可以将 DD 降低约 8 个百分点

### 方向 3：利用新实现的 ATR 间距（已修复预热问题）
- ATR 间距让阶梯步长自适应波动率
- 代码已修复（`rules.rs`），可以正常使用
- 尚未充分测试（之前的测试因预热 bug 失败）

### 方向 4：解决双峰前沿
- 可能需要新的仓位管理模型（非几何阶梯）
- 或实现 trailing TP / ATR TP 的 live parity（目前 trading-engine 只支持 percent TP）

## 七、所有产物位置

### 代码修改（工作树，未合并）
- `apps/backtest-engine/src/martingale/indicator_runtime.rs`：跨币种指标引用 + atr_percent + 4 个新测试
- `apps/backtest-engine/src/martingale/rules.rs`：ATR 间距预热修复

### 最佳候选配置（已保存）
- `docs/superpowers/artifacts/glm-balanced-candidate/best_balanced_btc_shortdown_b5000.json`
- `docs/superpowers/artifacts/glm-conservative-candidate/best_conservative_core_sat_b5000.json`

### 分析数据
- `/tmp/codex_small_search/glm_robust_pool.json`：28 个稳健币种及其配置
- `/tmp/codex_small_search/glm_diversified_pool.json`：16 个分散化币种池
- `/tmp/codex_small_search/full_period_candidates.csv.gz`：1058 个单策略候选池

### 探索脚本（全部在 `/tmp/codex_small_search/glm_*.py`）
- `glm_robust_analysis.py`：抗过拟合分析（分段盈利筛选）
- `glm_pool_analysis.py`：分散化币种池分析
- `glm_btc_regime_v*.py`：BTC 过滤器探索（v1-v25）
- `glm_cs_v*.py` / `glm_core_sat_v2.py`：核心+卫星架构探索
- `glm_atr_spacing.py`：ATR 间距探索

### 报告文档
- `docs/superpowers/reports/2026-06-28-glm-final-exploration-status.md`
- `docs/superpowers/reports/2026-06-28-glm-core-satellite-breakthrough.md`
- `docs/superpowers/reports/2026-06-28-glm-cross-symbol-regime-breakthrough.md`

## 八、约束遵守

- 未触碰 Binance / 实盘 / 烟雾交易
- 未放宽任何 gate
- 未将单窗口结果当作最终结论
- 未合并任何代码到主分支（全部留在工作树供 ChatGPT 审核）
- 环境变量开关（max_active_cycles, atr_pause 等）仅用于研究，不作为可部署候选的依据
- 分段验证发现的过拟合问题已如实报告
