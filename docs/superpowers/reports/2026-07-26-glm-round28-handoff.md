# GLM Martingale Core Round 28 交付报告

## 机器结论

`VALID_FRONTIER_PROGRESS_NO_TARGET`；`strict_valid_candidates=0`；`target_hit=false`。Round 27 修正状态为 `MATERIALLY_INCOMPLETE_INVALID_G2_AND_FAMILY_CLOSURE`。

## Round 27 七项 Recovery

方向、beta sizing、开放 censor、chronological concurrent scheduler、calendar blocks、完整 trace、独立 validator/G3 均已修复。原始证明位于 `gates/r0-recovery.json`、`signal-intent-manifests/`、raw account/risk traces、`account-independent-validator.json` 和 `validator-mutants.json`。

- Recovery status: `"PASS"`；C0 signal manifests: `4`。
- Model validator: passed=`true`，Copula checked=`103`，threshold checked=`312`。
- Account validator: passed=`true`，policies=`284`，mutants all rejected=`true`。

## C0/T1 Baseline 全表

| policy | ann% | DD% | blocks+ | FO/SO/TP/abort | rejects | max groups | cost ratio | P-B |
|---|---:|---:|---:|---:|---:|---:|---:|---|
| R28-C0-1h-Beta-A0.10-SO0.50-R28-CORRECTED-DIRECTION-CAUSAL-CENSORED-CONCURRENT-CALENDAR12-Beta | 0.6305 | 1.2653 | 6/12 | 55/0/31/24 | 31 | 3 | 0.0384 | false |
| R28-C0-1h-Beta-A0.10-SO0.75-R28-CORRECTED-DIRECTION-CAUSAL-CENSORED-CONCURRENT-CALENDAR12-Beta | 0.6305 | 1.2653 | 6/12 | 55/0/31/24 | 31 | 3 | 0.0384 | false |
| R28-C0-1h-Beta-A0.20-SO0.50-R28-CORRECTED-DIRECTION-CAUSAL-CENSORED-CONCURRENT-CALENDAR12-Beta | 0.5197 | 1.4532 | 7/12 | 75/0/50/25 | 41 | 3 | 0.0441 | false |
| R28-C0-1h-Beta-A0.20-SO0.75-R28-CORRECTED-DIRECTION-CAUSAL-CENSORED-CONCURRENT-CALENDAR12-Beta | 0.5197 | 1.4532 | 7/12 | 75/0/50/25 | 41 | 3 | 0.0441 | false |
| R28-C0-1h-Equal-A0.10-SO0.50-R28-CORRECTED-DIRECTION-CAUSAL-CENSORED-CONCURRENT-CALENDAR12-Equal | 0.6636 | 1.2577 | 8/12 | 58/0/33/25 | 29 | 3 | 0.0402 | false |
| R28-C0-1h-Equal-A0.10-SO0.75-R28-CORRECTED-DIRECTION-CAUSAL-CENSORED-CONCURRENT-CALENDAR12-Equal | 0.6750 | 1.2573 | 9/12 | 58/0/33/25 | 29 | 3 | 0.0402 | false |
| R28-C0-1h-Equal-A0.20-SO0.50-R28-CORRECTED-DIRECTION-CAUSAL-CENSORED-CONCURRENT-CALENDAR12-Equal | 0.1856 | 1.7059 | 7/12 | 75/1/47/28 | 42 | 3 | 0.0449 | false |
| R28-C0-1h-Equal-A0.20-SO0.75-R28-CORRECTED-DIRECTION-CAUSAL-CENSORED-CONCURRENT-CALENDAR12-Equal | 0.2421 | 1.7131 | 6/12 | 75/0/47/28 | 42 | 3 | 0.0441 | false |
| R28-C0-5m-Beta-A0.10-SO0.50-R28-CORRECTED-DIRECTION-CAUSAL-CENSORED-CONCURRENT-CALENDAR12-Beta | 0.3492 | 0.9242 | 8/12 | 83/1/58/25 | 57 | 3 | 0.0571 | false |
| R28-C0-5m-Beta-A0.10-SO0.75-R28-CORRECTED-DIRECTION-CAUSAL-CENSORED-CONCURRENT-CALENDAR12-Beta | 0.3491 | 0.9242 | 8/12 | 83/0/58/25 | 57 | 3 | 0.0574 | false |
| R28-C0-5m-Beta-A0.20-SO0.50-R28-CORRECTED-DIRECTION-CAUSAL-CENSORED-CONCURRENT-CALENDAR12-Beta | -0.1238 | 0.9560 | 4/12 | 108/0/80/28 | 71 | 3 | 0.0682 | false |
| R28-C0-5m-Beta-A0.20-SO0.75-R28-CORRECTED-DIRECTION-CAUSAL-CENSORED-CONCURRENT-CALENDAR12-Beta | -0.1238 | 0.9560 | 4/12 | 108/0/80/28 | 71 | 3 | 0.0682 | false |
| R28-C0-5m-Equal-A0.10-SO0.50-R28-CORRECTED-DIRECTION-CAUSAL-CENSORED-CONCURRENT-CALENDAR12-Equal | 0.6795 | 1.2212 | 9/12 | 76/4/52/24 | 64 | 3 | 0.0504 | true |
| R28-C0-5m-Equal-A0.10-SO0.75-R28-CORRECTED-DIRECTION-CAUSAL-CENSORED-CONCURRENT-CALENDAR12-Equal | 0.5980 | 1.2240 | 9/12 | 76/3/52/24 | 64 | 3 | 0.0501 | true |
| R28-C0-5m-Equal-A0.20-SO0.50-R28-CORRECTED-DIRECTION-CAUSAL-CENSORED-CONCURRENT-CALENDAR12-Equal | 0.1748 | 1.3558 | 7/12 | 91/3/64/27 | 88 | 3 | 0.0583 | false |
| R28-C0-5m-Equal-A0.20-SO0.75-R28-CORRECTED-DIRECTION-CAUSAL-CENSORED-CONCURRENT-CALENDAR12-Equal | 0.1645 | 1.2509 | 7/12 | 91/1/64/27 | 88 | 3 | 0.0578 | false |

每个 policy 的 12 blocks、symbols/pairs、direction/weight、concentration、eligible/executed/reject denominator 与 trace hashes 位于 `gates/g2-replay.json` 和 `replay-results/`。

## TAR/MTAR Activation

- "TAR": snapshots=152/152, pair denominator=18342, finite=17000, BH=30, matched rolls=2, signals=7, first_failed_gate=`"rolls_with_exact_pair"`，passed=false。
- "MTAR": snapshots=152/152, pair denominator=18342, finite=17000, BH=43, matched rolls=2, signals=4, first_failed_gate=`"rolls_with_exact_pair"`，passed=false。

## Scheduler、G3 与 Ensemble

eligible intents=2084，executed=1242，overlap=1562；每类 reject 见全表 JSON。G3 status=`"TERMINAL"`，matrix rows=240，stress rows=28。Cross-family ensemble applicable=`false`；不存在时原因来自冻结 P-B family gate。

## Failure Closure 与 Git

Round 27 八条错误 G2 只保留为 `invalid_implementation_evidence`。Round 28 exact failures 和 never-repeat reason 位于 `round28-failure-ledger.jsonl`；不关闭所有 Martin 或 threshold cointegration。

Source/validator commit: `d516d40dafbac63d0c9d607224cae8de9721a2ce`。Final evidence commit 为包含本报告的 docs commit；push 在全部本地证据提交后单次执行。Raw root: `artifacts-local/round28/d516d40dafbac63d0c9d607224cae8de9721a2ce`。

## 十项交接核对

1. **Round 27 七项阻断**：Copula 方向、beta sizing、future-free censor、chronological concurrent scheduler、12 calendar blocks、完整 trace、独立 validator/真实 G3 均已修复；测试见 `r24-engine` 23/23、`r24-research` 62/62 和 9/9 mutants，raw proof 位于本报告所列 raw root，compact proof 位于 `gates/r0-recovery.json`、三个 real canaries 和双 validator。
2. **C0 16-policy 全表**：上表逐项列出 ann、DD、positive blocks、FO/SO/TP/abort、reject、max groups、cost ratio 与 P-B；每个 policy 的 12 block PnL、concentration、完整 reject map 和 hash 在 `replay-results/` 与 `gates/g2-replay.json`。
3. **并发 scheduler**：16-policy 合计 eligible=2084、executed=1242、overlap=1562；reject 合计为 max-active=36、paired-fill=208、same-pair=482、symbol-conflict=116、SO-paired-fill=4，max active groups=3。逐 policy denominator 和 scheduler trace 见 G2 gate/raw traces。
4. **TAR/MTAR**：TAR 与 MTAR 均 152/152 terminal，pair denominator 各 18342，finite 各 17000，BH 分别 30/43，exact matched rolls 均 2，signals 分别 7/4；首次失败门均为 `rolls_with_exact_pair`，所以没有 T1 G2 terminal。
5. **P-B/P-C/target candidates**：仅两个 P-B，均为 C0-5m-EQUAL-A0.10，SO 分别 0.50/0.75；各实际覆盖 22 alts、33 pairs，完整 symbols/pairs 数组在对应 `replay-results/`。方向严格使用 Table 4，EQUAL weights=0.5/0.5，G2 leverage=2x，Martin layers=`[1.0,1.25,1.55,1.90]`；P-C 与 target candidates 均为空。
6. **G3**：两个 P-B 均真实执行 conservative/balanced/aggressive、8 budgets、5 cold starts，共 240 terminals；14 种固定 stress 各 2 terminals，共 28，全部 final state reconciled。48 个 budget summaries、每个 cold-start 行、每项 stress、anti-overfit 和 executable principal 均在 `gates/g3-tiers.json`；两者 theoretical minimum executable principal 均为 200U，固定网格无通过 tier，target claims 为空。
7. **失败 closure**：`round28-failure-ledger.jsonl` 共 24 条，其中 Round 27 invalid evidence 8 条、Round 28 valid failures 16 条；每条均含 exact fingerprint、首失败门、numerator/denominator、hash、causal reason 和 never-repeat rule，不作跨 family closure。
8. **Cross-family ensemble**：不存在适用的 chronological shared-account cross-family ensemble，因为 TAR/MTAR 均未通过 G1 activation；未拼接 finished curves，ensemble terminals=0。
9. **Hashes/Git/push**：source/validator commit=`d516d40dafbac63d0c9d607224cae8de9721a2ce`，source tree=`998fa2b9745ee9c0d79118daaf92c3d21e5b6f8e`，data hash=`36ec45fed5d048a783e16ecc0f90ea46e0ffedededf157d25f127edf7d19baa6`，policy hash=`0898a8d7bba0e0369e09b92cd9e7b589eda7519367add0666b391cbe23d6f486`；trace hashes 在 16 个 trace manifests，validator hashes 在 authority。Final evidence commit 为包含本报告与 compact artifacts 的 docs commit；本报告落盘时 `push_status=PUSH_PENDING_LOCAL_COMPLETE`，任务书规定 evidence commit 后单次 push。
10. **最终机器门**：`strict_valid_candidates=0`；`target_hit=false`；authority status=`VALID_FRONTIER_PROGRESS_NO_TARGET`，不得解释为保守、平衡或激进目标命中。
