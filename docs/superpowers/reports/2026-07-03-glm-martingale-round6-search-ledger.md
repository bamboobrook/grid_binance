# GLM Martingale Round 6 DD Compression Search Ledger

Branch: `glm-martingale-core-round6`
Plan: `docs/superpowers/plans/2026-07-03-glm-martingale-core-round6-dd-compression-plan.md`
Round 5 best: `r5-G-best-ANKRUSDT` ann 59.5%, DD 32.1%, 4/5 pos, 2025 -17.5%.
Goal: compress DD to ≤10% (conservative) / ≤20% (balanced) / ≤30% (aggressive).
Order: A(attribution) → B(DD state machine) → C(quarantine) → D(trailing lock) → E(safety freeze) → F(blend) → G(parity) → H(ANKR closeout).


## r6-A-dd-attribution-001 (Task A: Drawdown-Window Attribution)
- Segment DD: 2024=15.7%, h1_2023=12.3%, 2025=6.0%, h2_2023=5.3%, 2026=3.4%
- Overall 32.1% DD includes startup effect (trough in Jan 2023 before gains compound)
- Funding drag: 677 USDT on 5000U budget (13.5% annual)
- Routes: B(DD state machine), F(blend), D(trailing lock - low stop rate)
