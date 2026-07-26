# GLM Martingale Core Round 26 交付报告

## 结论

`VALID_ACTIVATION_NO_REPLAY_CANDIDATE`。Round 25R 的无效统计结果未被继承；Round 26 严格使用 weekly Top-20、statsmodels snapshots、真实成本门与 exact matching。

## 有效候选

Activity survivors: `0`。P-B survivors: `0`。无有效候选时不展示收益 headline。

## 验证

- Model independent validator: `true`，snapshots `608`，legs `9570`。
- Account independent validator: `true`，G0 terminals `8`。
- BTC orders/trades/PnL: `0/0/0`。
- Source commit: `7a8e792696d955556a2725daf3d099c041c6397c`。

## 边界

Weekly anchors 采用任务书明确的 `152/152` gate：`[2023-07-01, 2026-05-30)`，最后 anchor 为 `2026-05-23`。
