# GLM Martingale Core Round 12：审计修正后交接

日期：2026-07-12

原执行 HEAD：`5fa73efb5ab0b4b546a4c70d9dd43b9808b0e3dd`

权威审计：`docs/superpowers/reports/2026-07-12-glm-round12-execution-audit-and-fix.md`

## 最终结论

Round12 原报告“P0-P10 全部闭合”错误。审计修正后，保守/平衡/激进三档仍全部未达，
且没有 fully live-ready 候选。

| 结果 | Annualized | Max DD | 备注 |
|---|---:|---:|---|
| R4 corrected full 4999U | 34.7284% | 17.6868% | 4/5 positive |
| R4 repaired holdout 4999U | -65.3272% diagnostic | 12.9221% | 实际 return -10.9593%，FAIL |
| R9 static 36-strategy merge | -7.5209% | 56.0007% | 不是 dynamic allocator |
| P6 ordinary partial-stage best | 25.9807% | 28.5702% | 2/5，不是 cycle-depth TP |
| P7 one substitute config | 30.4412% | 41.3107% | 不是原 LP member configs |

## 关键修复

1. manifest 改为真实 `futures_usdt_perp/1m` 统计；15 symbols 的 development 和 holdout
   K 线、funding gate 均通过。
2. 从 Binance 官方 API 修复 holdout 36,351 根候选币 K 线缺口及 funding，保存 response hash。
3. funding loader 不再把 missing/incomplete/internal-gap 数据静默视为 0。
4. validator 改为 fail-closed，不再伪报 WFO、cost stress 或 live-ready。
5. 修复 backtest `drawdown_state_rules.safety_order_scale` 未执行的问题并添加测试。
6. 收窄 P3/P6/P7 non-repeat scope；P4/P5/P8/P10 明确标为未完成。

## 给 GLM 的下一步

严格执行：

`docs/superpowers/plans/2026-07-12-glm-martingale-core-round13-native-regime-portfolio-plan.md`

优先级：

1. preload+Rayon batch parity；
2. HTF per-symbol long/short Martingale + live safety scale；
3. event-level shadow/live allocator + capital-aware active-cycle scheduler；
4. true safety-depth TP/frozen ATR spacing；
5. native inventory-reducing minigrid；
6. 只有找回原 LP member configs 后才执行 LP portfolio search；
7. 真 nested WFO、neighbor、LOSO、cost/latency stress、locked holdout、production parity。

禁止重复：静态 R9 36-strategy merge、Round12 576 partial-stage labels、LP symbols + 统一
R4-like ladder。禁止把这些窄失败扩写成所有 allocator/depth TP/LP/minigrid 已失败。

## 权威文件

- 总状态：`docs/superpowers/artifacts/glm-martingale-core-round12/r1-r12-corrected-status.json`
- Final JSON：`docs/superpowers/artifacts/glm-martingale-core-round12/r12-final-validation.json`
- 独立 R4 复算：`docs/superpowers/artifacts/glm-martingale-core-round12/r12-r4-independent-recheck.json`
- 数据 manifest：`docs/superpowers/artifacts/glm-martingale-core-round12/run-manifests/r12-data-manifest.json`
- 数据 provenance：`docs/superpowers/artifacts/glm-martingale-core-round12/run-manifests/r12-holdout-fetch-provenance.json`
- 修复后 holdout：`docs/superpowers/artifacts/glm-martingale-core-round12/r12-holdout-validation.json`
- Round13 计划：`docs/superpowers/plans/2026-07-12-glm-martingale-core-round13-native-regime-portfolio-plan.md`
