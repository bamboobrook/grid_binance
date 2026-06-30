# 2026-06-30 ChatGPT Phase A 复核与外部约束检查

> 目标不变：≤5000U 保证金、多币种、抗过拟合、分段均衡、保守 >50%/DD≤10%、平衡 >90%/DD≤20%、激进 >110%/DD≤30%，且实盘可复现。
>
> 本轮仅做离线复核、只读 SQL 聚合和文档整理；未启动 live、未操作 Binance、未写 flyingkid。

## 0. 2026-06-30 追加复核

本轮继续基于当前工作区重跑，结论没有反转：

- 新增永久回归测试：`tests/verification/martingale_robustness_validator_contract.test.mjs`，锁住 `evaluate_gate()` 必须按显式 `budget` 判断 `max_capital_used`。
- 验证命令：
  - `python3 -m py_compile scripts/validate_martingale_portfolio_robustness.py`：exit 0。
  - `node --test tests/verification/martingale_robustness_validator_contract.test.mjs`：1 pass / 0 fail。
  - 一次性 Python 行为断言：`evaluate_gate budget regression ok`。
- 当前二进制重跑三条代表候选：
  - Conservative 5000U：`FAIL`，full `33.56% / DD 10.67% / blocked 75`；segment 失败，`2024 DD 14.7%`、`2025 DD 25.4%`、仅 `2/5` 正段。
  - Balanced 5000U：`FAIL`，full `99.42% / DD 24.27% / blocked 68`；segment 失败，`2025 DD 36.6%`、`2026_ytd DD 36.7%`、仅 `2/5` 正段，`2024-2026 = -29.4%`。
  - Aggressive 4000U：`full_period_gate=True`，但最终 `FAIL`；full `120.59% / DD 28.97% / blocked 0`，segment 失败，`H2-2023 DD 48.2%`、`2025 DD 51.3%`、`2026_ytd DD 39.3%`、仅 `1/5` 正段，`2024-2026 = -62.8%`。
- 追加漏网扫描：递归读取 `/tmp/codex_small_search/**/manifest.json` 和已有 replay summary，只保留能定位到具体 JSON config 的候选。
  - manifest 层面 `full-pass-like` 共 `721` 条，全部只属于 aggressive；conservative/balanced 为 `0`。
  - 基于已有 replay summary 的实际 full gate，`passes.aggressive` 只有 `8` 条，conservative/balanced 仍为 `0`。
  - 8 条 aggressive full-pass 全部复跑 segment robustness，全部 `FAIL`：
    - `0105_full_pool_b3000_top_12_fixed_cash_b3250`：full `133.54% / DD 29.88% / blocked 0`，但 segment `FAIL`；仅 `1/5` 正段。
    - `0105_full_pool_b3000_top_12_fixed_cash_b3500`：full `128.82% / DD 29.57% / blocked 0`，但 segment `FAIL`；仅 `1/5` 正段。
    - `0105_full_pool_b3000_top_12_fixed_cash_b3750`：full `124.52% / DD 29.26% / blocked 0`，但 segment `FAIL`；仅 `1/5` 正段。
    - `0105_full_pool_b3000_top_12_fixed_cash_b4000`：full `120.59% / DD 28.97% / blocked 0`，但 segment `FAIL`；仅 `1/5` 正段，`2024-2026 = -62.8%`。
    - `0105_full_pool_b3000_top_12_fixed_cash_b4250`：full `116.97% / DD 28.67% / blocked 0`，但 segment `FAIL`；仅 `1/5` 正段，`2024-2026 = -60.1%`。
    - `0105_full_pool_b3000_top_12_fixed_cash_b4500`：full `113.63% / DD 28.39% / blocked 0`，但 segment `FAIL`；仅 `1/5` 正段。
    - `0105_full_pool_b3000_top_12_fixed_cash_b4750`：full `110.53% / DD 28.11% / blocked 0`，但 segment `FAIL`；仅 `1/5` 正段，`2024-2026 = -55.5%`。
    - `0178_full_pool_b2000_top_27`：full `115.79% / DD 29.10% / blocked 0`，但 segment `FAIL`；H2-2023/2024/2025/2026_ytd DD 分别约 `74.4/73.9/67.4/66.0%`，`2024-2026 = -91.5%`。

## 1. 复核结论

GLM Phase A 的“不存在达标纯马丁组合”结论，在当前证据下成立。更精确地说：

- 已有可实盘表达的纯马丁/网格候选，没有任何一条同时通过 full gate、segment gate、budget gate、live-parity gate。
- 高年化候选主要来自 2023H1，后续 2024-2026 多数转负或 DD 超标。
- DD 合格的候选，年化远低于 50/90/110 目标。
- funding/carry 作为非 MR 收益源，本地 30 币历史 funding 粗算最高约 10.31%/年，不能单独补足目标年化。

这不是证明“任何交易策略都不可能”，而是证明当前约束下的“纯马丁 + live-parity + ≤5000U + 分段均衡”没有找到可交付解。

### 1.1 原目标逐项验收矩阵

| 要求 | Conservative | Balanced | Aggressive | 当前证据 |
|---|---:|---:|---:|---|
| 小资金可运行 `<5000U` | 部分满足 | 部分满足 | 满足 | 多数候选 budget 在 2000-5000U；aggressive 3250-4750U full-pass 候选 `budget_blocked=0`。 |
| 多币种 | 满足 | 满足 | 满足 | 代表候选覆盖 3-4 个 symbol；不把单 SOL 漏网作为有效候选。 |
| full 年化门槛 | FAIL | 表面接近/部分满足 | 满足 | C 最佳复跑 `33.56% < 50%`；B 代表 `99.42% > 90%` 但 DD/budget/segment 不过；A 8 条 full-pass `110.53%-133.54%`。 |
| full DD 门槛 | FAIL | FAIL | 满足 | C `10.67% > 10%`；B `24.27% > 20%`；A full DD `28.11%-29.88% <= 30%`。 |
| budget 执行性 | FAIL | FAIL | 满足 | C 有 `75` 条 budget blocked；B 有 `68` 条 budget blocked；A 8 条 full-pass `blocked=0`。 |
| 分段均衡/抗过拟合 | FAIL | FAIL | FAIL | C 仅 `2/5` 正段；B 仅 `2/5` 正段且 `2024-2026=-29.4%`；A 8 条 full-pass 全部仅 `1/5` 正段。 |
| live-parity | 通过现有 gate | 通过现有 gate | 通过现有 gate | 代表候选均 `live_parity=True`，但 Binance Algo Order 迁移意味着上线前还要补交易所条件单/主动平仓路径验证。 |
| 最终组合可交付 | FAIL | FAIL | FAIL | 没有任何候选同时满足 full gate、budget gate、segment gate、live-parity gate。 |

验收结论：当前证据没有证明“数学上绝对不可能”，但已经证明 **当前数据、当前纯马丁/网格机制、当前 live-parity 表达能力下没有可交付解**。唯一 full 层面看似达标的是 aggressive，但全部被分段均衡否决。

## 2. 复跑过的关键候选

命令：`python3 scripts/validate_martingale_portfolio_robustness.py ... --budgets 5000`

| 候选 | Profile | Full ann/DD | Full gate | Segment gate | Live parity | 主要失败原因 |
|---|---:|---:|---|---|---|---|
| `best_conservative_core_sat_b5000.json` | C | 33.56 / 10.67 | FAIL | FAIL | PASS | 年化不足、DD 略超、2024/2025 为负、75 条 budget blocked |
| `best_balanced_btc_shortdown_b5000.json` | B | 99.42 / 24.27 | FAIL | FAIL | PASS | DD 超 20、2024-2026 合计 -29.4%、68 条 budget blocked |
| `best_aggressive_fixed_cash_b3250_config.json` | A | 107.65 / 27.83 | FAIL | FAIL | PASS | 年化未过 110、仅 1/5 正段、2024-2026 合计 -53.3% |
| `max_ann_largecap_long_widesl.json` | B | 19.68 / 48.97 | FAIL | FAIL | PASS | 收益不足且 DD 极高，2024-2026 合计 -43.9% |

说明：这四条都通过当前保守 live-parity gate，所以失败不是因为“回测配置无法实盘表达”，而是收益/DD/分段/预算执行性不达标。

补充核对：同一激进配置在 `4000U` 预算下 full gate 表面通过（120.59% / DD 28.97 / blocked=0），符合“低于 5000U”的资金条件；但完整 robustness 仍 FAIL：H2-2023 DD 48.2%、2025 DD 51.3%、2026_ytd DD 39.3%，仅 1/5 正段，2024-2026 合计 -62.8%。因此它是 2023H1 型过拟合候选，不能作为最终激进组合。

## 3. 已有搜索池复核

| 搜索池 | Profile | Full rows | Segment rows | Passes | DD 约束内最高年化 | 年化约束内最低 DD |
|---|---:|---:|---:|---:|---:|---:|
| origpack v7 | C | 319 | 24 | 0 | 30.35 / DD 8.90 | 55.70 / DD 21.17 |
| origpack v7 | B | 312 | 24 | 0 | 53.92 / DD 13.29 | 93.75 / DD 39.85 |
| origpack v7 | A | 314 | 24 | 0 | 64.16 / DD 26.84 | 120.47 / DD 41.56 |
| P4 row combo | C | 260 | 20 | 0 | 11.02 / DD 8.42 | 无 |
| P4 row combo | B | 260 | 20 | 0 | 45.88 / DD 17.74 | 无 |
| P4 row combo | A | 260 | 20 | 0 | 74.31 / DD 25.78 | 无 |
| segment-first run1 | C | 60 | 60 | 0 | 1.46 / DD 8.97 | 无 |
| segment-first run1 | B | 60 | 60 | 0 | 4.22 / DD 11.40 | 无 |

解读：

- origpack 能把年化推高，但需要接受 40% 左右 DD，且分段失败。
- P4 combo 更接近实盘风控，但 DD 合格时年化大幅低于目标。
- segment-first 抗过拟合最强，但年化接近 0-5%。

Validator 审计备注：审计时发现 `scripts/validate_martingale_portfolio_robustness.py` 内部一个未使用的 `evaluate_gate()` helper 保留了错误条件 `(max_capital_used <= 0)`；主流程没有调用该 helper，实际 `full_period_gate` 使用的是正确的 `ann/DD/principal/budget_blocked` 判断。已修正该 helper，并用一次性断言验证 helper 对 4000U 激进型达标 full 指标返回 `True`、超预算返回 `False`。本报告另用独立脚本重算了 `docs/superpowers/reports/replay_*.json` 的 full gate，结果与 JSON `gate.passed` 一致。

历史 manifest 再扫描：递归扫 `/tmp/codex_small_search/**/manifest.json` 后，仅找到 1 个带具体配置路径的 aggressive full-pass-like 记录：`direct_b5000_SOLUSDT...`，manifest 中为 132.41% / DD 27.98。复跑当前 `portfolio_budget_replay` 后确认该数字实际来自 2024 单段；full period 为 -1.06% / DD 4.21，且只有 1/5 正段。因此它不是漏网最终候选。

2026-06-30 追加漏网扫描修正：更宽松地解析 manifest 与 summary 后，发现多个 aggressive full-pass-like 是同一组低资金 fixed-cash 变体或 LP top 变体；它们虽然 full gate 可过，但 5 段 robustness 全失败，模式仍是 `H1-2023` 极高收益覆盖后续连续亏损。没有发现 conservative/balanced 的 full-pass 候选。

## 4. Funding 粗算

本地 `data/funding_rates.db` 覆盖 30 个币，2023-01-01 到 2026-06-22。按“空永续收正 funding”的乐观口径，逐币求 `sum(funding_rate) / days * 365`：

- 全样本平均：4.44%/年。
- 最高：DYDXUSDT 10.31%/年。
- ≥20%/年的币：0/30。
- ≥50%/年的币：0/30。
- 分年平均：2023=5.11%、2024=11.84%、2025=0.34%、2026=-4.00%。

这还没扣现货买卖成本、借贷成本、基差滑点、再平衡成本和交易所限制。因此 funding sleeve 可以做低相关收益源，但不能单独把纯马丁组合补到 50/90/110。

## 5. 外部约束核对

- Binance USD-M 实盘必须用 [`exchangeInfo`](https://developers.binance.com/docs/derivatives/usds-margined-futures/market-data/rest-api/Exchange-Information) 的 symbol filters 校验下单数量、价格 tick、step size、最小名义值等；本地 replay 目前用 `--exchange-min-notional 5` 作为统一近似，最终上线前仍需逐 symbol 精确过滤。
- Binance USD-M [`New Order`](https://developers.binance.com/docs/derivatives/usds-margined-futures/trade/rest-api) 接口存在 `reduceOnly` 等平仓参数约束；若新增 portfolio-stop/cycle-trailing，实盘必须以 reduce-only 平仓路径实现。
- Binance USD-M [`New Algo Order`](https://developers.binance.com/docs/derivatives/usds-margined-futures/trade/rest-api/New-Algo-Order) 是当前 TP/SL/条件单的官方路径，支持 `STOP_MARKET`、`TAKE_PROFIT_MARKET`、`STOP`、`TAKE_PROFIT`、`TRAILING_STOP_MARKET`。若未来把回测中的 TP/SL 依赖交易所条件单，而非引擎主动触发，就必须补 `algoOrder` live-parity。
- Binance 2025-11-06 change log 明确说明 USDⓈ-M Futures 条件单从 2025-12-09 迁移到 Algo Service，影响 `STOP_MARKET`/`TAKE_PROFIT_MARKET`/`STOP`/`TAKE_PROFIT`/`TRAILING_STOP_MARKET`。这意味着最终实盘不能只检查旧 `/fapi/v1/order`，还要验证 `/fapi/v1/algoOrder` 或引擎主动触发平仓路径。
- Binance [`Get Funding Rate History`](https://developers.binance.com/docs/derivatives/usds-margined-futures/market-data/rest-api/Get-Funding-Rate-History) 是实盘 PnL 对齐的必要输入；回测 funding 与实盘 income history/funding fee 口径必须对齐。
- [Deflated Sharpe Ratio](https://papers.ssrn.com/sol3/papers.cfm?abstract_id=2460551) / backtest overfitting 的文献约束与本项目现象一致：大量参数试验后只挑 full-period top，会系统性选中 2023H1 过拟合候选。
- [Probability of Backtest Overfitting](https://papers.ssrn.com/sol3/papers.cfm?abstract_id=2326253) 的 CSCV/PBO 框架也支持本项目的筛选纪律：大量参数/窗口/币种组合里只挑最优 full-period，不能视为抗过拟合证据。
- [Dynamic Grid Trading Strategy](https://arxiv.org/abs/2506.11921) 的研究方向也支持当前判断：传统 grid 在简单假设下期望收益接近 0，要改善必须动态适应市场条件。
- 网格/马丁风险资料同样指向本地观察：grid/martingale 适合震荡，遇到持续趋势会累积风险；要提高收益通常会扩大尾部回撤，而不是免费提高风险调整收益。

## 6. 下一步建议

继续找“满足原目标”的唯一有意义路线不是再扫 multiplier，而是改变收益来源：

1. **非马丁趋势 sleeve 最小验证**：先 backtest-only 加 Donchian/highest-lowest 或 breakout entry，验证 2024 趋势段是否能显著盈利；若不能，不投入 live-parity。
2. **Funding sleeve 作为低相关补充**：只作为 5-10% 量级收益源和波动平滑，不把它当 50%+ 主收益。
3. **高时间框 regime + 动态币种选择**：减少 1m 噪声，避免 2024/2025 反相关状态互相拖累。
4. **若坚持“纯马丁”定义**：应把可达目标改为约 C 10%/DD12、B 15%/DD18、A 20%/DD25，再做上线级预算矩阵。

### 6.1 趋势 sleeve 粗探针

为确认是否值得做引擎扩展，用本地 futures 1m 数据抽 UTC 日收盘，做了一个非最终、无引擎改动的趋势收益源探针：

- 符号：BTC/ETH/SOL/INJ/AAVE/LINK/DOGE/ADA/BNB/XRP。
- 区间：2023-01-01 到 2026-05-31。
- 规则：日线 EMA50/200、EMA20/50、20 日动量，多/空/多空切换。
- 成本：每次换仓 2 bps，未计 funding、滑点细节、仓位上限、逐仓约束。

结果：

- `BNBUSDT ema20_50_ls`：full ann 48.0%、DD 35.2%、4/5 正段、2024-2026 合计 +204.1%。
- `BTCUSDT ls_ema`：full ann 28.3%、DD 34.3%、4/5 正段、2024-2026 合计 +73.8%。
- `ETHUSDT long_ema`：full ann 12.1%、DD 36.6%、5/5 正段、2024-2026 合计 +33.3%。
- 随机组合这些日线趋势收益流 20,000 次后，C/B/A 原门槛仍 0 pass；能到 79-119% 年化的组合，DD 约 45-75%，远超目标。
- 加入理想化组合 DD stop/cooldown 后，8,000 次随机组合仍 0 pass：
  - DD≤10% 的最高年化约 6.6%。
  - DD≤20% 的最高年化约 40.2%。
  - DD≤30% 的最高年化约 65.0%。
- 扩展到 funding 表覆盖的 30 个 futures 符号后，单流最高是 `ZECUSDT mom20_ls` ann 74.0% / DD 66.4%；正段≥4 且 2024-2026 为正的最高是 `INJUSDT mom60_ls` ann 64.8% / DD 82.0%。没有发现 DD 与收益同时接近原门槛的漏网趋势标的。

解读：趋势 sleeve 有真实收益源迹象，尤其能覆盖 2024 趋势段，但 DD 仍高，且这不是马丁策略、也不是 live-parity 回测结果。下一步若继续原目标，应先做 **backtest-only 的最小趋势/突破引擎验证**，证明趋势 sleeve 能在组合层降低 DD 后仍贡献足够年化，再决定是否补 live-parity。

## 7. 安全状态

复核结束时：

- `portfolio_budget_replay` 进程数：0
- `search_small_capital_martingale` 进程数：0
- P4/origpack/segment-first Python 搜索进程数：0
