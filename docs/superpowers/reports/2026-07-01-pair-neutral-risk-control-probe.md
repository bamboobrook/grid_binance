# 2026-07-01 Pair-Neutral Risk Control Probe

This is a research-only DD stop/cooldown check for pair-neutral grid streams. It does not trade, touch Binance, flyingkid, live mode, or real funds.

- live_parity_status: `research_only`
- rows: `27216`

## conservative

- passes: `0`
- best_ann: `dd10_cd15` `BNBUSDT,SOLUSDT` lb `80` z `1.0` ann `54.41` DD `20.99` cap `1000.00` pos `5/5` 2024-2026 `141.07` events `22` pass `False`
- best_ann_seg_cap: `dd10_cd60` `ADAUSDT,LINKUSDT` lb `20` z `1.5` ann `16.10` DD `22.02` cap `1000.00` pos `5/5` 2024-2026 `2.70` events `9` pass `False`
- best_dd_seg_cap: `dd10_cd60` `ADAUSDT,LINKUSDT` lb `20` z `1.5` ann `16.10` DD `22.02` cap `1000.00` pos `5/5` 2024-2026 `2.70` events `9` pass `False`

## balanced

- passes: `0`
- best_ann: `dd10_cd15` `BNBUSDT,SOLUSDT` lb `80` z `1.0` ann `54.41` DD `20.99` cap `1000.00` pos `5/5` 2024-2026 `141.07` events `22` pass `False`
- best_ann_seg_cap: `dd10_cd15` `BNBUSDT,SOLUSDT` lb `80` z `1.0` ann `54.41` DD `20.99` cap `1000.00` pos `5/5` 2024-2026 `141.07` events `22` pass `False`
- best_dd_seg_cap: `dd10_cd60` `BTCUSDT,SOLUSDT` lb `20` z `2.0` ann `4.15` DD `12.18` cap `3000.00` pos `3/5` 2024-2026 `9.56` events `12` pass `False`

## aggressive

- passes: `0`
- best_ann: `dd10_cd15` `BNBUSDT,SOLUSDT` lb `80` z `1.0` ann `54.41` DD `20.99` cap `1000.00` pos `5/5` 2024-2026 `141.07` events `22` pass `False`
- best_ann_seg_cap: `dd10_cd15` `BNBUSDT,SOLUSDT` lb `80` z `1.0` ann `54.41` DD `20.99` cap `1000.00` pos `5/5` 2024-2026 `141.07` events `22` pass `False`
- best_dd_seg_cap: `dd10_cd60` `BTCUSDT,SOLUSDT` lb `20` z `2.0` ann `4.15` DD `12.18` cap `3000.00` pos `3/5` 2024-2026 `9.56` events `12` pass `False`

## Conclusion

Potential research-only pair-neutral risk-control passes found: `0` under this scan. DD stop/cooldown does not preserve enough return while satisfying the original drawdown and segment gates.
