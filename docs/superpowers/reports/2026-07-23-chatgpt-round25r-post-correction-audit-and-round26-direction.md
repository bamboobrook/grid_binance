# Round 25R 后续修正独立审计与 Round 26 方向

审计日期：2026-07-23。

## 1. 权威结论

Round 25R 的账户层修正有价值，但整体结果仍不能认定为有效。权威状态改为：

```text
MATERIALLY_INCOMPLETE_INVALID_RESULTS
```

原因不是收益太低，而是入选模型的统计门实现错误。16 个 replay 已完成且账户流水可以复算，但它们只能计入
全局 trial ledger，不能称为 16 个 valid failures，不能关闭 corrected fingerprint，也不能用于判断收益上限。

可以继续推进。下一步应保留已经修好的账户执行器，替换模型统计层，并采用本地已有完整数据的 29 个 alt、
每周滚动和 14/21 天形成期。这个 fingerprint 尚未被前 25 轮有效执行。

## 2. 任务完成度

| 阶段 | 审计状态 | 说明 |
|---|---|---|
| R0 reserve ledger | `PASS` | reserve 生命周期、终态归零和 restart 测试已补齐 |
| R1 filter sizing | `PARTIAL_PASS` | 2000U 下 ETH/LINK 可成交；相邻 budgets 与 exact minimum 未跑 |
| R2 reference Copula | `FAIL_P0` | EG p-value、independent reference、matching、cost gate 均不符合任务书 |
| R3 SO/1m path | `PARTIAL_PASS` | 1m 风险路径有效；rolling stationarity 继承错误统计函数 |
| R4 validator | `PARTIAL_PASS` | 可复算 wallet/reserve/cost/DD；未复算模型统计与 selector |
| G0/G1 | `EXECUTED_INVALID` | 8 个 G0 和 16 个 G1 均结束，但上游模型门无效 |
| G2/C2 | `NOT_APPLICABLE` | 没有有效 P-B parent，未执行是正确行为 |

可保留的工程证据：

- component reserve ledger 与 final positions/groups/pending/reserve 全归零；
- 真实 tick/step/minNotional 两腿原子 sizing；
- completed signal 后下一 1m event 成交；
- 1m high/low、funding、maintenance 与 liquidation 路径；
- timestamp-aligned Copula、Gaussian/Student-t 条件概率和 frozen active model；
- raw trace manifest、SHA256、确定性样本及账户指标复算。

## 3. P0 模型错误

### 3.1 ADF/EG p-value 使用了错误分布

`crates/r24-research/src/r25_corrected.rs:392-422` 的 `adf_lag_zero()` 最终返回：

```rust
normal_cdf(statistic)
```

ADF statistic 不服从普通正态分布；Engle-Granger residual 的 p-value 还需要 MacKinnon cointegration 分布。
因此 Rust 输出的极小 p-value 不能用于 stationarity admission。

### 3.2 实际入选 fit-cache 的独立复算

使用 `statsmodels 0.14.5`，按实际 fit-cache 的精确时间边界重新读取本地数据，并执行：

```text
statsmodels.tsa.stattools.coint(trend="c", autolag="aic")
statsmodels.tsa.stattools.kpss(regression="c", nlags="auto")
AR(1) half-life
```

结果：

| selected pair | independent EG p (left/right) | KPSS p (left/right) | 独立结论 |
|---|---:|---:|---|
| DOGE-LINK | 0.27367 / 0.26164 | 0.10000 / 0.03311 | fail |
| XRP-DOGE | 0.10341 / 0.19630 | 0.10000 / 0.05640 | fail |
| BNB-SOL | 0.00533 / 0.00195 | 0.02891 / 0.10000 | fail |
| SOL-DOGE | 0.01822 / 0.01635 | 0.10000 / 0.10000 | pass |
| ETH-LINK | 0.07662 / 0.06014 | 0.02656 / 0.03854 | fail |

严格 EG+KPSS 下只有 `1/5` pair、`3/10` selected legs 通过。若只按普通 residual ADF+KPSS，仍只有
`4/10` legs 通过，且也只有 `SOL-DOGE` 两腿同时通过。

最佳 `R25-C1-11/12` 的第一组 `DOGE-LINK` 来自应被拒绝的模型，并贡献约 51.22% 的正收益。因此其
`0.069087% ann / 0.085972% DD` 也不能保留为有效结果。

### 3.3 其他 R2 不完整项

1. `independent_reference_error` 无条件写为 `0.0`，不是独立误差。
2. `maximum_weight_disjoint()` 是按 score 排序后的 greedy，不是 maximum-weight matching。
3. `cost_feasible = sigma >= 0.001`，没有比较 fee、slippage、funding、legging 和真实 quantities。
4. `tail_sample_count = values.len()/20`，不是实际可成交 tail-to-neutral 样本计数。
5. validator 只复算账户，没有验证上述统计语义。

完整失败台账：

```text
docs/superpowers/artifacts/glm-martingale-core-round25-corrected/audit/
round25r-post-correction-failure-ledger.jsonl
```

## 4. Return-blind 激活普查

所有普查只读取 formation 数据，不读取后续收益。

### 4.1 原 6 alt

季度滚动天然过于稀疏。改为每周滚动后：

| frequency/lookback | >=2 alts | >=4 alts | >=6 alts | rolls |
|---|---:|---:|---:|---:|
| 1h / 14d | 12 | 2 | 1 | 152 |
| 1h / 21d | 11 | 3 | 1 | 152 |
| 5m / 14d | 3 | 0 | 0 | 152 |
| 5m / 21d | 2 | 0 | 0 | 152 |

原 6 alt 即使周更，也不足以稳定维持多组组合。

### 4.2 扩大到 29 alt

`funding_rates.db` 已有 BTC 加 29 个 alt 的完整 funding；29 个 alt 均有
`1,795,680` 根 1m futures bar，范围为 2023-01-01 至 2026-05-31。Binance 公开 API 当前也能返回全部
30 个 symbol 的 filters，所以不需要先等待行情下载。

raw EG+KPSS+finite-half-life：

| frequency/lookback | >=2 alts | >=4 alts | >=6 alts | max alts |
|---|---:|---:|---:|---:|
| 1h / 14d | 36 | 14 | 9 | 25 |
| 1h / 21d | 31 | 13 | 9 | 27 |
| 5m / 14d | 10 | 1 | 0 | 4 |
| 5m / 21d | 5 | 1 | 1 | 10 |

同一 roll/lookback 对 29 个 EG p-value 做 Benjamini-Hochberg `q=0.05` 后：

| frequency/lookback | >=2 alts | >=4 alts | >=6 alts | max alts |
|---|---:|---:|---:|---:|
| 1h / 14d | 3 | 2 | 2 | 25 |
| 1h / 21d | 6 | 4 | 4 | 27 |
| 5m / 14d | 4 | 0 | 0 | 2 |
| 5m / 21d | 1 | 1 | 1 | 10 |

扩大 universe 明显改善 raw activation，但严格 FDR 只保留少数市场共同回归阶段。下一轮必须同时保留
预注册 raw 与 FDR control，不能看完 OOS 收益后再决定使用哪个门。1h 是主臂，5m 只在 activation gate
通过时运行完整账户。

## 5. 外部证据与边界

1. [Copula-based trading of cointegrated cryptocurrency Pairs](https://doi.org/10.1186/s40854-024-00702-7)
   使用 Binance USDT futures、BTC reference、20 币、三周 formation/一周 trading 和每周更新。论文报告
   5m EG 年化 56.7%-75.2%，但初始资金为 20,000USDT，样本只有约两年；hourly EG 最大回撤约
   36.6%-41.6%。只能借用设计，不能继承收益。
2. [Pairs Trading in Cryptocurrency Markets](https://doi.org/10.1109/access.2020.3024619)
   对 Binance 26 个流动币比较 5m/1h/daily，确认高频结果更好，但也明确结果对参数、交易成本和 execution
   window 高度敏感。
3. [Anti-Persistent Values of the Hurst Exponent Anticipate Mean Reversion](https://doi.org/10.3390/math12182911)
   支持把 local `H<0.5` 作为 formation-only 的快速均值回归 veto；不得把它变成独立收益引擎。
4. [Optimal Market-Neutral Multivariate Pair Trading](https://doi.org/10.3390/ijfs12030077)
   牛熊样本报告年化 15.49%，说明跨牛熊、多变量与高收益并不自动同时成立。

这些研究不证明用户目标可以实现。尤其保守档 `50% ann / 10% DD` 尚无有效证据；平衡
`90%/20%` 与激进 `100%/30%` 的不确定性更高。继续搜索有合理的新方向，但不能承诺命中。

## 6. 下一步

唯一任务书：

```text
docs/superpowers/plans/2026-07-23-glm-martingale-core-round26-weekly-expanded-copula-martin-plan.md
```

Round 26 先修模型统计权威，再执行 29-alt weekly activation census。只有预注册 selector 通过 activity gate，
才进入 shared-account replay；只有 P-B survivor 才跑三档 sizing、budgets、cold starts 和压力测试。

## 7. 审计产物

```text
scripts/chatgpt_r25r_stationarity_scan.py
docs/superpowers/artifacts/glm-martingale-core-round25-corrected/audit/
  round25r-selected-fit-cache-independent-audit.json
  round25r-independent-stationarity-scan.json
  round25r-weekly-stationarity-scan.json
  round25r-weekly-5m-stationarity-scan.json
  round25r-weekly-29alt-1h-fdr-stationarity-scan.json
  round25r-weekly-29alt-5m-fdr-stationarity-scan.json
  perp-exchangeInfo-30-symbols-2026-07-23.json
  round25r-post-correction-failure-ledger.jsonl
```
