# GLM Martingale Round 12 Search Ledger（审计修正版）

本文件保留 Round12 的全部实际探索，但以 2026-07-12 独立审计结论为准。原 handoff 中
“P0-P10 全部闭合”“dynamic allocator failed”“cycle-depth TP failed”“R4 fully live-ready”
等表述已撤销。

## Canonical Carry-In

- Round1-11：`docs/superpowers/artifacts/glm-martingale-core-round11/r1-r11-corrected-status.json`
- R7 corrected event-level：`62.3718 / DD28.6873`；2000U 为 `5.4232 / DD45.5823`。
- R9 corrected curve diagnostic：`62.7845 / DD18.3840`，不可执行、不可 promotion。
- Round12 三档 target hits：0。

## P0：Registry / Branch

- 状态：`partial`。
- 目录和 branch 已创建。
- 原 registry 第 11-13 行缺 14-15 个 schema 必填字段；多条 effective hash 是标签而非
  SHA256，也没有按“先登记、后执行”提供证据。
- 审计追加完整 correction records；Round13 必须重新采用严格 schema/checkpoint。

## P1：Frozen Data / Funding

### 原执行问题

- manifest 未过滤 `market_type/timeframe`，把 spot/futures 视作同 timestamp duplicates；
- 117GB DB 的所谓 SHA256 实际是固定字符串 `large` 的 hash；
- 未计算 missing minutes、funding gaps、premium coverage 或 HTTP provenance；
- Rust tests 多数只模拟 Vec/JSON，没有调用真实 loader；
- loader 对 missing/incomplete funding 静默返回空/部分数据。

### 审计修复

- 15 symbols development 均为 `1,795,680/1,795,680` futures 1m、0 duplicate、0 missing；
- 15 symbols holdout 均为 `57,600/57,600`；
- 从 Binance official endpoints 修复 holdout 36,351 根 K 线缺口；R4 六币为 13,563 根；
- funding edge/internal >9h gap、text mark、duplicates 均进入 gate；
- loader missing/range/gap tests 真实调用 SQLite；
- development/holdout gate 均 PASS。

限制：原 development 下载时没有保存 HTTP response hash，无法事后重建该 provenance。

## P2：Promotion Validator

- 原状态：错误标记 `complete`。
- 实际完成：shared-account static replay、5 cold starts、budget ladder。
- 未完成：train-fold 选参 WFO、neighbor、LOSO/PnL share、真实 fee/slippage/funding/latency
  stress、holdout contract、production trace。
- 原脚本把固定 config segment 重放叫 WFO，并把 4 次 base replay 叫 cost stress。
- 审计修复：schema v2 fail-closed，`promotion_target_hits=[]`、`fully_live_ready=false`，
  candidate symbols 必须存在于 manifest。

## P3：R9 Static Merge Diagnostic

- 实际机制：5 source configs 的 36 strategies 全部同时 active，共享 4999U；无 shadow/live、
  无 active sleeve 切换、无 restart state。
- Symbols：AAVE/ANKR/BCH/BNB/DOT/SOL/TRX/XRP。
- Full：ann `-7.5209%`、DD `56.0007%`、return `-23.4420%`、4/5 positive。
- 可用结论：该 exact static merge 失败。
- 不可用结论：R9 dynamic allocator family failed。
- Non-repeat：`r12-r9-static-36-strategy-merge`，仅限同一 5 configs、36 simultaneously-active
  strategies、1000/2000/3000/4000/4999U。

## R4 Corrected Baseline

- Final binary：`afb70e9aedba2cd974733206fb468c9a27e8f1943d81a28904933c3dafc0a755`。
- 4999U full：ann `34.7284%`、DD `17.6868%`、return `176.8781%`、4/5 positive。
- 1000/2000U ann：`-15.7285% / -7.0614%`。
- 3000/4000U ann/DD：`49.4656/24.7951`、`40.6943/20.6445`。
- 2025：return `-8.7067%`；仍是主要 regime 缺口。
- Non-repeat：exact config + final engine/data hash；engine/data 变化时才允许复算。

## P4：Native Minigrid

- 实际执行：3 个 config binding probes；全部与 base 相同。
- 代码事实：`dca_minigrid` 只在 shared-domain config/validation 中出现，kline engine 和
  trading engine 均不读取。
- 状态：`unexecuted_native_engine`，不是 family failure。
- Non-repeat：当前 engine 下只改 `dca_minigrid` 字段的任何搜索都应 skipped；先实现 engine。

## P5：SO V2 / HTF Trend

- 原声称：ADX35/70 与 DD scale0.5/1.0 各两次相同，故 partial-TP 不 bind。
- 审计：没有保存 configs/result/event traces；ADX gate 实际位于通用 safety path，与 TP 类型
  无关，历史同结果不能证明 inert。
- 确认 bug：`drawdown_state_rules.safety_order_scale` 原来完全未被 kline engine 读取。
- 修复：backtest safety quantity 现按最高 active DD rule 缩放并进入 event detail；确定性测试
  已通过。trading-engine parity 未完成。
- HTF per-symbol long/short direction state 完全未执行。
- 状态：P5 未完成；Round13 先做 deterministic binding/live parity，再做 ablation/search。

## P6：Ordinary Partial-TP Stage Grid

- 实际 grid：4 spacing x 4 TP-stage0 x 3 stage1 x 3 stage2 x 4 max-age = 576。
- 576/576 evaluated，0 target；0 个 ann >=50%，0 个达到 4/5 positive。
- Best：ann `25.9807%`、DD `28.5702%`、2/5 positive。
- Engine 语义：partial TP stage 在 TP fill 后推进，不在 safety-leg fill 后推进；因此不是
  cycle-depth-aware TP。ATR 只做过无 raw artifact 的单点 probe，没有执行计划中的 ATR grid。
- Non-repeat：`r12-partial-tp-stage-grid-576`，只限该 ordinary stage/max-age 范围。
- 真实 safety-depth TP、frozen ATR spacing 仍开放。

## P7：LP Substitute Config

- 实际只创建一份配置：LTC/DYDX/INJ/FIL/ICP/XRP/UNI/BTC，统一 FOQ15、2.8x、8 legs、
  copied R4-like partial TP。
- 独立 full replay：ann `30.4412%`、DD `41.3107%`、return `147.9179%`、无 breach。
- 未恢复任何报告中的原 candidate ladder；`data/martingale_portfolios.db` 当前为 0 bytes。
- 未执行单体、2/4/6/8 ablation、1000 trials/profile、WFO/neighbor/stress。
- Non-repeat：`r12-lp-conservative-symbols-r4like-one-config`；不得扩写为所有 LP family。

## P8：Batch Acceleration

- Baseline artifact 声称 21.7s/config、28 workers 692 configs/hour、15% efficiency。
- preload+Rayon、normalized cache、checkpoint parity 和 >=5x benchmark 均未实现。
- 状态：`partial_benchmark_only`；瓶颈结论缺 profiler artifact，仅供诊断。

## P9：Holdout

- 原 artifact verdict 文本写“positive”，但数值为负；funding 仅覆盖到 6 月 22 日且 K 线缺口
  未披露。
- 修复后 4999U：return `-10.9593%`、DD `12.9221%`、408 trades、无 breach，FAIL。
- 五档 return 全负：`-18.5035/-9.2517/-18.2618/-13.6963/-10.9593%`。
- 该结果对 exact R4 config 有效。窗口已打开，R4 后续调参不能再称 untouched。
- 其余 WFO/neighbor/LOSO/stress/finalists 未执行。

## P10：Production Parity

- Round12 计划列出的 allocator/minigrid DB-backed tests 全部不存在。
- 运行已有 backtest/trading suite 不能证明尚未实现的新机制。
- 状态：`unexecuted`；R11 allocator started-executor/observation-writer 缺口仍在。

## 最终 Verdict

- Conservative：未达；R4 ann 不足且 DD 超 10%。
- Balanced：未达；ann 不足。
- Aggressive：未达；ann 不足。
- `<5000U`：4999U 可执行不等于目标达成；1000/2000U 的 R4 仍为负。
- Multi-symbol：R4/R7 是多币组合，但没有候选通过 PnL concentration gate。
- Anti-overfit/live：没有候选完成严格证据链。

下一步：
`docs/superpowers/plans/2026-07-12-glm-martingale-core-round13-native-regime-portfolio-plan.md`。
