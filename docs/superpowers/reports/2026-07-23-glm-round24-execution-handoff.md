> **SUPERSEDED_BY_CHATGPT_AUDIT（2026-07-23）**
>
> 本交接的 registry/trace 数量仍可作取证，但 `F1 16/16 valid failures`、exact fingerprint 已关闭及
> `BLOCKED_ENGINE_DATA_OR_EXECUTION` 完整状态已撤销。独立审计发现 current-hour look-ahead、G0 helper
> 未绑定 scored path、缺主动 next-SO reserve/filter/rounding/动态 concentration，以及单-family 100%
> 自锁门。唯一修正权威为
> `docs/superpowers/artifacts/glm-martingale-core-round24/round24-corrected-authority.json`。

# GLM Martingale Core Round 24 执行交接

| 字段 | 权威值 |
|---|---|
| audited commit / upstream / dirty | `9bb7c4a54e4f7bcb23e0557552c816caa0ffe371` / `9bb7c4a54e4f7bcb23e0557552c816caa0ffe371` / `false` |
| registry running / terminal / unique / violations | `19` / `19` / `19` / `0` |
| exact executed policies / G1 replays | `16 / 16` |
| phase complete / blocked | `R0,R1,R2(F1),R3,G0,G1,R8 complete`; `F3 data blocked`; `E1,F2,G2 not applicable` |
| strict survivors / P-C / P-D | `0 / 0 / 0` |
| best strict candidate | `null` |
| latest tier verdict | `50/10=false; 90/20=false; 100/30=false` |
| unresolved blockers | `F3 historical data incomplete; F1 16/16 failed P-B` |

## 最终状态

`BLOCKED_ENGINE_DATA_OR_EXECUTION`。本轮仅为 historical prequential backtest，未设置 30 天监控。

## 实际执行

- R0：32/32 canaries 被真实 validator 拒绝。
- R1：15/15 shared-account tests 通过，六币 probe、SQLite restart 与 independent adapters parity 通过。
- R2/R3：12 blocks、121 天 purge、5 cold starts 和 52 个含条件模板 policies 在收益前冻结；F1 perp/funding 可用，F3-M1/M2 数据不足。
- G0：8 synthetic + 4 real causal windows 完成；真实窗口为 no-fit rejection delta，不伪称交易激活。
- G1：16/16 policies 完整运行，11/12 blocks 为 no-fit 且保留在分母，P-B survivors=0。
- E1/F2/G2：parent gate 不满足，按计划不适用；未注入 finished curves，也未缩短窗口。

## 最佳失败诊断

`R24-F1-11`：ann `-2.716501%`，equity DD `7.728440%`，实际资产 `6`，SO `23`。它仍为 failed diagnostic，不是 candidate；失败原因为 `["group_positive_contribution_above_50pct","block_positive_contribution_above_50pct","family_positive_contribution_above_50pct","cost_to_gross_profit_above_50pct"]`。

全部 16 个 F1 exact fingerprints 已写入 never-repeat ledger。本结论只关闭该 fingerprint 范围，不声称穷尽所有马丁可能性。

## 证据修正

validator 将 staging 产生的 `144` 个 `/tmp` trace 路径映射为仓库相对路径；每个文件的 SHA256、字节数、JSONL 行数均与 immutable terminal row 一致，trace 内容未改变。
