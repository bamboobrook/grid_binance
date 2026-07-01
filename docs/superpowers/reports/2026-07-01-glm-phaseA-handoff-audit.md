# 2026-07-01 GLM Phase A Handoff Audit

This is a read-only audit of the GLM Phase A handoff referenced by the martingale/grid objective. It does not trade, touch Binance, flyingkid, live mode, or real funds.

## Source Checked

- Source worktree: `.claude/worktrees/p4-cycle-exit`
- Source branch: `worktree-p4-cycle-exit`
- Source commit seen locally: `ff937b5`
- Handoff file: `.claude/worktrees/p4-cycle-exit/docs/superpowers/plans/2026-06-30-glm-phaseA-handoff-for-chatgpt.md`
- Supporting proof report: `.claude/worktrees/p4-cycle-exit/docs/superpowers/reports/2026-06-30-glm-phaseA-infeasibility-proof.md`
- Current summary consumer: `docs/superpowers/reports/2026-07-01-final-martingale-verdict-and-external-check.md`

The handoff file is present in the retained P4 worktree rather than as a first-class file in the main worktree. This audit records the key evidence so the current main-branch report set can be reviewed without relying on memory.

## Requirement Match

The handoff preserves the same objective:

- capital at or below `5000 USDT`;
- multi-symbol, not a single-symbol escape;
- anti-overfit and balanced `H1-2023`, `H2-2023`, `2024`, `2025`, `2026_ytd` behavior;
- conservative `ann >50% / DD<=10%`;
- balanced `ann >90% / DD<=20%`;
- aggressive `ann >110% / DD<=30%`;
- live-parity before any Binance, flyingkid, live mode, or real funds.

## Key Evidence Cross-Check

| Handoff claim | Current audit status |
|---|---|
| Phase A used `portfolio_budget_replay` over `market_data_full.db` with about `1500` candidates and `590` segment validations. | Reflected in the final verdict's GLM Phase A section. |
| v1 large-cap regime MR: conservative best `1.5% ann / 9.0% DD`, balanced best `4.2% ann / 11.4% DD`. | Consistent with the current finding that low-DD pure MR misses annualized-return targets. |
| v2 broad alt pool plus wide SL and portfolio stop: conservative `3.5% / 5.8%`, balanced `9.3% / 13.7%`, aggressive `14.5% / 22.7%`. | Consistent with current evidence that portfolio stops do not create enough return. |
| v3 per-coin regime allocator: aggressive reached `21.2% ann / 41.7% DD`, `3/5` positive segments, `2025=+20.6`, `2024=-12.2`, `h1_2023=92.9`. | Consistent with the structural 2024/2025 conflict and DD failure. |
| Cross-experiment mining found only `2` configs with positive segments `>=4` and positive `2024-2026`, both around `0.8% ann`. | Consistent with current target-gap audit: robust rows exist, but annualized return is far below C/B/A targets. |
| `2024 >= 0` and `2025 >= 0` simultaneously: `0/590`. | Consistent with the final verdict's 2024/2025 anti-correlation summary. |
| Handoff recommends not repeating ten already rejected pure martingale/grid paths. | Consistent with `2026-07-01-martingale-grid-search-freeze-and-reopen-criteria.md`. |

## Interpretation

The GLM handoff is not a standalone mathematical impossibility proof. It is a bounded engineering search result over the pure martingale/grid mechanisms available in the P4 worktree. Current ChatGPT-side audits, DGT probes, pair-neutral probes, external-claim checks, and live-promotion gates independently point to the same result:

- no current martingale/grid candidate satisfies all C/B/A gates;
- high-return rows fail drawdown, segment balance, budget, or live-readiness;
- low-drawdown robust rows miss annualized-return targets by a wide margin;
- no live promotion is justified.

## Conclusion

The GLM Phase A handoff has been checked against the current main-branch evidence set. Its key numbers are consistent with the current final verdict, and it does not contain a hidden qualifying martingale/grid candidate.
