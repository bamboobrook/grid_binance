# GLM Round 25R Corrected Handoff

Final status: `VALID_HISTORICAL_PREQUENTIAL_NO_TARGET`.

- Replay source commit: `f710449b7f933fe954ae5a6e936ecf0b7a1798dd`
- Branch: `codex/glm-martingale-core-round25-corrected`
- Artifact root: `docs/superpowers/artifacts/glm-martingale-core-round25-corrected`
- Local raw root: `artifacts-local/round25-corrected/f710449b7f933fe954ae5a6e936ecf0b7a1798dd`
- Remote sync: `PUSH_PENDING`
- Historical only: `true`

## Gate Results

- R0-R4 unit and production contracts: `PASS`
- G0: `PASS_WITH_REAL_SCHEDULER_NOT_TESTABLE`, 8 fixed-window replays
- G1 activation census: 16/16 complete, no step truncation
- G1 continuous shared-account replay: 16/16 terminal
- P-B survivors: 0
- G2/C2/scheduler: `not_applicable_no_corrected_c1_pb_survivor`
- Independent raw-trace validator: `PASS`, 16/16, zero violations

## Outcome

The best corrected policies were `R25-C1-11` and `R25-C1-12`: annualized return
`0.0690874861%`, max equity drawdown `0.0859724102%`, 3 actual assets, 2 distinct pairs,
2/12 positive blocks, and no loss-after-add SO. They fail P-B and do not support a target or
frontier claim.

All 16 corrected policies are exact valid failures. The corrected fingerprints are closed only for
this frozen 16-policy population; the result does not claim all Martingale possibilities are exhausted.

## Evidence

Raw traces remain local and reconstructible from source/data/policy hashes. Repository evidence contains
per-stream row counts, bytes, SHA256, schema, first/last rows, deterministic 100-row samples, compact
metrics, and independent recomputation. No committed file exceeds 10MB.

The two failed G0 attempts and three interrupted performance checkpoints are retained as audit evidence.
No prior Round 25 artifact root or local raw file was deleted.
