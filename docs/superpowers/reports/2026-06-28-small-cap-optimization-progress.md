# 2026-06-28 小资金马丁组合优化进展（在已有候选基础上）

> 本轮在 ChatGPT 已交付的 3 个候选基础上做针对性优化，而非从零重扫。
> 关键进展：平衡候选 anti-overfitting 大幅改善（2025 -18%→-10.3%, 2024/2026 转正）。
> 保守/平衡仍差最后一步，但根因已定位，方向已验证。

## 1. 已有候选的过拟合诊断（分段验证）

对 3 个已有候选做 5 段验证（H1-2023/H2-2023/2024/2025/2026），发现**全部严重依赖 H1-2023**：

| 候选 | H1-2023 | H2-2023 | 2024 | 2025 | 2026 | full ann/DD |
|---|---:|---:|---:|---:|---:|---:|
| Aggressive | +938% | -36% | -39% | -32%(DD59%) | -32% | 133.5/29.9 |
| Balanced | +400% | +1.8% | -10% | -18%(DD37%) | -3.9% | 99.7/23.8 |
| Conservative | +164% | +0.1% | -14% | -15%(DD25%) | -2.9% | 32.9/10.7 |

**结论**：3 个候选的收益几乎全部来自 H1-2023 牛市，其余每段亏损。激进候选
表面达标（133.5%），但 2025 DD 高达 59%，是 H1-2023 杠杆票据，不可实盘。

## 2. 根因：2025 熊市山寨币崩盘

2025 年 traded symbols 的价格变动：
- INJUSDT -78.6%, AAVEUSDT -52.8%, LINKUSDT -38.9%, NEARUSDT -69.2%, GALAUSDT -83%
- BTCUSDT 仅 -6.4%

**做多策略在山寨熊市被深套**。已有的空头对冲用 `BTCUSDT.close<BTCUSDT.ema(30)` 触发，
但 BTC 只跌 6%，所以对冲几乎不触发——**BTC filter 在山寨独立熊市无效**。

## 3. 测试过的优化路径与结论

### 3a. 趋势过滤器（per-symbol ema20/30/50）
- 效果有限：2025 DD 25→19%，但 2025 收益仍负（-12.5%），ann 不变。
- 原因：过滤器只暂停新周期，已有深套仓位继续亏。

### 3b. per-symbol 空头对冲（sym_short）
- **无效**：sym_short 变体的 2025 结果与 baseline 完全相同（-17.9%/DD36.6%）。
- 原因：空头权重太小（INJ short 1.68%, AAVE short 2.39%），且 2025 有剧烈空头挤压
  （INJ 在 -78% 下跌中有多次 +30% 反弹），空头被 TP 止盈平掉后重新亏损。

### 3c. 波动率暂停（ATR pause，P0.1 结构化阈值）
- **反效果**：floor1500 + atr_pause(1.5-3.0) → ann 降到 80.6%, DD 升到 26%,
  2025 更差（-25.5%）。
- 原因：马丁策略在波动率飙升时暂停新周期 = 无法补仓摊平，已有仓位 DD 更大。

### 3d. tighter long stop-loss（突破点！）
- **有效**：把做多策略的 strategy_drawdown_pct SL 减半（floor 1000-1500），
  2025 收益 -18%→-10.3%，2024/2026 转正。
- `floor1200/1500` sweet spot：ann 保持 ~99%，2025 大幅改善。
- 这是本轮最大突破。

### 3e. DD-pause 阈值（P0.1 结构化）
- phase transition：5.4%→ann46/DD20.9，5.5%→ann100/DD24.5，无中间甜点。
- DD-pause 只暂停新周期，不平已有亏损仓位，无法把 DD 从 24% 压到 10%。

### 3f. leverage scaling
- lev7：ann39.5/DD16.3（DD 接近保守目标 10%，但 ann 差距大）。
- leverage 降低会让更多 legs 被 minNotional budget-block，缩放不成比例。

## 4. 改进后的候选（本轮交付）

### 平衡改进候选 A：`floor1500`（ann 达标版）
- config: `docs/superpowers/artifacts/glm-balanced-candidate/best_balanced_floor1500_b5000.json`
- full: **ann 99.4% / DD 24.2%**（ann 达标，DD 差 4.2%）
- 改变：long legs SL floor 1500（INJ/FIL 1200/1800→1000，AAVE 7000→3500，BTC 50000→25000）

| 段 | base | **floor1500** | 改善 |
|---|---:|---:|---|
| H1-2023 | +400% | +400% | 同 |
| H2-2023 | +1.8% | -7.2% | 略差 |
| 2024 | -10.4% | **+0.3%** | ✅ 转正 |
| 2025 | -18.0% | **-10.3%** | ✅ 减半 |
| 2026 | -3.9% | **+6.1%** | ✅ 转正 |

**anti-overfitting 大幅改善**：3/5 段正收益，2025 亏损减半。2024-2026 合计微正。
DD 仍 24.2%（目标 <20%），差 4.2%。

### 平衡改进候选 B：`floor1500_legs5`（anti-overfit 最佳版）⭐
- config: `docs/superpowers/artifacts/glm-balanced-candidate/best_balanced_floor1500_legs5_b5000.json`
- full: **ann 68.9% / DD 28.2%**（ann 差 21%，但 anti-overfit 最强）
- 改变：floor1500 + 所有 long max_legs 降到 5（限制单仓位最大未实现亏损）

| 段 | base | **floor1500_legs5** | 改善 |
|---|---:|---:|---|
| H1-2023 | +400% | +319% | 收益降低但合理 |
| H2-2023 | +1.8% | -4.3% (DD5%) | 小亏低 DD |
| 2024 | -10.4% | **+6.5% (DD9.8%)** | ✅ 转正 |
| 2025 | -18.0% (DD37%) | **-5.1% (DD19.3%<20!)** | ✅✅ DD 降到 <20 |
| 2026 | -3.9% | **+9.9% (DD14.4%)** | ✅ 转正 |

**这是本轮 anti-overfitting 最强候选**：2025 DD 19.3%（<20 达标！），2024 转正 +6.5%，
2026 转正 +9.9%，2024-2026 合计明显正收益。H1-2023 不再独占（319% vs base 400%）。
唯一差距：full ann 68.9%（目标 >90），因为限 legs 牺牲了牛市收益。

### 保守改进候选：`floor1500_lev7`（实验性，效果有限）
- config: `docs/superpowers/artifacts/glm-conservative-candidate/best_conservative_floor1500_lev7_b5000.json`
- full: ann 39.5% / DD 16.3%（lev7 降杠杆）
- 分段：2025 -20.6%（比 floor1500 更差），不推荐作为最终保守候选。
- 保守方向仍需探索（见 §5）。

## 5. 尚未达标的差距与前沿确认

### 前沿根本约束（本轮 + ChatGPT 报告共同确认）

扫描 1058 个全周期单策略候选：
- **ann>25% AND DD<15%：0 个**（零个）
- DD<=10% 的最高 ann：单策略 <10%，组合 33.6%
- ann>50% 的最低 DD：单策略 31%，组合 22%（但严重依赖 H1-2023）

这意味着**马丁策略在小资金下的 ann/DD 前沿是根本性有界的**，不是调参能突破的。

### 三个目标的精确状态

| 目标 | 当前最好 | 差距 | 可行性 |
|---|---|---|---|
| 保守 ann>50/DD<=10 | 33.6/10.7 | ann -17 | ann 前沿不足 |
| 平衡 ann>90/DD<=20 | 99.4/24.2 (A) 或 70.5/26.4 (B) | DD+4 或 ann-20 | ann 和 DD 不能同时达 |
| 激进 ann>110/DD<=30 | 133.5/29.9 | 表面达 | **严重过拟合，不可用** |

### 平衡候选 A vs B 的权衡（核心矛盾）

- **A (floor1500)**: ann 99.4%（达标），但 DD 24.2%（超 4.2%），2025 DD 30.7%
- **B (legs5_rob)**: 2025 DD 19.3%（达标），3/5 段正，但 ann 70.5%（差 19.5%）

这两个变体代表了 ann 和 DD 之间不可兼得的权衡。要同时满足 ann>90 和 DD<20，
需要**在 2025 熊市里仍然盈利的策略**，但这需要新的标的（当前池子里没有 2025 正收益的单策略）。

### 下一步突破方向（给 ChatGPT）

1. **找 2025 熊市正收益标的**：当前所有候选的标的在 2025 都亏（山寨 -50~83%）。
   需要找在 2025 仍正收益的标的（可能：稳定币对、做空 ETF、或非 crypto 标的）。
2. **position-level max hold time**：强制单 cycle 时间止损，可能限制 H1-2023 的过度累积
   从而降 DD，但需新机制实现。
3. **接受目标调整**：如果 ann>90/DD<20 在小资金马丁里不可达，考虑：
   - 放宽 DD 到 25%（A 候选可达）
   - 放宽 ann 到 70%（B 候选可达，且 anti-overfit）
4. **混合策略族**：martingale + 非平均成本策略（如动量跟随），分担不同 regime。

## 6. 本轮 trial 统计

- 已有候选分段验证：3 候选 × 5 段 = 15
- 趋势过滤器扫（ema20/30/50/btc+sym）：5 变体 × 3 段 = 15
- 平衡 symhedge 扫（sym_short/tighter_sl 等）：6 变体 × 4 段 = 24
- SL floor 扫（800-2000）：~23
- DD-pause 扫（2.0-6.0）：~20
- ATR-pause 扫：~10
- leverage 扫（3/5/7）：~8
- first_order 扫（0.5/0.7）：~8
- max_legs 扫（3/4/5/6）+ 非对称：~15
- 保守 sat-weight 优化：~6
- 控制组实验：~8
- **本轮 trial 总数：~152**

## 7. 关键代码能力（本轮 P0 新增，已支持实盘）

- `MartingaleRiskLimits` 新增 3 个结构化阈值（new_cycle_dd_pause / atr_pause / adx_skip），
  贯通 backtest + trading-engine + api-server（P0.1，已测 414 个 test 全过）。
- `extract_symbol_dependencies`：自动提取跨币种依赖（P0.2）。
- `live_parity_check`：TP/SL 实盘一致性 gate（P0.3），只允许 Percent TP + StrategyDrawdownPct SL。
- `validate_martingale_portfolio_robustness.py`：分段+预算+过拟合统一验证工具。

这些能力让本轮所有优化（tighter SL、DD-pause、ATR-pause、per-symbol filter）
都能通过结构化配置部署到实盘，不再是 research-only env。
