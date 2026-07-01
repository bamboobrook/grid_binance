# 2026-07-01 Martingale Target Gap Audit

This is a read-only gap audit of saved research artifacts. It does not run live, touch Binance, flyingkid, or real funds.

Targets use the original gates: conservative ann >50% DD <=10%, balanced ann >90% DD <=20%, aggressive ann >110% DD <=30%, capital below 5000U, at least 4/5 positive segments, and positive combined 2024-2026 return.

- normalized candidate rows: `37199`
- final target passes: `0`

## Sources

- `trend_sleeve` rows `1200` path `/tmp/trend_sleeve_frontier_probe.json`
- `trend_risk_control` rows `14400` path `/tmp/trend_risk_control_probe.json`
- `pair_neutral_grid` rows `3024` path `/tmp/pair_neutral_grid_probe.json`
- `funding_sleeve` rows `90` path `/tmp/funding_sleeve_probe.json`
- `saved_result_leak_audit` rows `18485` path `/tmp/martingale_result_leak_audit_wide.json`

## conservative

- target ann: `>50.0`
- target DD: `<=10.0`
- rows: `11031`
- passes: `0`
- nearest by transparent gap score:
  - `saved_result_leak_audit` `dgt_dynamic_grid_probe_smoke.json` score `0.520` ann `23.98` gap `26.02` DD `8.29` excess `0.00` cap `100.00` cap_excess `0.00` pos `4` seg_gap `0` c2426 `40.96` c2426_gap `0.00`
  - `saved_result_leak_audit` `dgt_dynamic_grid_probe_smoke.json` score `0.520` ann `23.98` gap `26.02` DD `8.29` excess `0.00` cap `200.00` cap_excess `0.00` pos `4` seg_gap `0` c2426 `40.96` c2426_gap `0.00`
  - `saved_result_leak_audit` `dgt_dynamic_grid_probe_g2.json` score `0.520` ann `23.98` gap `26.02` DD `9.19` excess `0.00` cap `100.00` cap_excess `0.00` pos `4` seg_gap `0` c2426 `40.96` c2426_gap `0.00`
  - `saved_result_leak_audit` `dgt_dynamic_grid_probe_g2.json` score `0.520` ann `23.98` gap `26.02` DD `9.19` excess `0.00` cap `200.00` cap_excess `0.00` pos `4` seg_gap `0` c2426 `40.96` c2426_gap `0.00`
  - `saved_result_leak_audit` `dgt_dynamic_grid_probe_g2.json` score `0.520` ann `23.98` gap `26.02` DD `9.19` excess `0.00` cap `300.00` cap_excess `0.00` pos `4` seg_gap `0` c2426 `40.96` c2426_gap `0.00`
- highest annualized rows:
  - `saved_result_leak_audit` `dgt_dynamic_grid_probe_smoke.json` score `20.913` ann `698.09` gap `0.00` DD `67.24` excess `57.24` cap `79693.65` cap_excess `74693.65` pos `3` seg_gap `1` c2426 `70.82` c2426_gap `0.00`
  - `saved_result_leak_audit` `dgt_dynamic_grid_probe_smoke.json` score `36.851` ann `698.09` gap `0.00` DD `67.24` excess `57.24` cap `159387.30` cap_excess `154387.30` pos `3` seg_gap `1` c2426 `70.82` c2426_gap `0.00`
  - `saved_result_leak_audit` `dgt_dynamic_grid_probe_smoke.json` score `20.913` ann `698.09` gap `0.00` DD `67.24` excess `57.24` cap `79693.65` cap_excess `74693.65` pos `3` seg_gap `1` c2426 `70.82` c2426_gap `0.00`
- lowest drawdown rows:
  - `saved_result_leak_audit` `dgt_dynamic_grid_probe_smoke.json` score `1.297` ann `10.13` gap `39.87` DD `0.00` excess `0.00` cap `100.00` cap_excess `0.00` pos `2` seg_gap `2` c2426 `25.75` c2426_gap `0.00`
  - `saved_result_leak_audit` `dgt_dynamic_grid_probe_smoke.json` score `1.297` ann `10.13` gap `39.87` DD `0.00` excess `0.00` cap `200.00` cap_excess `0.00` pos `2` seg_gap `2` c2426 `25.75` c2426_gap `0.00`
  - `saved_result_leak_audit` `dgt_dynamic_grid_probe_smoke.json` score `0.927` ann `16.16` gap `33.84` DD `0.00` excess `0.00` cap `100.00` cap_excess `0.00` pos `3` seg_gap `1` c2426 `58.42` c2426_gap `0.00`

## balanced

- target ann: `>90.0`
- target DD: `<=20.0`
- rows: `10792`
- passes: `0`
- nearest by transparent gap score:
  - `pair_neutral_grid` `BNBUSDT,SOLUSDT` score `0.575` ann `54.41` gap `35.59` DD `23.60` excess `3.60` cap `1000.00` cap_excess `0.00` pos `5` seg_gap `0` c2426 `142.51` c2426_gap `0.00`
  - `pair_neutral_grid` `BNBUSDT,SOLUSDT` score `0.575` ann `54.41` gap `35.59` DD `23.60` excess `3.60` cap `2000.00` cap_excess `0.00` pos `5` seg_gap `0` c2426 `142.51` c2426_gap `0.00`
  - `pair_neutral_grid` `BNBUSDT,SOLUSDT` score `0.575` ann `54.41` gap `35.59` DD `23.60` excess `3.60` cap `4000.00` cap_excess `0.00` pos `5` seg_gap `0` c2426 `142.51` c2426_gap `0.00`
  - `pair_neutral_grid` `BNBUSDT,SOLUSDT` score `0.575` ann `54.41` gap `35.59` DD `23.60` excess `3.60` cap `3000.00` cap_excess `0.00` pos `5` seg_gap `0` c2426 `142.51` c2426_gap `0.00`
  - `saved_result_leak_audit` `dgt_dynamic_grid_probe_smoke.json` score `0.734` ann `23.98` gap `66.02` DD `8.29` excess `0.00` cap `100.00` cap_excess `0.00` pos `4` seg_gap `0` c2426 `40.96` c2426_gap `0.00`
- highest annualized rows:
  - `saved_result_leak_audit` `dgt_dynamic_grid_probe_smoke.json` score `17.551` ann `698.09` gap `0.00` DD `67.24` excess `47.24` cap `79693.65` cap_excess `74693.65` pos `3` seg_gap `1` c2426 `70.82` c2426_gap `0.00`
  - `saved_result_leak_audit` `dgt_dynamic_grid_probe_smoke.json` score `33.489` ann `698.09` gap `0.00` DD `67.24` excess `47.24` cap `159387.30` cap_excess `154387.30` pos `3` seg_gap `1` c2426 `70.82` c2426_gap `0.00`
  - `saved_result_leak_audit` `dgt_dynamic_grid_probe_smoke.json` score `17.551` ann `698.09` gap `0.00` DD `67.24` excess `47.24` cap `79693.65` cap_excess `74693.65` pos `3` seg_gap `1` c2426 `70.82` c2426_gap `0.00`
- lowest drawdown rows:
  - `saved_result_leak_audit` `dgt_dynamic_grid_probe_smoke.json` score `1.387` ann `10.13` gap `79.87` DD `0.00` excess `0.00` cap `100.00` cap_excess `0.00` pos `2` seg_gap `2` c2426 `25.75` c2426_gap `0.00`
  - `saved_result_leak_audit` `dgt_dynamic_grid_probe_smoke.json` score `1.387` ann `10.13` gap `79.87` DD `0.00` excess `0.00` cap `200.00` cap_excess `0.00` pos `2` seg_gap `2` c2426 `25.75` c2426_gap `0.00`
  - `saved_result_leak_audit` `dgt_dynamic_grid_probe_smoke.json` score `1.070` ann `16.16` gap `73.84` DD `0.00` excess `0.00` cap `100.00` cap_excess `0.00` pos `3` seg_gap `1` c2426 `58.42` c2426_gap `0.00`

## aggressive

- target ann: `>110.0`
- target DD: `<=30.0`
- rows: `15376`
- passes: `0`
- nearest by transparent gap score:
  - `saved_result_leak_audit` `dgt_dynamic_grid_probe_smoke.json` score `0.357` ann `152.58` gap `0.00` DD `40.70` excess `10.70` cap `1674.72` cap_excess `0.00` pos `4` seg_gap `0` c2426 `393.24` c2426_gap `0.00`
  - `saved_result_leak_audit` `dgt_dynamic_grid_probe_smoke.json` score `0.357` ann `152.58` gap `0.00` DD `40.70` excess `10.70` cap `3349.45` cap_excess `0.00` pos `4` seg_gap `0` c2426 `393.24` c2426_gap `0.00`
  - `saved_result_leak_audit` `dgt_dynamic_grid_probe_smoke.json` score `0.359` ann `152.64` gap `0.00` DD `40.76` excess `10.76` cap `1705.47` cap_excess `0.00` pos `4` seg_gap `0` c2426 `392.47` c2426_gap `0.00`
  - `saved_result_leak_audit` `dgt_dynamic_grid_probe_smoke.json` score `0.359` ann `152.64` gap `0.00` DD `40.76` excess `10.76` cap `3410.94` cap_excess `0.00` pos `4` seg_gap `0` c2426 `392.47` c2426_gap `0.00`
  - `saved_result_leak_audit` `dgt_dynamic_grid_probe_smoke.json` score `0.359` ann `131.09` gap `0.00` DD `40.77` excess `10.77` cap `1252.76` cap_excess `0.00` pos `4` seg_gap `0` c2426 `409.49` c2426_gap `0.00`
- highest annualized rows:
  - `saved_result_leak_audit` `dgt_dynamic_grid_probe_smoke.json` score `16.430` ann `698.09` gap `0.00` DD `67.24` excess `37.24` cap `79693.65` cap_excess `74693.65` pos `3` seg_gap `1` c2426 `70.82` c2426_gap `0.00`
  - `saved_result_leak_audit` `dgt_dynamic_grid_probe_smoke.json` score `32.369` ann `698.09` gap `0.00` DD `67.24` excess `37.24` cap `159387.30` cap_excess `154387.30` pos `3` seg_gap `1` c2426 `70.82` c2426_gap `0.00`
  - `saved_result_leak_audit` `dgt_dynamic_grid_probe_smoke.json` score `16.430` ann `698.09` gap `0.00` DD `67.24` excess `37.24` cap `79693.65` cap_excess `74693.65` pos `3` seg_gap `1` c2426 `70.82` c2426_gap `0.00`
- lowest drawdown rows:
  - `saved_result_leak_audit` `member_02.json` score `2.010` ann `0.00` gap `110.00` DD `0.00` excess `0.00` cap `0.00` cap_excess `0.00` pos `missing` seg_gap `4` c2426 `missing` c2426_gap `1.00`
  - `saved_result_leak_audit` `dgt_dynamic_grid_probe_smoke.json` score `1.408` ann `10.13` gap `99.87` DD `0.00` excess `0.00` cap `100.00` cap_excess `0.00` pos `2` seg_gap `2` c2426 `25.75` c2426_gap `0.00`
  - `saved_result_leak_audit` `dgt_dynamic_grid_probe_smoke.json` score `1.408` ann `10.13` gap `99.87` DD `0.00` excess `0.00` cap `200.00` cap_excess `0.00` pos `2` seg_gap `2` c2426 `25.75` c2426_gap `0.00`

## Conclusion

No saved row from the audited DGT, trend, funding, and leak-audit artifacts meets the original target gates. The nearest rows still fail on annualized return, drawdown, segment balance, or missing segment evidence.
