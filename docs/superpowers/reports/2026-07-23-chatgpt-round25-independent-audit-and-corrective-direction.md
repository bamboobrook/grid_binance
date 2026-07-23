# Round 25 独立审计与修正方向

审计日期：2026-07-23。

## 1. 结论

Round 25 recovery 不能认定为 `VALID_HISTORICAL_PREQUENTIAL_NO_TARGET`，正确状态是：

```text
MATERIALLY_INCOMPLETE_INVALID_RESULTS
```

现有 `0.12%–0.40%` 年化和 `0.65%–0.96%` 回撤只能作为失效执行器的原始输出，不能用于判断
Copula Martin 的收益上限，也不能关闭 C1 fingerprint。Round 25 必须执行 corrected replay，暂不编号 Round 26。

## 2. 原始结果前五（全部无效）

| policy | ann | equity DD | final equity/1000U | actual assets | SO fills |
|---|---:|---:|---:|---:|---:|
| R25-C1-03 | 0.3985% | 0.8053% | 1011.6838 | 4 | 2 |
| R25-C1-04 | 0.3985% | 0.8053% | 1011.6838 | 4 | 2 |
| R25-C1-12 | 0.3700% | 0.8209% | 1010.8440 | 4 | 0 |
| R25-C1-15 | 0.2168% | 0.6458% | 1006.3456 | 4 | 0 |
| R25-C1-16 | 0.2168% | 0.6458% | 1006.3456 | 4 | 0 |

16/16 原始 final equity 为正，但没有一个满足六币，也没有任何有效 5-cold-start、budget、stress 或
anti-overfit 结论。因此不能称为“16 个有效失败”，更不能称为目标已证伪。

## 3. P0：资金预留泄漏锁死账户

`SharedAccount::remove_group()` 删除 group 时不释放 `group.reserved_next_so`，而 production close path 只调用
`remove_group()`。成功 FO 的 reserve 因此永久留在 `reserved_quote`。

独立复算 `R25-C1-03` margin trace：

```text
reserve accepted:   365,617 rows / 4,813,347.805011773 U
reserve consumed:   365,541 rows / 4,812,347.265011770 U
unreleased:              76 rows /     1,000.540000003 U
```

76 正好对应成功进入并最终关闭的 Martin groups。1000U 账户被幽灵 reserve 基本锁满后，后续信号只能拒单。
同一 trace 中出现 `130,849` 次 `shared_reserve_unavailable`。因此交易次数、币种覆盖、收益和回撤都被错误压低。

## 4. P0：六币门在订单层结构性不可能

冻结 group FO gross 为 20U，equal-dollar 两腿各 10U。当前 filter snapshot：

```text
ETHUSDT minNotional = 20U
LINKUSDT minNotional = 20U
```

ETH/LINK 每腿 10U 必然 atomic reject；其余可交易 alts 只有 BNB/SOL/XRP/DOGE，所以 16/16 恰好都只成交
4 币。任务书同时要求六币，形成不可达 gate。`R25-C1-03` 有 `366,018` 次
`min_notional_or_pair_gross_mismatch`，不是 alpha 失效。

corrected baseline 必须用 filter-feasible sizing。预注册修正为 2000U calibration principal、50U group FO，
layer `[50,62.5,77.5,95]U`；每腿仍须按真实 tick/step 解析，resolved gross 不达 minNotional 则拒单。

## 5. P0：Copula 与 SO 不是任务书定义的模型

1. `spread_corr()` 对两组各自排序后的 spreads 按 rank 下标计算相关，而不是按 timestamp 对齐；这会把边际
   分布形状误当成联合依赖。
2. production 只使用 Gaussian conditional CDF；没有 train AIC 的 Gaussian/Student-t 选择。
3. 没有 EG-ADF、KPSS、half-life、break frequency、tail count 和 cost feasibility gate。
4. active SO 的 `pair_h_values()` 把 rho 固定为 0.0，没有使用 open-time frozen copula。
5. `current_one_bar_adverse_increment_worsening=false` 与 `rolling_stationarity_valid=true` 被写死。

所以即便修复 reserve，当前信号也不能直接称为有效 Copula Martin。

## 6. P1：完整性与实盘门未执行

- `positive_blocks` 固定为 0，P-B 没有执行 `>=8/12` 正收益门；
- 未计算 symbol/group/block positive contribution、dynamic 35% freeze 和 all-in cost/gross profit；
- 结果文件缺 compounded return、balance DD、block returns、cost components、concentration 和 final reserve；
- account 只在 5m/1h signal event mark，未用完整 1m 路径检查区间内 maintenance/liquidation；
- final close error 被忽略；没有强制 positions/groups/reserved_quote 最终归零；
- activation census 对 5m policy 只跑 20,000 steps，并非完整 12 blocks；
- registry 的 `wall_seconds=1`、`peak_rss_kb=1` 是常量；
- final validator 只检查 G0 replay count、16 policy count 和 registry 行数，没有从 traces 复算账户；
- G2 因错误的 P-B=0 未运行，三档、8 budgets、5 cold starts 和实盘压力均未评价。

## 7. 哪些证据仍可保留

- BTC 仅作 reference，现有 summaries 中 BTC order/trade count 为 0；
- 六个 alt 的 1m/funding/filter data gate 可用，LTC funding 缺失被正确标记不可交易；
- completed-bar 到下一 1m open 的基本时间边界已进入 production path；
- 16 个预注册 policy 和原始 traces 保留为 invalid trial，计入 global trial ledger；
- `r24-research` 11 tests 与 `r24-engine` 15 tests 通过，但测试集没有覆盖上述 reserve 生命周期错误。

## 8. 是否还有提升空间

有，但现在不能量化。

1. reserve 泄漏修复会恢复资本循环，交易数量、六币覆盖、收益和回撤都会同时上升；不能线性外推。
2. filter-feasible 50U FO 让 ETH/LINK 真正进入组合，并把 2000U baseline 的风险维持在小资金范围。
3. 正确的 timestamp-aligned Copula、stationarity 和 tail model 会重排 pair 与信号，结果可能变好也可能变差。
4. deficit scheduler、neutral-reset 后再入场和 C2 asymmetric-tail veto 有机会分别改善多币分散、成本和尾部回撤，
   但必须在 corrected C1 P-B 后条件执行。

当前没有证据支持 50%/90%/100% 已可实现，也没有证据证明不可实现。保守档仍可继续验证；平衡与激进档
在没有 corrected P-B 前属于高不确定目标，不能靠加杠杆伪造。

## 9. 下一步

唯一任务书：

`docs/superpowers/plans/2026-07-23-glm-martingale-core-round25r-corrected-copula-replay-plan.md`

先修执行与模型，再原样重跑 16 policies。禁止基于本轮无效 headline 扩 alpha、TP、multiplier 或新增 family。

## 10. Git 交付问题

GLM 的未推送 evidence commit 包含多份 225–270MB 单文件 trace，超过 GitHub 100MB 上限。corrected run 的
raw traces 必须保留在本地可重建目录，仓库只提交 manifest、SHA256、指标、确定性抽样和必要小型 canary traces。
当前 oversized commit 不应作为下一轮远端基线。
