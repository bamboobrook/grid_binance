# 2026-07-01 Dynamic Breakout/Trend Probe Design

## 1. Goal

Build a `research_only` dynamic breakout/trend portfolio probe to test whether a non-martingale return source is strong enough to justify later live-parity work.

The original user target remains unchanged:

- capital below `5000 USDT`;
- multi-symbol portfolio;
- anti-overfit and balanced `H1-2023`, `H2-2023`, `2024`, `2025`, and `2026_ytd` behavior;
- conservative `ann >50% / DD<=10%`;
- balanced `ann >90% / DD<=20%`;
- aggressive `ann >110% / DD<=30%`;
- live trading only after offline proof, live-parity design, and explicit user approval.

This probe does not redefine success. It only answers whether a separate trend/breakout sleeve deserves more work.

## 2. Current Evidence

Current evidence rejects the available martingale/grid paths under the original gates:

- `docs/superpowers/reports/2026-07-01-martingale-frontier-evidence-audit.md` indexes `16` reports with `0` machine-reported final/pass rows.
- `docs/superpowers/reports/2026-07-01-martingale-goal-completion-audit.md` says `Goal Complete: False`.
- `docs/superpowers/reports/2026-07-01-martingale-live-promotion-gate-audit.md` says `Promotion Allowed: False`.
- `docs/superpowers/reports/2026-07-01-new-return-source-decision.md` says repeated martingale/grid sweeps remain closed, and a new return source must first pass offline gates before live-parity work.

Existing trend evidence is promising but insufficient:

- `docs/superpowers/reports/2026-07-01-trend-sleeve-frontier-probe.md` scanned `1200` research-only rows and found `0` passes. The best observed annualized trend row had high DD: `43.24% ann / 63.21% DD`.
- `docs/superpowers/reports/2026-07-01-trend-risk-control-probe.md` scanned `14400` risk-control rows and still found `0` passes.
- `docs/superpowers/reports/2026-07-01-funding-sleeve-probe.md` found low-DD carry, but the top standalone funding stream was only `9.31% ann`.

The missing experiment is not another fixed equal-weight trend grid. It is a dynamic multi-symbol breakout/trend portfolio with explicit rolling selection and volatility targeting.

## 3. Probe Scope

The probe is an isolated Python research script and report. It must not modify Rust engines, trading code, live configuration, Binance credentials, flyingkid output, or real-funds paths.

Initial data:

- local SQLite market data: `data/market_data_full.db`;
- futures symbols already used by current trend probes: `BTCUSDT`, `ETHUSDT`, `BNBUSDT`, `SOLUSDT`, `INJUSDT`, `AAVEUSDT`, `LINKUSDT`, `DOGEUSDT`, `ADAUSDT`, `XRPUSDT`;
- optional extended universe from the 30-symbol funding coverage, only if the implementation can verify sufficient daily history per symbol.

Initial signals:

- Donchian breakout `20` and `60`;
- momentum `20` and `60`;
- EMA trend `20/50` and `50/200`;
- long/flat and long/short variants where the rule is unambiguous.

Initial portfolio controls:

- weekly or monthly rebalance;
- rolling signal-strength ranking;
- choose top `N` streams, with `N` at least `2`;
- max symbol weight cap;
- rolling realized-volatility target;
- portfolio DD stop and cooldown;
- explicit fee/slippage bps assumption;
- capital cap below `5000U`.

## 4. Architecture

Add one isolated script:

- `scripts/dynamic_breakout_trend_probe.py`

Responsibilities:

- load market bars from local SQLite;
- compress 1m bars into daily OHLC without lookahead;
- compute signal streams from completed daily bars only;
- rank streams using rolling, past-only statistics;
- build dynamic portfolio equity curves with weight caps and volatility target;
- apply optional portfolio DD stop/cooldown;
- compute full-period and segment metrics;
- evaluate C/B/A gates and anti-overfit gates;
- write JSON and Markdown outputs.

Add focused tests:

- `tests/verification/test_dynamic_breakout_trend_probe.py`

Responsibilities:

- prove daily compression uses deterministic OHLC grouping;
- prove signal generation does not use the current day's close for same-day entry;
- prove ranking uses only prior data;
- prove weight caps and top-N selection prevent single-symbol concentration;
- prove DD stop/cooldown behavior;
- prove gate evaluation rejects high-return/high-DD rows;
- prove outputs remain `research_only`.

Add report:

- `docs/superpowers/reports/2026-07-01-dynamic-breakout-trend-probe.md`

## 5. Data Flow

1. Load 1m OHLCV rows for each symbol.
2. Compress to UTC daily OHLC bars.
3. For each symbol/rule pair, build a unit-capital return stream using only completed prior daily bars for the next day's position.
4. At each rebalance date, rank symbol/rule streams by trailing risk-adjusted return and trend strength using only historical data available before the rebalance.
5. Select top `N` streams subject to symbol weight caps.
6. Scale weights to a rolling volatility target and capital cap.
7. Apply portfolio-level DD stop/cooldown if configured.
8. Compute equity metrics for full period and required segments.
9. Write machine-readable JSON rows and a concise Markdown frontier report.

## 6. Gate Rules

The probe must keep the original C/B/A gates:

| Profile | Annualized return | Max drawdown | Budget |
|---|---:|---:|---:|
| Conservative | `>50%` | `<=10%` | `<5000U` |
| Balanced | `>90%` | `<=20%` | `<5000U` |
| Aggressive | `>110%` | `<=30%` | `<5000U` |

Additional gates:

- symbol count must be at least `2`;
- no single symbol may exceed the configured weight cap;
- at least `4/5` required segments must be positive;
- combined `2024-2026` return must be positive;
- every output candidate must state `live_parity_status: research_only`;
- any pass-like row requires manual replay before it can influence live design.

## 7. Error Handling

Fail closed:

- missing or sparse data rejects the symbol/rule stream;
- zero or negative equity rejects the candidate;
- unknown signal mode rejects the run;
- unknown fee/slippage mode rejects the run;
- budget `>=5000U` rejects the candidate;
- a single-symbol selected portfolio cannot pass.

Reports should include rejection reasons rather than silently hiding failures.

## 8. Testing

Implementation must use TDD. Required tests:

- daily compression keeps daily open, high, low, and close deterministic;
- next-day execution prevents lookahead;
- rolling ranking excludes current and future bars;
- top-N selection enforces at least two symbols when enough valid symbols exist;
- max-weight cap is enforced;
- volatility target scales weights down when realized volatility is high;
- DD stop/cooldown freezes exposure and records risk events;
- C/B/A gate evaluation rejects high-DD candidates even with high annualized return;
- report output remains `research_only`.

## 9. Non-Goals

- Do not claim this is a martingale strategy.
- Do not start live trading.
- Do not touch Binance credentials.
- Do not publish to flyingkid.
- Do not modify Rust backtest or trading engines in this phase.
- Do not weaken original C/B/A thresholds.
- Do not promote any candidate to live based on this probe alone.

## 10. Decision Rule

After implementation:

- If no row passes or approaches conservative `>50% ann / DD<=10%`, do not build live-parity trend infrastructure yet.
- If a row passes offline C/B/A gates, write a separate live-parity promotion design for the exact rule family used by that row.
- If a row only passes by single-symbol concentration, future leakage, excessive leverage, or one-period overfit, reject it.

This keeps the original martingale/grid objective honest while allowing a clearly separated test of a new return source.
