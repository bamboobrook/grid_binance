# Round 6 Task A: Drawdown-Window Attribution Report

## Method
Ran r5-G-best-ANKRUSDT full period with 5000-point equity curve. Analyzed drawdown windows, segment DD, and price context.

## Key Findings

### DD is budget-based (32.1%), not margin-based (25.6%)
- on_budget DD: 32.1% (budget + cum_pnl basis)
- margin-based equity DD: 25.6% (margin capital basis)
- Difference: the on_budget metric uses a smaller denominator (5000U budget) while the margin equity grows to 86K

### Segment-level DD Attribution
| Segment | DD% | Peak | Trough |
|---|---:|---:|---:|
| h1_2023 | 12.3% | 73K | 64K |
| h2_2023 | 5.3% | 74K | 70K |
| 2024 | **15.7%** | 86K | 73K |
| 2025 | 6.0% | 86K | 81K |
| 2026_ytd | 3.4% | 85K | 82K |

**2024 has the worst segment DD (15.7%)**, followed by h1_2023 (12.3%).
The overall 32.1% DD comes from the on_budget metric which tracks peak-to-trough
across the ENTIRE period — the trough occurs early (Jan 2023) before the strategy
has accumulated gains, while the peak occurs in late 2025.

### Root Cause
The 32.1% DD is largely a STARTUP EFFECT: early in the backtest (h1_2023), the
strategy hasn't accumulated profits yet, so any drawdown from the initial 5000U
budget creates a large percentage DD. As the strategy compounds gains (equity
grows from 5K to 86K), later drawdowns are smaller in percentage terms even
though they may be larger in absolute terms.

### Route Mapping
| Finding | Route | Priority |
|---|---|---|
| DD concentrated in 2024 (15.7%) | Task B DD state machine to throttle in high-DD periods | High |
| h1_2023 DD 12.3% is startup effect | Task F blend with R4 (lower DD) may smooth | Medium |
| 2025 DD only 6.0% — 2025 ann is -17.5% but DD is modest | Task C quarantine won't help much here | Low |
| Only 367 trades total, 49 stops (13% stop rate) | Low stop rate = TP1 trailing lock (Task D) may not fire often | Low |
| Fees 310 + funding 677 = 987 total costs | Funding is significant (677 on 5000 budget = 13.5% annual drag) | Medium |

### Key Insight: Funding Drag
Total funding cost is 677 USDT on a 5000U budget = 13.5% annual drag. This is
significant and suggests the short sleeve (pump-fade) pays funding in bull markets.
Reducing short allocation or adding funding-aware gates could improve returns.
