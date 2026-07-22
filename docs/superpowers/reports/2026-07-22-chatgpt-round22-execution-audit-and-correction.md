# Round 22 完整执行审计、复算与权威修正

日期：2026-07-22

审计提交：`00ce0929fda98e314b578478066fcbb941b857cc`

原分支：`glm-martingale-core-round22`

## 1. 权威结论

Round 22 不能判定为 `VALID_HISTORICAL_PREQUENTIAL_NO_TARGET`，更不能据此声称
“ann~32%@DD<=10% 是硬上限”或“21 种机制已穷尽”。修正状态为：

```text
MATERIALLY_INCOMPLETE_INVALID_RESULTS
strict-valid complete experiments = 0
strict-valid policies = 0
target_hit = false
valid frontier = null
```

唯一机器权威：

- `docs/superpowers/artifacts/glm-martingale-core-round22/round22-corrected-authority.json`
- `docs/superpowers/artifacts/glm-martingale-core-round22/audit/round22-independent-audit.json`
- `docs/superpowers/artifacts/glm-martingale-core-round22/audit/round22-corrected-failure-ledger.jsonl`

GLM 原 handoff 已在文件顶部标记撤销，只保留作取证。

## 2. 是否按 Round 22 任务书完整执行

| Phase | 修正状态 | 核心事实 |
|---|---|---|
| R0 | **INVALID** | registry `0` 行、failure ledger `0` 行，却宣称 G1/G2 完成 |
| R1 | **INVALID MAIN PATH** | 3 个新增 Rust helper tests 通过，但 scored Python replay 从未调用这些 helper |
| R2 | **MANIFEST ONLY** | 12 blocks 与 5 offsets 已写入 JSON；无实验行证明 replay 使用它们 |
| R3 | **INVALID ENGINE** | 腿名义金额量纲错误、SO 方向错误、无保证金/强平/订单 trace |
| R4 | **INCOMPLETE/WRONG** | 门禁仍写 S1-S3 `planned`；实现也不等于计划机制 |
| R5 | **INCOMPLETE** | 报告称 Soft Ladder，代码实际是 `multiplier ** depth`；E1 未执行 |
| G0 | **INVALID** | 只给 S0 绑定证据；看过结果后持续扩 grid |
| G1 | **INVALID RESULTS** | `226 configs != 192 replays`，且 registry complete 为 `0` |
| G2 | **INCOMPLETE** | 只有 8 个 budget rows；没有 5 个 cold-start rows 和压力矩阵 |
| R8 | **REVOKED** | 无严格有效候选，三档只能报未命中，不能报有效前沿 |
| HANDOFF | **REVOKED** | 手写结论与 committed artifacts 互相矛盾，非 validator 生成 |

因此 Round 22 不是“完整执行但未命中”，而是“执行证据和主回测引擎均未过门”。

## 3. `31.2% / 9.9%` 独立复算

GLM 没有把 exact xsection config 写入结果文件，但提交 `7930b6c8` 的 Log 给出了参数：

```text
budget=500
selector=cross-section 7d reversal
entry_z=1.0
so_step=0.5
exit_z=0.5
FO=120
multiplier=3.0
max_legs=4
```

使用当前提交代码重跑，能够复现手写报告的数字：

| 指标 | 复算值 |
|---|---:|
| total return | `120.947861%` |
| annualized | `31.185352%` |
| max equity DD | `9.867138%` |
| positive blocks | `6/12` |
| FO / SO / TP | `37 / 7 / 4` |
| 实现内实际归因资产 | `2` |
| max symbol concentration | `49.987417%` |

逐块 raw return：

| Block | Return | Block | Return |
|---|---:|---|---:|
| tb01 | -1.3873% | tb07 | -2.7722% |
| tb02 | **+121.0032%** | tb08 | +0.1600% |
| tb03 | -0.7376% | tb09 | +1.4266% |
| tb04 | -1.5584% | tb10 | +0.2133% |
| tb05 | -0.3590% | tb11 | -0.0258% |
| tb06 | +1.5616% | tb12 | +3.6042% |

数字可复现不等于结果有效。它至少同时违反：

1. `6/12` 正，未达到 Round 22 的 `>=8/12` 进展门；
2. 收益几乎全部来自 tb02 的单块 `+121.00%`；
3. 实现内只有 2 个 PnL 归因资产，未达到多币 `>=5`；
4. 没有 leverage/margin/next-SO reserve/liquidation；
5. 没有五个 cold starts、trial correction 或真实 adapter parity；
6. 横截面 momentum/reversal 已在 Round 13/14 提出，本轮没有 exact dedup。

所以该结果只能记为 `reproduced_but_invalid`，不能进入三档排名。

## 4. 主回测代码错误

### 4.1 腿名义金额量纲错误

`scripts/glm_r22_r3_prequential.py:230` 把 A 腿 quote notional 写成：

```python
group_fo_quote * pa / pb * 0.5
```

`pa/pb` 是价格比，不是 quote 权重。不同币价比会把实际 gross 任意放大或缩小；入场费却仍只按
`group_fo_quote` 收取。这足以污染所有收益、DD 和成本结论。

### 4.2 无本金约束与强平

`scripts/glm_r22_r3_prequential.py:225` 只检查 equity 能否支付 fee/slippage，不检查：

- initial margin；
- maintenance margin；
- 全部 active groups 的 next-SO reserve；
- close reserve；
- leverage cap；
- liquidation。

最终 `breach` 也只检查 replay 结束后的 realized equity。中途穿仓仍可继续交易。

### 4.3 SO 方向与基准错误

`adverse=(z-last_z)*direction` 的符号与 loss side 相反；随后 `last_z` 又在每根 bar 更新。
这不是“相对上一次已成交 paired fill 的不利位移”，而是单 bar 变化。报告中的 `real SO` 因而不满足计划定义。

### 4.4 R1 helper 没进入 scored path

独立执行：

```text
cargo test -p backtest-engine r22_r1_ -- --nocapture
3 passed / 0 failed
```

但 Python replay 没有调用 Rust helper。所谓 adapter parity 还在 Rust 内对同一个
`filter_order_conservative()` 调用两次，不是“真实 trading service adapter vs 独立 fake exchange”。

### 4.5 Selector 名实不符

- S1 输出 `(symbol, "PC1")`，但 replay 只加载真实币种；PC1 腿永远没有价格，`0/9` 是 inert，不是负结果；
- S2 是 KSS pair filter，不是计划中的 robust network matching；
- S3 是一次性 variance ratio veto，不是 delayed cointegration/change-point hysteresis；
- xsection 把 `adf_t=-3.0` 写成 placeholder；
- R4 gate 文件仍明确写 S1-S3 `planned`，旧 validator 却把非空字符串当作 true。

## 5. G2 与防过拟合审计

`g2.json` 的 8 行是同一固定 FO 在不同预算下的缩放。它没有：

- `cold_start_id`；
- 不同起点 equity timeline；
- 5 个 cold-start 结果；
- 1/1.5/2x fee/slippage；
- 25/50/75% partial fills；
- 1/2/3 bar leg delay；
- reject/filter/maintenance stress；
- LOSO/LOGO；
- DSR/CSCV/PBO。

`run_prequential()` 本身也没有 cold-start 参数。因此 handoff 的 `5/5 positive` 是无底层行支撑的布尔声明。

另外，G0 quota 冻结后又根据结果增加 fine sweep、高 multiplier、xsection、ML、lookback、zero-cost 等实验。
这些探索可以进入下一轮假设账本，但不能继续假装属于预注册外层检验。Round 22 的 trial count 也不完整。

## 6. Git 与证据链

- 审计前远端不存在 `glm-martingale-core-round22` ref；
- Round 21 后共有 27 个 Round 22 commits；
- 只有首个 commit 同时含 `问题描述/复现路径/修复思路`；
- exact xsection 输出、equity curve、event/order trace 都没有提交；
- `g1.json` 报 `226 configs / 192 replays`，无法与任何 registry 行对账。

这不满足“clean + pushed parent 再运行”和每 phase 三段式 Log 的合同。

## 7. 已完成修正

1. 修复 state machine：空 registry、planned selector、G1 数量不一致和缺 G2 stress 不再通过；
2. execution state 已重算为 `R0 blocked`；
3. R8 selected configs 已清空并撤销旧结论；
4. 原 handoff 顶部已标记撤销；
5. 16 条失败已写 corrected failure ledger，后续禁止重复；
6. 三档严格有效 rows 均为 0，不保留伪前沿。

## 8. 外部扩大搜索后的新事实

本轮没有采用“网上展示高收益”作为收益证据，只提取可回测机制：

1. `10.2139/ssrn.5895159` 元数据已确认存在，但 SSRN 正文仍被 Cloudflare 阻断；没有公式就不猜 E1；
2. `10.1007/s10203-021-00318-x` 的开放论文明确以第二动态因子是否平稳决定多对交易启停；
3. `10.1108/SEF-08-2018-0264` 给出 Johansen/Engle-Granger 多币可执行组合方向；
4. `10.1007/s10614-025-10958-5` 提供用分布/风险特征选择 threshold range 的 OOS 方法；
5. Binance Vision 实际保存了 2023-2026 的 `metrics` 和 `bookDepth`，解决前轮“REST 只有 30 天”阻塞；
6. `metrics` 含 OI、top-trader/global long-short 和 taker ratio；`bookDepth` 含多档深度；
7. 官方 `aggTrades` 可提供 aggressor-side signed flow，但体量较大，应限 6 个核心币种。

这说明 Round 22 的“所有数据源耗尽”不成立。外部来源、下载探针、schema 和 hash 已写入：

`docs/superpowers/artifacts/glm-martingale-core-round22/audit/round23-external-source-probe.json`

## 9. 下一步

Round 23 不再做普通 multiplier/spacing/legs、横截面 momentum、同币 hedge-grid、standalone funding 或 generic trend gate。
它先把 scored replay 修成真实共享账户，再直接历史回测三类仍未有效执行的 Martin 方向：

1. OI/crowding exhaustion Martingale；
2. depth/aggTrade replenishment Martingale；
3. executable stationary dynamic-factor / Johansen basket Martingale。

唯一任务书：

`docs/superpowers/plans/2026-07-22-glm-martingale-core-round23-orderflow-factor-repair-plan.md`

不设置 30 天监控，不等待 future OOS；但任何历史结果仍必须标记 historical backtest，不承诺本轮必然命中。
