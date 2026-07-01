# 2026-07-01 Pair-Neutral Grid Probe

This is a research-only pair-neutral grid check. It does not trade, touch Binance, flyingkid, live mode, or real funds.

- live_parity_status: `research_only`
- rows: `3024`

## conservative

- passes: `0`
- best_ann: `BNBUSDT,SOLUSDT` lb `80` z `1.0` ann `54.41` DD `23.60` cap `1000.00` pos `5/5` 2024-2026 `142.51` pass `False`
- best_ann_seg_cap: `None`
- best_dd_seg_cap: `None`

## balanced

- passes: `0`
- best_ann: `BNBUSDT,SOLUSDT` lb `80` z `1.0` ann `54.41` DD `23.60` cap `1000.00` pos `5/5` 2024-2026 `142.51` pass `False`
- best_ann_seg_cap: `BNBUSDT,SOLUSDT` lb `80` z `1.0` ann `54.41` DD `23.60` cap `1000.00` pos `5/5` 2024-2026 `142.51` pass `False`
- best_dd_seg_cap: `BTCUSDT,LINKUSDT` lb `80` z `2.0` ann `12.93` DD `22.05` cap `1000.00` pos `4/5` 2024-2026 `41.43` pass `False`

## aggressive

- passes: `0`
- best_ann: `BNBUSDT,SOLUSDT` lb `80` z `1.0` ann `54.41` DD `23.60` cap `1000.00` pos `5/5` 2024-2026 `142.51` pass `False`
- best_ann_seg_cap: `BNBUSDT,SOLUSDT` lb `80` z `1.0` ann `54.41` DD `23.60` cap `1000.00` pos `5/5` 2024-2026 `142.51` pass `False`
- best_dd_seg_cap: `BTCUSDT,LINKUSDT` lb `80` z `2.0` ann `12.93` DD `22.05` cap `1000.00` pos `4/5` 2024-2026 `41.43` pass `False`

## Conclusion

Potential research-only pair-neutral grid passes found: `0` under this scan. Pair-neutral spread grids reduce directional beta, but the observed frontier still fails the original return/DD/segment gates.
