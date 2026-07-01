# 2026-07-01 Pair-Neutral Portfolio Probe Design

## Goal

Close one remaining martingale/grid evidence gap: existing pair-neutral grid probes test one pair at a time, but they do not test whether several pair-neutral grid streams can be combined to reduce portfolio drawdown enough to satisfy the original gates.

The original gates remain unchanged:

- capital below `5000U`;
- multi-symbol exposure;
- robust H1-2023, H2-2023, 2024, 2025, and 2026_ytd behavior;
- conservative: annualized return above `50%` with DD at or below `10%`;
- balanced: annualized return above `90%` with DD at or below `20%`;
- aggressive: annualized return above `110%` with DD at or below `30%`;
- no live/Binance/flyingkid/real-funds action before a separate live-parity promotion.

This phase is `research_only`. A passing row would be a research candidate, not live-ready proof.

## Current Gap

`scripts/pair_neutral_grid_probe.py` and `scripts/pair_neutral_risk_control_probe.py` found `0` passes on single pair streams. The nearest useful frontier was `BNBUSDT,SOLUSDT` around `54.41% ann` with DD between about `17.74%` and `23.60%` depending on risk control, and with strong segment balance.

That does not pass any final profile, but it raises a narrow question: can several lower-correlated pair streams, sized under one portfolio budget, reduce drawdown while preserving enough return?

## Approach

Add one isolated Python research script:

- `scripts/pair_neutral_portfolio_probe.py`

The script will reuse existing helpers instead of duplicating model logic:

- `pair_neutral_grid_probe.py` to build per-pair equity streams from local daily close pairs;
- `hybrid_martingale_frontier_probe.py` to combine streams and evaluate full and segment gates.

The probe will:

1. Build candidate pair streams for selected symbol pairs, lookbacks, entry z-scores, and per-pair allocations.
2. Select 2-4 pair streams into a portfolio.
3. Enforce symbol-overlap controls so apparent diversification is not just repeated exposure to one symbol.
4. Combine equity curves with forward-fill semantics already used by the hybrid probe.
5. Evaluate the original C/B/A gates and segment gates.
6. Report passes, near misses, highest annualized rows, lowest drawdown rows, and transparent gap scores.

## Search Scope

Default symbols:

- `BTCUSDT`, `ETHUSDT`, `BNBUSDT`, `SOLUSDT`, `XRPUSDT`, `ADAUSDT`, `DOGEUSDT`, `LINKUSDT`

Default stream parameters:

- lookbacks: `20`, `40`, `80`
- entry z-scores: `1.0`, `1.5`, `2.0`
- exit z-score: `0.25`
- fee: `4 bps`
- per-pair allocations: `500`, `1000`, `1500`
- pair portfolio sizes: `2`, `3`, `4`
- portfolio budget: `5000U`

The first implementation should keep the search bounded by row limits and sorted candidate preselection, because full pair-stream cross-products can grow quickly.

## Gates

A row passes only if:

- `passes_offline` from the hybrid report is true for the relevant profile;
- `max_capital_used_quote < 5000`;
- `budget_blocked_events == 0`;
- final symbol count is at least `2`;
- segment gate passes with the existing profile-specific segment constraints;
- `live_parity_status == research_only`.

The script must not emit `live_parity_passed`.

## Outputs

Write:

- JSON to a user-specified path;
- markdown to a user-specified path;
- final repo report at `docs/superpowers/reports/2026-07-01-pair-neutral-portfolio-probe.md` after the search.

The markdown report must state:

- this is research-only;
- no live trading, Binance, flyingkid, or real funds were touched;
- total rows and pass counts;
- best rows per C/B/A profile;
- whether multi-pair diversification closes the previous pair-neutral gap.

## Testing

Use TDD with focused unit tests in:

- `tests/verification/test_pair_neutral_portfolio_probe.py`

Required behaviors:

- pair portfolio generation can reject excessive symbol overlap;
- combining two deterministic pair streams sums capital and equity correctly;
- row conversion includes all constituent pairs and symbols;
- gap scoring ranks rows with lower annualized/DD/capital/segment gaps ahead;
- output remains `research_only` and does not use live-parity labels.

## Non-Goals

- Do not touch production live trading code.
- Do not query Binance or external APIs.
- Do not modify existing pair-neutral probe behavior except by reusing its helpers.
- Do not claim a passing research row is live-ready.
- Do not weaken the original gates.

## Decision Rule

If this probe finds a pass, the next step is a separate live-parity promotion design for only that mechanism. If it finds `0` passes, the pair-neutral diversification escape hatch is closed for this bounded research scope.
