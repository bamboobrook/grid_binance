# 2026-07-01 DGT Dynamic Grid Probe Design

## 1. Goal

Build a `research_only` Dynamic Grid Trading (DGT) probe to test whether dynamic grid reset mechanics can move the current martingale/grid frontier toward the original target:

- capital or maximum input below 5000 USDT;
- multi-symbol, not a single BTC escape hatch;
- robust across H1-2023, H2-2023, 2024, 2025, and 2026_ytd;
- conservative: annualized return above 50% with max drawdown at or below 10%;
- balanced: annualized return above 90% with max drawdown at or below 20%;
- aggressive: annualized return above 110% with max drawdown at or below 30%;
- any final candidate must later be promoted to live-parity before live trading, Binance, flyingkid, or real funds.

The target is not lowered. This phase only answers whether DGT is worth promoting.

## 2. Current Evidence

Existing pure martingale/grid evidence remains negative under the original gates:

- GLM Phase A and ChatGPT audits found no pure martingale candidate that passes full-period, budget, segment robustness, and live-parity gates together.
- Hybrid martingale + trend + funding probes improved the frontier, but still produced 0 passes.
- Funding/carry history is too small to bridge the 50%/90%/110% annualized targets alone.
- A one-off DGT rough probe found a promising but invalid seed: BTC DGT can reach about 136% annualized with about 36.8% DD and about 1300U max input. It is single-symbol, not live-parity, and DD exceeds the aggressive 30% gate.

The next useful question is whether multi-symbol DGT and portfolio-level filtering can reduce drawdown enough without destroying annualized return.

## 3. DGT Mechanism Under Test

The probe models a dynamic grid:

1. Start each symbol with a grid centered on the current price.
2. Use symmetric grid levels around the center, parameterized by spacing and half-grid count.
3. When price crosses grid levels, count grid traversals and realized grid profit under an explicit fee model.
4. When price breaks above or below the outer grid boundary, close or roll the active grid and reset a new grid around the breakout price.
5. Track stranded inventory from downside breaks instead of pretending all principal remains liquid.
6. Track additional capital required to start new grids, and mark the run invalid if `max_input_quote >= 5000`.

The accounting must be conservative enough to reject false positives. It is acceptable for Phase 1 to be pessimistic; it is not acceptable to hide capital top-ups, stranded inventory, fees, or drawdown.

## 4. Search Scope

Initial symbol pool:

- `BTCUSDT`
- `ETHUSDT`
- `BNBUSDT`
- `SOLUSDT`
- `XRPUSDT`
- `DOGEUSDT`
- `ADAUSDT`
- `LINKUSDT`
- `AAVEUSDT`
- `INJUSDT`

Initial parameter dimensions:

- grid spacing: bounded values around the rough-positive BTC seeds, plus wider low-turnover settings;
- half-grid count: small and medium grids, including the rough-positive BTC `half=2` and `half=7` seeds;
- per-symbol principal;
- symbol group size from 2 to 5;
- portfolio allocation weights;
- optional portfolio drawdown stop and cooldown;
- optional 2026_ytd guard to reject candidates that only pass because of earlier trend periods;
- optional low-weight funding/carry stream as a smoothing sleeve, not as the primary return source.

The search must never accept a single-symbol candidate as satisfying the user target.

## 5. Outputs

The probe must emit JSON and markdown reports with:

- live parity status: always `research_only` in this phase;
- selected symbols and DGT parameters;
- full-period annualized return, total return, max drawdown, max input/capital used, and fee estimate;
- segment metrics for H1-2023, H2-2023, 2024, 2025, and 2026_ytd;
- positive segment count and 2024-2026 aggregate return;
- per-symbol attribution;
- invalidation reasons for over-budget, single-symbol, insufficient data, negative equity, or segment failure;
- top near misses for each C/B/A profile.

Reports must state explicitly that passing this probe does not imply live readiness.

## 6. Acceptance Gates

The final C/B/A gates remain:

| Profile | Annualized return | Max drawdown | Budget |
|---|---:|---:|---:|
| Conservative | > 50% | <= 10% | < 5000 USDT |
| Balanced | > 90% | <= 20% | < 5000 USDT |
| Aggressive | > 110% | <= 30% | < 5000 USDT |

Additional DGT-specific gates:

- symbol count must be at least 2;
- every signal and reset must use only current or prior market data;
- `max_input_quote` must be below budget, not just final equity;
- every segment must be reported;
- positive segment count and 2024-2026 aggregate return must be used to reject overfit candidates;
- no candidate may be labeled `live_parity_passed` in this phase.

## 7. Architecture

Keep the first implementation isolated in Python research scripts:

- `scripts/dgt_dynamic_grid_probe.py`: DGT simulator, metrics, segment evaluation, and report writer.
- `tests/verification/test_dgt_dynamic_grid_probe.py`: deterministic tests for accounting and gates.
- `docs/superpowers/reports/2026-07-01-dgt-dynamic-grid-probe.md`: final search report.

The probe can reuse existing segment constants and gate semantics from `scripts/hybrid_martingale_frontier_probe.py` where practical, but it should not modify production trading-engine code.

Promotion to Rust backtest/live code is a separate phase and requires a new design after a research candidate passes.

## 8. Data Flow

1. Load local 1m market data from `data/market_data_full.db`.
2. Build per-symbol DGT equity streams under selected parameters.
3. Combine streams under portfolio weights and budget caps.
4. Compute full-period and segment metrics.
5. Apply C/B/A gates and DGT-specific overfit gates.
6. Write candidate JSON and markdown summaries.

The default run should use local data only. It must not call Binance, external APIs, live services, or flyingkid.

## 9. Error Handling

Fail closed:

- missing or sparse data rejects the symbol for that run;
- negative or zero equity marks the candidate invalid;
- `max_input_quote >= budget` marks the candidate invalid;
- any unknown fee or budget accounting mode marks the candidate invalid;
- output files must include invalidation reasons rather than silently dropping failures.

## 10. Testing

Implementation must use TDD. Required tests:

- DGT reset accounting tracks additional capital after downside breaks;
- `max_input_quote` gate rejects over-budget candidates;
- single-symbol candidates cannot pass the final target gates;
- segment splitter covers the exact five required periods;
- a deterministic tiny market fixture produces expected reset counts and equity;
- reports keep `live_parity_status` as `research_only`.

## 11. Non-Goals

- Do not start live trading.
- Do not touch Binance credentials or place orders.
- Do not publish to flyingkid.
- Do not weaken C/B/A thresholds.
- Do not claim DGT is live-ready.
- Do not rewrite the Rust engine in this phase.
- Do not treat BTC-only high annualized return as satisfying the multi-symbol target.

## 12. Decision Rule

After the search:

- If at least one multi-symbol DGT candidate passes all offline gates, write a Phase 2 live-parity promotion design for only the mechanisms used by that candidate.
- If no candidate passes, report the DGT Pareto frontier and which requirements remain unmet.
- If a candidate passes only by concentrating return in one symbol or one segment, reject it as overfit even if the full-period numbers pass.
