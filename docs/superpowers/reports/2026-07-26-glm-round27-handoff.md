# GLM Martingale Core Round 27 交付报告

## 结论

机器权威状态：`VALID_HISTORICAL_PREQUENTIAL_NO_PB`。三个预注册 arm 均完成 152/152 weekly snapshots、return-blind activation 和各自 terminal；双 validator 均通过，但 8 个 G2 policy 全部 outer return 为负，故 0 P-B、0 P-C、未命中三档目标。

## 执行与首次失败门

- R0：精确复现 Round 26 的 `608 snapshots / 9570 legs`，PASS。
- C0：1h/5m 21d RAW 均通过 activity gate，分别有 `108/180` crossings，并晋级 G2。1h/5m FDR 首次失败于 `rolls_with_exact_pair >=12`，实际 `7/9`。
- C1：14d E0/E1、21d E0/E1 均完整执行；首次失败于 `candidate_fo_crossings >=100`，实际依次 `46/28/33/26`。
- P1：三组件 PBD 完整执行，`123` crossings；首次失败于 `rolls_with_exact_pair >=12`，实际 `11`，且仅覆盖 `5/12` outer blocks。
- G0：9 configs 均有 return-blind fitted-pair terminal；8 个真实 signal/order-hash canary，另 1 个显式 no-signal；BTC orders/trades/positions/PnL 均为 0。
- G1：9/9 configs terminal；survivors 为 `C0-5m-21d-RAW`、`C0-1h-21d-RAW`。
- G2：8/8 P-A，0 P-B。G3 为 `NOT_APPLICABLE_NO_PB`。

## 双 Validator

- Model validator：PASS，独立读取 raw DB；`7166` legs、`243` Copula pairs、`16` PBD pairs、0 violations，最大数值误差 `2.274e-13`。
- Account validator：PASS，8 个 policy 各有精确 `1,531,680` logical 1m risk rows，0 violations；所有终态 positions/groups/pending/reserve 为 0。
- Source/validator commit：`aaa02fda4e588ced0359318465ee682c677ae00c`；source tree：`42c605916e4419e4cd556125e82dc0609205f95f`。

## G2 Policy 结果

`conc` 顺序为 symbol/pair/group/block；`cost` 为 all-in cost / gross positive PnL。所有百分比保留两位小数。

| policy | ann | DD | positive blocks | cost | conc | alts/pairs | FO/SO/TP/abort |
|---|---:|---:|---:|---:|---|---:|---:|
| C0-5m-21d-RAW-A0.10-SO0.50 | -0.75% | 2.55% | 0/12 | 25.15% | 46.11/84.06/43.35/0.00% | 19/18 | 59/2/8/51 |
| C0-5m-21d-RAW-A0.10-SO0.75 | -0.64% | 2.21% | 0/12 | 27.22% | 52.49/84.06/43.35/0.00% | 19/18 | 59/0/8/51 |
| C0-5m-21d-RAW-A0.20-SO0.50 | -0.90% | 2.75% | 1/12 | 24.62% | 27.30/100.00/35.87/100.00% | 17/24 | 91/1/16/75 |
| C0-5m-21d-RAW-A0.20-SO0.75 | -0.87% | 2.66% | 1/12 | 23.72% | 27.30/100.00/35.87/100.00% | 17/24 | 91/0/16/75 |
| C0-1h-21d-RAW-A0.10-SO0.50 | -0.48% | 1.84% | 2/12 | 11.60% | 38.67/97.69/73.75/85.91% | 19/19 | 34/0/8/26 |
| C0-1h-21d-RAW-A0.10-SO0.75 | -0.48% | 1.84% | 2/12 | 11.60% | 38.67/97.69/73.75/85.91% | 19/19 | 34/0/8/26 |
| C0-1h-21d-RAW-A0.20-SO0.50 | -0.84% | 2.94% | 1/12 | 13.50% | 40.25/100.00/64.89/100.00% | 19/21 | 46/2/6/40 |
| C0-1h-21d-RAW-A0.20-SO0.75 | -0.74% | 2.66% | 1/12 | 13.43% | 37.39/100.00/64.89/100.00% | 19/21 | 46/1/6/40 |

12-block PnL，按时间顺序：

- `5m A0.10 SO0.50`: `[0,0,-4.40,0,-5.73,-1.94,-5.00,0,-19.93,-1.52,-2.16,-2.83]`
- `5m A0.10 SO0.75`: `[0,0,-4.40,0,-2.35,-1.94,-5.00,0,-16.60,-1.52,-2.16,-2.83]`
- `5m A0.20 SO0.50`: `[0,0,-10.93,-3.63,0.34,-2.90,-3.24,0,-21.05,-0.75,-2.11,-7.96]`
- `5m A0.20 SO0.75`: `[0,0,-10.93,-3.63,0.34,-2.90,-3.24,0,-19.25,-0.75,-2.11,-7.96]`
- `1h A0.10 SO0.50/0.75`: `[0,0,-6.41,0,-6.40,-1.08,-1.23,0.15,-13.61,0,0,0.93]`
- `1h A0.20 SO0.50`: `[0,0,-6.15,-2.03,-20.63,-1.15,-1.23,-0.65,-17.91,0,0,1.11]`
- `1h A0.20 SO0.75`: `[0,0,-6.15,-2.03,-14.39,-1.15,-1.23,-0.65,-17.91,0,0,0.52]`

所有 8 个 policy 首先因 `compounded_return > 0` 失败 P-B，并同时未达到 `positive blocks >=8/12`。5 个 cold starts、三档 stress 和 anti-overfit 未执行，状态均为 `not_applicable_no_pb`，不得把缺失项解释为通过。

## Budget 与目标

| tier | budgets 500/750/1000/1500/2000/3000/4000/4999U | exact minimum principal | cold starts |
|---|---|---|---|
| conservative | N/A: no P-B survivor | N/A | N/A |
| balanced | N/A: no P-B survivor | N/A | N/A |
| aggressive | N/A: no P-B survivor | N/A | N/A |

P-C progress 也不适用，因为没有正收益 P-B 候选进入 G3。

## 实际账户绑定

- Principal `2000U`，FO group gross `100U`（5%），leverage cap `2x`，最多 3 active groups；SO relative layers 固定为 `[1.00,1.25,1.55,1.90]`，实际最多触发 2 个 loss-after-add groups。
- 两腿方向由每个 frozen Copula signal 的 `direction=+1/-1` 转换为真实 paired orders；没有静态 portfolio 权重、独立 PnL sleeve 或 BTC 腿。event-level 方向、数量、价格和 group ownership 位于 raw engine traces。
- 实际交易 23 alts：AAVE, ADA, APT, AVAX, BCH, BNB, CRV, DOGE, DOT, DYDX, ETC, ETH, FIL, GALA, HBAR, INJ, LINK, NEAR, SOL, TRX, UNI, XRP, ZEC。
- 实际交易 38 distinct pairs：`AAVE-FIL, ADA-DOT, ADA-ETH, ADA-GALA, ADA-LINK, ADA-XRP, APT-DOGE, APT-DOT, AVAX-DOGE, AVAX-ETH, AVAX-NEAR, BCH-CRV, BCH-ETC, BCH-GALA, BCH-SOL, BCH-ZEC, BNB-TRX, CRV-FIL, CRV-GALA, CRV-UNI, DOGE-HBAR, DOGE-INJ, DOGE-NEAR, DOGE-XRP, DOT-DYDX, DOT-NEAR, DOT-SOL, ETC-FIL, ETC-NEAR, ETH-LINK, ETH-SOL, FIL-INJ, FIL-NEAR, GALA-LINK, HBAR-LINK, INJ-SOL, LINK-NEAR, SOL-XRP`。
- 各 policy 的完整 actual symbol/pair 集合及 trace hashes 位于 `gates/g2-replay.json`、`replay-results/` 和 `trace-manifests/`。

## Failure Closure

Activation exact failures：`C0-1h-21d-FDR`、`C0-5m-21d-FDR`、`C1-1h-14d-E0`、`C1-1h-14d-E1`、`C1-1h-21d-E0`、`C1-1h-21d-E1`、`P1-5m-7d`。Never repeat：不得在看到 terminal 后放宽冻结 activity thresholds。

G2 exact failures：上述 8 个完整 policy IDs。Never repeat：不得根据 outer return 调 entry、SO、formation、family、budget 或追加 trial。完整 gates、metrics 与 fingerprints 位于 `round27-failure-ledger.jsonl` 和 `round27-closed-fingerprints.json`。

## Ensemble 与 Git

- 不存在 shared-account multi-family ensemble：只有 C0 family 进入 G2，且没有 P-B survivor。
- Immutable source/validator commit：`aaa02fda4e588ced0359318465ee682c677ae00c`。
- Final evidence commit：本报告与 compact artifacts 所在的最终 docs commit；其 hash 以分支最终 `git log` 为准。
- Push：`PENDING_FINAL_SINGLE_PUSH`，完成最终 docs commit 后仅执行一次 `git push -u origin glm-martingale-core-round27`。
- 最终 raw root：`artifacts-local/round27/aaa02fda4e588ced0359318465ee682c677ae00c`。
- 保留的历史 roots：`739ca15bd35442093028da6c0078d9673fe054fa`、`739ca15b4d42948c900cf0b779da35af05f6e31c`、`88e3f3c2fe0a46faa4fc501a8fedeaf698a24a94`；均未删除或搬运。
