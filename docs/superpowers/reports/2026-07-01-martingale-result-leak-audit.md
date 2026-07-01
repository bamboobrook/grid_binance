# 2026-07-01 Martingale Result Leak Audit

This is a read-only audit of saved JSON artifacts. It does not run live, touch Binance, flyingkid, or real funds.

- JSON-like records scanned: `18485`

- Full-gate pass-like rows: `19`
- Final-gate pass rows: `0`

## conservative

- rows: `4793`
- full-gate pass-like: `0`
- final-gate pass: `0`
- top annualized rows:
  - `dgt_dynamic_grid_probe_smoke.json` ann `698.0860751805847` DD `67.24030464549284` cap `79693.6499999998` full `False` final `False`
  - `dgt_dynamic_grid_probe_smoke.json` ann `698.0860751805847` DD `67.24030464549284` cap `159387.2999999996` full `False` final `False`
  - `dgt_dynamic_grid_probe_smoke.json` ann `698.0860751805847` DD `67.24030464549284` cap `79693.6499999998` full `False` final `False`
  - `dgt_dynamic_grid_probe_smoke.json` ann `698.0860751805847` DD `67.24030464549284` cap `159387.2999999996` full `False` final `False`
  - `dgt_dynamic_grid_probe_g2.json` ann `698.0860751805847` DD `69.11503004457488` cap `79693.6499999998` full `False` final `False`

## balanced

- rows: `4554`
- full-gate pass-like: `0`
- final-gate pass: `0`
- top annualized rows:
  - `dgt_dynamic_grid_probe_smoke.json` ann `698.0860751805847` DD `67.24030464549284` cap `79693.6499999998` full `False` final `False`
  - `dgt_dynamic_grid_probe_smoke.json` ann `698.0860751805847` DD `67.24030464549284` cap `159387.2999999996` full `False` final `False`
  - `dgt_dynamic_grid_probe_smoke.json` ann `698.0860751805847` DD `67.24030464549284` cap `79693.6499999998` full `False` final `False`
  - `dgt_dynamic_grid_probe_smoke.json` ann `698.0860751805847` DD `67.24030464549284` cap `159387.2999999996` full `False` final `False`
  - `dgt_dynamic_grid_probe_g2.json` ann `698.0860751805847` DD `69.11503004457488` cap `79693.6499999998` full `False` final `False`

## aggressive

- rows: `9138`
- full-gate pass-like: `19`
- final-gate pass: `0`
- full-gate pass-like rows:
  - `replay_aggressive_4000.json` ann `120.58741821698207` DD `28.965042906691608` cap `2735.2156560105173` pos `None` c2426 `None` final `False` violations `missing segment evidence; missing 2024-2026 return evidence`
  - `best_aggressive_fixed_cash_b3250_result.json` ann `133.54374560822632` DD `29.87601985463672` cap `2735.2156560105173` pos `None` c2426 `None` final `False` violations `missing segment evidence; missing 2024-2026 return evidence`
  - `curve_smoke.json` ann `132.4088184538756` DD `27.975609394409663` cap `1755.6875000258876` pos `None` c2426 `None` final `False` violations `missing segment evidence; missing 2024-2026 return evidence`
  - `aggressive_fixed_pass__default_dd6_atr2_adx45.json` ann `133.54374560822632` DD `29.87601985463672` cap `2735.2156560105173` pos `None` c2426 `None` final `False` violations `missing segment evidence; missing 2024-2026 return evidence`
  - `aggressive_fixed_pass__strict_dd4_atr15_adx40.json` ann `136.62492203475574` DD `29.467825801094904` cap `2735.215656010533` pos `None` c2426 `None` final `False` violations `missing segment evidence; missing 2024-2026 return evidence`
  - `0178_full_pool_b2000_top_27.json` ann `115.78712934678617` DD `29.101659064338257` cap `1347.3945222688988` pos `None` c2426 `None` final `False` violations `missing segment evidence; missing 2024-2026 return evidence`
  - `0105_full_pool_b3000_top_12_fixed_cash_b4250.json` ann `116.97291007069639` DD `28.673605230736417` cap `2735.2156560105173` pos `None` c2426 `None` final `False` violations `missing segment evidence; missing 2024-2026 return evidence`
  - `0105_full_pool_b3000_top_12_fixed_cash_b3250.json` ann `133.54374560822632` DD `29.87601985463672` cap `2735.2156560105173` pos `None` c2426 `None` final `False` violations `missing segment evidence; missing 2024-2026 return evidence`
  - `0105_full_pool_b3000_top_12_fixed_cash_b4500.json` ann `113.6331773798271` DD `28.387973852550108` cap `2735.2156560105173` pos `None` c2426 `None` final `False` violations `missing segment evidence; missing 2024-2026 return evidence`
  - `0105_full_pool_b3000_top_12_fixed_cash_b4000.json` ann `120.58741821698207` DD `28.965042906691608` cap `2735.2156560105173` pos `None` c2426 `None` final `False` violations `missing segment evidence; missing 2024-2026 return evidence`
- top annualized rows:
  - `dgt_dynamic_grid_probe_smoke.json` ann `698.0860751805847` DD `67.24030464549284` cap `79693.6499999998` full `False` final `False`
  - `dgt_dynamic_grid_probe_smoke.json` ann `698.0860751805847` DD `67.24030464549284` cap `159387.2999999996` full `False` final `False`
  - `dgt_dynamic_grid_probe_smoke.json` ann `698.0860751805847` DD `67.24030464549284` cap `79693.6499999998` full `False` final `False`
  - `dgt_dynamic_grid_probe_smoke.json` ann `698.0860751805847` DD `67.24030464549284` cap `159387.2999999996` full `False` final `False`
  - `dgt_dynamic_grid_probe_g2.json` ann `698.0860751805847` DD `69.11503004457488` cap `79693.6499999998` full `False` final `False`

## Conclusion

Potential final-gate rows found: `0`. The saved JSON artifact pool contains no machine-detected row satisfying the original gates with segment evidence.

This audit is supplementary evidence; replay validation remains authoritative for any future candidate.
