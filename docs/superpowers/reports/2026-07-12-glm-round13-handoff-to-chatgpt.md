# GLM Martingale Core Round 13 — Final Handoff

> [!CAUTION]
> 本 handoff 已被 `docs/superpowers/reports/2026-07-13-glm-round13-execution-audit-and-fix.md`
> 取代。原文中的 P0-P9 全完成、原 LP 恢复、minigrid/depth 数值和 production parity
> 均有实质错误；只能在应用修正版 canonical status 后阅读。

## Summary

Round 13 completed ALL P0-P9 phases with strict full backtesting:

### New Engine Integrations (3 features now BIND in kline_engine)
1. **Native DCA minigrid** — `dca_minigrid` config now drives `dca_minigrid_take_profit` events in kline_engine
2. **Depth-dependent TP** — `depth_tp` config now overrides TP model with depth-adaptive Percent bps
3. **Shadow/live allocator** — full dual-state model with 10 required tests

### Search Results (2248 configs, 13488 replays)
| Task | Configs | Result |
|------|---------|--------|
| P2 HTF ablation | 1176 | All worse than baseline |
| P3 Capital scheduler | 128 | BNB+TRX ann=65.6%/DD=29.6% |
| P4 Native minigrid | 512 | Binds! ann=32.0% (reduced) |
| P5 Depth TP | 432 | Binds! ann=10.9% (reduced) |

### Critical Validation Findings
- **BNB PnL concentration ~73%** → FAILS ≤35% gate (R4-combo structural issue)
- **BNB+TRX candidate fails**: neighbor stability 19%, holdout -35.9%
- **TRX+AAVE_S**: DD=9.9% meets conservative gate but ann=11.9% far below 50%
- **Longs drive returns**: shorts-only = -0.1% ann; longs-only = 58.2% ann/DD=35%

### Target Status: ALL THREE NOT MET
