# 2026-06-29 ChatGPT P2 计划执行报告:2025 单段单策略搜索与全周期 regime 验证

> 执行 ChatGPT 计划 `docs/superpowers/plans/2026-06-28-chatgpt-verdict-after-glm-optimization.md` 的 P2(2025-focused 单策略搜索)+ Task 3/4/5。结论:**2025 正收益源确实存在,但现有 martingale-only + 现有 filter 无法组合出达标三档;确认需进 P4(regime_break_stop + max_cycle_age_hours)。**

## 1. 方法

- **搜索**:`search_small_capital_martingale`,时间段 2025-01-01..2025-12-31,budget 3000,17 symbol(robust_pool BTC/TRX/XRP/BCH/ETC/LTC/HBAR/DOT + diversified INJ/AAVE/GALA/NEAR/ADA/UNI/APT/COMP/ICP),direction long_only/short_only/long_and_short,entry-filter 全 9 档(none/trend/trend_rsi/rsi_extreme/rsi_moderate/bb_extreme/bb_moderate/rsi_bb_extreme/rsi_bb_moderate),grid small,`--max-params-per-symbol-budget 50`。TP/SL 硬编码 Percent+StrategyDrawdownPct(live-parity)。**13974 trial / 52min**。
- **全周期分段验证**:自建 `scripts/validate_2025_single_strategy_segments.py`,复刻 search 的 config 构造语义(entry_filter→trigger 表达式精确对齐、indicators=atr(21)+adx(14)),用 `portfolio_budget_replay` 跑 full+5 段,带自校验(比对 replay 2025 vs search 2025)。产物:`docs/superpowers/artifacts/glm-p0-search/screen/2025_single_3000.json` + `docs/superpowers/reports/2025_single_strategy_segments.json`。

## 2. 2025 单段 Pareto 前沿(回答"2025 有无正收益单策略")

| 前沿档 | symbol | 方向 | filter | ann% | DD% |
|---|---|---|---|---|---|
| highest_annualized | BCH | short | bb_moderate | 434.0 | 47.5 |
| best_under_dd10 | APT | short | rsi_moderate | 27.2 | 8.0 |
| best_under_dd20 | ICP | long_and_short | bb_extreme | 75.6 | 19.5 |
| best_under_dd30 | APT | short | rsi_moderate | 172.2 | 25.9 |
| lowest_dd_over_ann50 | BCH | short | rsi_moderate | 51.1 | 13.7 |
| lowest_dd_over_ann90 | APT | short | rsi_moderate | 108.8 | 20.8 |
| lowest_dd_over_ann110 | APT | short | rsi_moderate | 154.1 | 24.6 |

**结论:2025 正收益单策略存在**。做空 2025 崩盘币(BCH/DOT/APT/ETC/NEAR/COMP,2025 +200~434%)是主力收益源;long trend(BCH/XRP/BTC/TRX,+9~60%)是低收益稳健腿。**这与 GLM 报告"2025 无收益源"的结论相反** —— GLM 失败根因是用 `BTC.close<BTC.ema(30)` 触发 short,但 BTC 2025 只跌 6.4% 触发不了;per-symbol filter(rsi/bb)直接捕捉山寨超买,才是正解(印证 ChatGPT P2"不要只用 BTC filter")。

## 3. 全周期分段验证(short 死 / long 低 / 双方向跨不过 regime)

对 30 个多样化候选(覆盖 short/long/双方向、各 symbol、各 DD 档)跑 full+5 段:

### 3.1 short_only 高 ret → 全周期全死
2023 大牛市做空必亏:DOT short(2025+368%)→ full_ann **-17.3%**/DD 47.9;APT short(+311%)→ **-19.0%**/DD 51.6;ETC short(+303%)→ **-18.4%**/DD **81.9%**。**单一 short 不能跨 regime 存活**。

### 3.2 long_only + trend → 全周期正收益 + 抗过拟合,但 ann 低
| symbol | filter | 2025 | full_ann | full_DD | H1贡献 | 2024-26 |
|---|---|---|---|---|---|---|
| BCH | trend | +28.7 | **+14.25** | 21.9 | 24% | +14.6 |
| XRP | trend | +16.4 | **+12.08** | **13.7** | **15%** | +7.6 |
| XRP | trend(高foq) | +39.7 | **+25.24** | 26.2 | 15% | +2.6 |
| BTC | trend | +9.5 | +5.30 | 22.0 | 22% | +1.4 |
| TRX | bb_moderate | +59.9 | **+28.48** | 49.4 | 23% | +62.3 |

H1 贡献仅 15~27%(远低于 70% 过拟合线),全周期正、2024-2026 赚 —— 真正的抗过拟合腿。但单策略 ann 5~28,组合后仍不够 balanced 90。

### 3.3 long_and_short → 跨不过 2023 H1 转折期
8 个 long_and_short+trend 候选 **full 全亏(ann -1~-6.6),h1contrib=1.0**(亏损 100% 集中在 2023 H1)。DOT foq349:H1 **-20.9%**,但 2024-2026 合计正(2025+46/2026+18)。问题精准锁定在 **2023 上半年牛熊转折期**:ema 滞后导致 short 被套。

### 3.4 探针:ema 周期与 Indicator SL 都不是解
- **ema 周期**:ema50/100 比 ema200 仅略优(full 仍亏 -2~-3)。entry filter 无法解决"已开仓 short 在牛市被套"。
- **Indicator SL(单条件 regime 退出 `close>ema`)**:让结果**更差**(BCH ema100:2025 从 +2.5→+30.2,但 H1 从 -8.7→-14.5)。牛市震荡里 close 频繁穿越 ema,反复开平亏手续费。**真正的 regime_break_stop 需"close<ema 且浮亏>X"复合条件**,而回测 expression 语言不支持 AND,Indicator SL 模拟不了。

### 3.5 long_only trend 组合尝试
BCH+XRP+BTC+TRX 等权组合:full **ann 16.05 / DD 36.97**,2025 段 +85.9%(分散效应好),但 **2026 大亏 -31.4** 拖累,H1 贡献 59%。远低于 balanced 90/20。

## 4. 结论(对齐 ChatGPT 第 7 节失败证明)

- **搜索空间**:17 symbol × 9 filter × 3 dir × small grid(budget 3000);**trial 13974**。
- **2025 单段 Pareto 前沿**:见 §2(DD≤10 最高 ann 27.2;DD≤20 最高 75.6;ann>50 最低 DD 13.7;ann>110 最低 DD 24.6)。
- **2025 正收益单策略存在**:是(short 崩盘币 + long trend)。
- **全周期存活**:short 全死;long 正但 ann 低 + 2026 亏;long_and_short 跨不过转折期。
- **P3(纯组合)不可行**:无论怎么组合现有 filter 的单策略,都跨不过"2025 short 收益 vs 全周期存活"+ "分段不一致(2026 亏)"双重锁死。

## 5. 必须进 P4(对齐 ChatGPT 第 5 节 P4 + 行动清单第 4 条)

突破点已锁定:**cycle 级退出机制**。需实现并补 live parity(backtest + trading-engine),再重搜:

1. **`regime_break_stop`**:long cycle 持仓中,若本币种 `close<ema(50/100)` **且浮亏>X**,则停补或平仓;short 反向。关键是要**复合条件**(close<ema AND drawdown>X),现有 `MartingaleStopLossModel::Indicator` expression 不支持 AND,需扩展 expression 语言或新增 trigger/SL 类型。
2. **`max_cycle_age_hours`**:cycle 超 N 小时未 TP,按 StrategyDrawdownPct 或 reduceOnly 平仓。先扫 24/48/72/120/168h。
3. **live parity**:backtest `kline_engine.rs` + trading-engine `martingale_runtime.rs` + `martingale_exit.rs` 三处对齐;`live_parity_check` 接入(当前未接线)。

## 6. 附:可直接落地的副产物
- **抗过拟合基线**:long_only trend 候选(BCH/XRP/BTC,full ann 5~25、H1 贡献 15~27%)可作保守/抗过拟合组合素材(ann 达不到 conservative 50,但 segment 健康远胜旧 aggressive)。
- **工具**:`scripts/validate_2025_single_strategy_segments.py`(2025 段搜索候选 → 全周期分段验证,带自校验)。
- **P7 待办**:`validate_martingale_portfolio_robustness.py` 的 `evaluate_gate()` 死 bug(max_capital_used<=0 永假,未被调用但定时炸弹)+ 接入 Rust `live_parity_check`。
