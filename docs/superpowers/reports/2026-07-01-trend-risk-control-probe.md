# 2026-07-01 Trend Risk Control Probe

This is a research-only portfolio DD stop/cooldown check. It does not trade, touch Binance, flyingkid, live mode, or real funds.

- live_parity_status: `research_only`
- rows: `14400`

## conservative

- passes: `0`
- best_ann: `dd20_cd15` `mom60_ls` `ETHUSDT,INJUSDT` ann `43.24` DD `61.88` cap `1000.00` pos `4/5` 2024-2026 `39.67` events `58` pass `False`
- best_ann_seg_cap: `None`
- best_dd_seg_cap: `None`

## balanced

- passes: `0`
- best_ann: `dd20_cd15` `mom60_ls` `ETHUSDT,INJUSDT` ann `43.24` DD `61.88` cap `1000.00` pos `4/5` 2024-2026 `39.67` events `58` pass `False`
- best_ann_seg_cap: `dd20_cd60` `donchian60_ls` `BTCUSDT,BNBUSDT` ann `22.55` DD `26.24` cap `3000.00` pos `3/5` 2024-2026 `69.51` events `8` pass `False`
- best_dd_seg_cap: `dd20_cd60` `donchian60_ls` `BTCUSDT,BNBUSDT` ann `22.55` DD `26.24` cap `3000.00` pos `3/5` 2024-2026 `69.51` events `8` pass `False`

## aggressive

- passes: `0`
- best_ann: `dd20_cd15` `mom60_ls` `ETHUSDT,INJUSDT` ann `43.24` DD `61.88` cap `1000.00` pos `4/5` 2024-2026 `39.67` events `58` pass `False`
- best_ann_seg_cap: `dd40_cd15` `mom60_ls` `ETHUSDT,BNBUSDT` ann `41.92` DD `36.25` cap `1000.00` pos `4/5` 2024-2026 `116.08` events `0` pass `False`
- best_dd_seg_cap: `dd20_cd60` `donchian60_ls` `BTCUSDT,BNBUSDT` ann `22.55` DD `26.24` cap `3000.00` pos `3/5` 2024-2026 `69.51` events `8` pass `False`

## Conclusion

Potential research-only risk-controlled trend passes found: `0` under this scan. Portfolio DD stop/cooldown lowers drawdown in some cases but does not preserve enough annualized return to meet the original gates.
