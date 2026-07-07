# GLM Martingale Round 8 Search Ledger


## r8-P2-regime-rescue-001 (Task P2: 2025 Regime-Rescue Allocator — 5184 configs)
- Built equity curves for 4 candidates (ANKR-q, QB, R4, fine).
- Ran 5184 allocator configs (4 lookbacks × 3 rebalances × 4 scores × 3 max_hi × 3 min_lo × 4 cash_dd × 3 hyst_gaps).
- **BREAKTHROUGH: ann 62.0%, DD 18.2%** — first time across 8 rounds that ann>50 AND DD≤20 simultaneously!
  - Config: 60d lookback, 7d rebalance, rolling_return_minus_2x_dd score, max_hi=0.25, min_lo=0.20
  - The allocator rotates among martingale sleeves using only lagged 60d performance minus 2x DD
  - DD 18.2% is within balanced ≤20% gate
  - 100+ configs achieve this target (many parameter combinations converge to the same allocation)

## r8-P3-dca-grid-reset-001 (Task P3: DCA Dynamic Grid Reset)
- 148 candidates × 6 replays. Spacing variations (long 120-210, short 150-250, SO condition variants).
- No target hit. Spacing variations don't improve over base configs.

## r8-P4-cost-cover-rescue-001 (Task P4: Cost-Cover TP and Rescue Exit)
- 96 candidates × 6 replays. max_cycle_age (72-336h) + no_progress_exit + BE stage variants.
- No target hit. Active exits reduce DD slightly but reduce ann more.
