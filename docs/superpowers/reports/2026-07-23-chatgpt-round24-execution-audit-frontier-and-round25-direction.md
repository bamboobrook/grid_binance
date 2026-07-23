# Round 24 独立审计、1-24 轮前沿与 Round 25 方向

日期：2026-07-23。

## 1. 权威结论

Round 24 的文件证据链完整，但 scored F1 回放语义无效。修正状态为：

```text
MATERIALLY_INCOMPLETE_INVALID_RESULTS
strict-valid candidates = 0
strict-valid 5/5 = 0
target hit = false
validly closed Round 24 F1 fingerprints = 0
```

机器权威：

- `docs/superpowers/artifacts/glm-martingale-core-round24/round24-corrected-authority.json`
- `docs/superpowers/artifacts/glm-martingale-core-round24/audit/round24-independent-audit.json`
- `docs/superpowers/artifacts/glm-martingale-core-round24/audit/round24-corrected-failure-ledger.jsonl`
- `docs/superpowers/artifacts/glm-martingale-core-round24/audit/round1-24-consolidated-frontier.json`

旧 `round24-authority.json` 和 GLM handoff 只保留作取证，已加 superseded 指针。

## 2. Round 24 做完整的部分

以下事实独立复核通过：

| 检查 | 结果 |
|---|---:|
| registry | 19 running / 19 terminal / 19 unique |
| trace refs | 152/152 SHA256、bytes、JSONL rows 一致 |
| trace parse | 152/152 可解析 |
| R0 canaries | 32/32 被 validator 拒绝 |
| tests | 24 passed / 0 failed |
| F1 process quota | 16/16 实际启动并产出 traces |
| Git | clean、upstream=HEAD、远端存在；Round 24 commits 均含问题描述/复现路径/修复思路 |

F3-M1/M2 因 metrics/depth/aggTrades 历史不完整而 fail-closed，E1/F2/G2 因无有效 parent 未启动，
这些条件跳过本身合理。

## 3. Round 24 的致命遗漏

1. `residual.rs` 把小时最后一根 1m close 标成小时起点；G1 在同 timestamp 算信号并成交，前视约 59 分钟。
2. G0 的 `allow_so()` worsening veto 没有被 G1 replay 调用；scheduler 三臂也只是手工字符串排序。
3. next-SO reserve 只在 SO 已触发时临时申请，FO 后没有持续覆盖所有 active groups 的下一层和 close。
4. scored path 没有交易所 filter、qty/price rounding、min-notional 或 per-symbol maintenance tier。
5. 单一 F1 family 的 contribution 被定义为 100%，同时 `>50%` 必失败，因此任何 F1 天生不能过 P-B。
6. 实时 35% freeze、symbol/group 25% reserved-gross cap 没实现；跨 block active groups 可与新 pair 共用 symbol。
7. order weights 固定 `0.5/-0.5`，没有把方向及可审计 beta sizing 绑定到真实 quantity。
8. cost/gross-profit 分子漏掉 close、abort、end-close、funding/borrow 和 legging 成本。
9. block DD 用全局 DD 差值代替 block running-peak DD，tb12 在 end-close 前先落盘。

因此 `R24-F1-11 -2.72% ann / 7.73% DD` 只能保留为 invalid forensic row，不能证明 causal residual
Martin 已失败，也不能写入 never-repeat exact closure。

## 4. 前 24 轮较好的组合

严格按当前共同合同，没有一个可实盘 promotion 的组合。下面是按“对下一轮的信息价值和 ann/DD 形状”
排序的研究前五，不是可部署排名：

| 排名 | 组合 | Ann / DD | 稳定性 | 本金/杠杆 | 主要构成 | 为什么仍不可用 |
|---:|---|---:|---:|---|---|---|
| 1 | R19 `M1R_F3_044` | 41.43% / 6.27% | 5/5 inner-train | 1000U / 3x | LTC-TRX、BCH-ETH、XRP-BNB | inner-fit 前视；symbol/group 贡献约 56%；执行模型不完整 |
| 2 | R9 allocator，funding 修正 | 62.78% / 18.38% | 5/5 curve slices | 未证明 / 混合 | AAVE、ANKR、BCH、BNB、DOT、SOL、TRX、XRP | finished curves；无 event-level position continuity/shared reserve |
| 3 | R7 `ANKR_q1w24p24` | 62.51% / 28.62% | 4/5 | 4999U / 10x | 多 BNB/TRX/ANKR；空 AAVE/SOL/DOT | 1000-4000U 收益塌缩；2025 负；杠杆和 DD 超门 |
| 4 | R4 corrected | 34.81% / 17.67% | 4/5 | 4999U / 10x | 多 BNB/TRX/BCH；空 AAVE/SOL/DOT | 1000/2000U 负；贡献集中；2025 负 |
| 5 | R6 `xrpq20_r460` | 33.96% / 18.03% | 4/5 | 5000U / 10x | 7 币 quarantine/R4 curve mix | 本金不小于 5000；curve blend；2025 负 |

补充事实：

- Round 14 的 `3000U 50.70%/20.29%/5-of-5` 和 `4999U 35.07%/13.56%/5-of-5`
  已因 loader/selector/budget 语义修复撤销，不能恢复排名。
- Round 22 的 `31.19%/9.87%` 只有两个归因资产，收益几乎由单块 `+121%` 构成，且 Python
  engine 无 margin/liquidation，不能算多币组合。
- Round 23 的 5/5 是单 BTC，且 funding、registry、trial correction 不合格。
- Round 24 没有新增正收益行。

完整 24 轮逐轮代表值及前五构成已写入 consolidated frontier JSON。

## 5. 外部检索的新方向

主来源为 2025 年 Financial Innovation 论文
[`10.1186/s40854-024-00702-7`](https://doi.org/10.1186/s40854-024-00702-7)，PDF SHA256 为
`61541b3b501d0491c1a635cfe5067aba0439fc6612ee0159b678d6373e5bcecb`。

论文的有效新信息不是它的收益数字，而是下面的信号结构：

```text
BTC-reference stationary spreads
  -> formation-only cointegration
  -> two spread marginals + fitted copula
  -> conditional tail probabilities
  -> relative mispricing between two altcoins
```

代码库此前只有 Round 22 taskbook 一次 `copula` 文字，没有 executable family、registry replay 或有效负结果，
所以它不是重复 fingerprint。它可只使用现有完整 perp klines/funding，不需要等待缺失的 order-flow archives。

但论文不能直接当收益证据：只覆盖 2021-2023、formation windows 高度重叠、阈值在同历史比较、每侧约
20000U、没有 funding/margin/liquidation/partial fill。其最佳 5m EG 行虽报 ann 75.2%，equity DD 仍是
30.5%；KSS 行 DD 超过 160%。这些数值和参数一律不导入本地 promotion。

第二来源 [`10.1214/21-AOAS1568`](https://doi.org/10.1214/21-AOAS1568) 证明 crypto 上下尾依赖可不对称，
Round 25 只把它转成 train-only SO veto。Hawkes/VPIN 仍与 F3 重叠且数据不全；dynamic factor/VECM/Johansen
已在 Round 18-23 出现，均不再换名重跑。

完整来源、hash、采用/拒绝边界：

`docs/superpowers/artifacts/glm-martingale-core-round24/audit/round25-external-source-update.json`

## 6. 下一步

Round 25 不再扩大普通 ladder grid。唯一主线是：

```text
先修 Round 24 scored engine
  -> 三组 disjoint BTC-reference conditional-copula alt pairs
  -> shared-account Soft Martin FO/SO/TP/abort
  -> asymmetric-tail 只在 parent 通过后 veto SO
  -> 12-block prequential + 5 cold starts + budgets/stresses
```

唯一任务书：

`docs/superpowers/plans/2026-07-23-glm-martingale-core-round25-reference-copula-recovery-plan.md`
