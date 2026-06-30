# Hybrid Trend Rule Expansion Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Expand Phase 1 research-only trend sleeves beyond EMA20/50 long-flat by adding momentum, Donchian/high-low breakout, and long-short variants, then rerun bounded wide search against the original C/B/A gates.

**Architecture:** Extend `scripts/hybrid_martingale_frontier_probe.py` with rule-specific daily trend stream builders that preserve no-lookahead by using only previous completed daily data for signals. Extend `scripts/hybrid_frontier_wide_search.py` to precompute streams by `(rule, symbol)` and scan `--trend-rules`; outputs remain `research_only`.

**Tech Stack:** Python 3 standard library, existing SQLite daily-close loader, existing unit tests and Node contract tests.

---

## Task 1: Trend Rule Unit Tests

**Files:**
- Modify: `tests/verification/test_hybrid_frontier_probe_sample.py`
- Modify: `scripts/hybrid_martingale_frontier_probe.py`

- [ ] **Step 1: Add failing tests**

Append to `tests/verification/test_hybrid_frontier_probe_sample.py`:

```python
    def test_build_momentum_stream_can_go_long_short(self):
        db = self.tmp_path / "market_mom.db"
        day = 86_400_000
        closes = [100, 110, 121, 133, 120, 108, 97, 90, 99, 109, 120, 132]
        rows = [("BTCUSDT", "futures_usdt_perp", "1m", i * day, c, c, c, c, 1.0, i * day + 60_000 - 1) for i, c in enumerate(closes)]
        self.make_market_db(db, rows)
        stream = probe.build_momentum_stream(db, "BTCUSDT", allocation_quote=1000.0, lookback=2, mode="long_short", fee_bps=0.0)
        self.assertEqual(stream["name"], "trend:BTCUSDT:mom2_long_short")
        self.assertIs(stream["no_lookahead"], True)
        self.assertGreater(len(stream["points"]), 5)

    def test_build_donchian_stream_uses_previous_channel(self):
        db = self.tmp_path / "market_donchian.db"
        day = 86_400_000
        closes = [100, 101, 102, 99, 98, 103, 104, 97, 96, 105, 106]
        rows = [("BTCUSDT", "futures_usdt_perp", "1m", i * day, c, c, c, c, 1.0, i * day + 60_000 - 1) for i, c in enumerate(closes)]
        self.make_market_db(db, rows)
        stream = probe.build_donchian_stream(db, "BTCUSDT", allocation_quote=1000.0, lookback=3, mode="long_short", fee_bps=0.0)
        self.assertEqual(stream["name"], "trend:BTCUSDT:donchian3_long_short")
        self.assertIs(stream["no_lookahead"], True)
        self.assertGreater(len(stream["points"]), 5)
```

- [ ] **Step 2: Run tests to verify they fail**

Run:

```bash
python3 -m unittest tests/verification/test_hybrid_frontier_probe_sample.py
```

Expected: FAIL with missing `build_momentum_stream`.

- [ ] **Step 3: Implement rule builders**

Add to `scripts/hybrid_martingale_frontier_probe.py` after `build_trend_stream`:

```python
def build_momentum_stream(market_db: str | Path, symbol: str, allocation_quote: float, lookback: int = 20, mode: str = "long_flat", fee_bps: float = 2.0) -> dict:
    daily = load_daily_closes(market_db, symbol)
    equity = allocation_quote
    points = []
    position = 0
    for index in range(lookback + 1, len(daily)):
        prev_close = daily[index - 1]["close"]
        ref_close = daily[index - 1 - lookback]["close"]
        momentum = prev_close / ref_close - 1.0 if ref_close > 0 else 0.0
        desired = 1 if momentum > 0 else (-1 if mode == "long_short" and momentum < 0 else 0)
        if desired != position:
            equity *= 1.0 - fee_bps / 10_000.0
            position = desired
        last_close = daily[index - 1]["close"]
        close = daily[index]["close"]
        if last_close > 0:
            if position == 1:
                equity *= close / last_close
            elif position == -1:
                equity *= 2.0 - close / last_close
        points.append({"timestamp_ms": daily[index]["timestamp_ms"], "equity_quote": equity})
    return {
        "name": f"trend:{symbol}:mom{lookback}_{mode}",
        "kind": "trend",
        "symbols": [symbol],
        "points": points,
        "max_capital_used_quote": allocation_quote,
        "budget_blocked_events": 0,
        "fee_bps": fee_bps,
        "no_lookahead": True,
        "live_parity_status": LIVE_PARITY_STATUS,
    }


def build_donchian_stream(market_db: str | Path, symbol: str, allocation_quote: float, lookback: int = 20, mode: str = "long_flat", fee_bps: float = 2.0) -> dict:
    daily = load_daily_closes(market_db, symbol)
    equity = allocation_quote
    points = []
    position = 0
    for index in range(lookback + 1, len(daily)):
        window = [row["close"] for row in daily[index - 1 - lookback:index - 1]]
        upper = max(window)
        lower = min(window)
        signal_close = daily[index - 1]["close"]
        desired = 1 if signal_close > upper else (-1 if mode == "long_short" and signal_close < lower else position)
        if mode == "long_flat" and signal_close < lower:
            desired = 0
        if desired != position:
            equity *= 1.0 - fee_bps / 10_000.0
            position = desired
        last_close = daily[index - 1]["close"]
        close = daily[index]["close"]
        if last_close > 0:
            if position == 1:
                equity *= close / last_close
            elif position == -1:
                equity *= 2.0 - close / last_close
        points.append({"timestamp_ms": daily[index]["timestamp_ms"], "equity_quote": equity})
    return {
        "name": f"trend:{symbol}:donchian{lookback}_{mode}",
        "kind": "trend",
        "symbols": [symbol],
        "points": points,
        "max_capital_used_quote": allocation_quote,
        "budget_blocked_events": 0,
        "fee_bps": fee_bps,
        "no_lookahead": True,
        "live_parity_status": LIVE_PARITY_STATUS,
    }
```

- [ ] **Step 4: Run tests**

Run:

```bash
python3 -m unittest tests/verification/test_hybrid_frontier_probe_sample.py
node --test tests/verification/hybrid_frontier_probe_contract.test.mjs
python3 -m py_compile scripts/hybrid_martingale_frontier_probe.py
```

Expected: all pass.

- [ ] **Step 5: Commit**

```bash
git add scripts/hybrid_martingale_frontier_probe.py tests/verification/test_hybrid_frontier_probe_sample.py
git commit -m "feat: 修复思路 增加 momentum 和 Donchian 趋势 sleeve"
```

## Task 2: Wide Search Trend Rule Support

**Files:**
- Modify: `scripts/hybrid_frontier_wide_search.py`
- Modify: `tests/verification/test_hybrid_frontier_wide_search.py`

- [ ] **Step 1: Add failing test for rule parser**

Append to `tests/verification/test_hybrid_frontier_wide_search.py`:

```python
    def test_build_trend_key_names_rule_and_symbol(self):
        self.assertEqual(wide.trend_key("mom20_ls", "BTCUSDT"), "mom20_ls:BTCUSDT")
```

- [ ] **Step 2: Run test**

Run:

```bash
python3 -m unittest tests/verification/test_hybrid_frontier_wide_search.py
```

Expected: FAIL with missing `trend_key`.

- [ ] **Step 3: Implement rule dispatch**

Add to `scripts/hybrid_frontier_wide_search.py`:

```python
def trend_key(rule: str, symbol: str) -> str:
    return f"{rule}:{symbol}"


def build_rule_stream(market_data: str, symbol: str, rule: str) -> dict:
    if rule == "ema20_50_lf":
        return probe.build_trend_stream(market_data, symbol, 1.0, fast=20, slow=50)
    if rule == "ema50_200_lf":
        return probe.build_trend_stream(market_data, symbol, 1.0, fast=50, slow=200)
    if rule == "mom20_lf":
        return probe.build_momentum_stream(market_data, symbol, 1.0, lookback=20, mode="long_flat")
    if rule == "mom20_ls":
        return probe.build_momentum_stream(market_data, symbol, 1.0, lookback=20, mode="long_short")
    if rule == "mom60_ls":
        return probe.build_momentum_stream(market_data, symbol, 1.0, lookback=60, mode="long_short")
    if rule == "donchian20_lf":
        return probe.build_donchian_stream(market_data, symbol, 1.0, lookback=20, mode="long_flat")
    if rule == "donchian20_ls":
        return probe.build_donchian_stream(market_data, symbol, 1.0, lookback=20, mode="long_short")
    raise ValueError(f"unknown trend rule: {rule}")
```

Modify `parse_args()` to add:

```python
    parser.add_argument("--trend-rules", default="ema20_50_lf,mom20_lf,mom20_ls,mom60_ls,donchian20_lf,donchian20_ls")
```

Modify `run_search()`:

```python
    trend_rules = parse_csv(args.trend_rules)
    trend_base = {
        trend_key(rule, symbol): build_rule_stream(args.market_data, symbol, rule)
        for rule in trend_rules
        for symbol in trend_symbols
    }
```

When building streams, use the first rule for each selected group in a deterministic nested loop:

```python
                        for rule in trend_rules if t_alloc > 0 else ["none"]:
                            valid_trend_groups = [()] if t_alloc == 0 else [group for group in trend_groups if group]
                            ...
                                streams += [scale_stream(trend_base[trend_key(rule, symbol)], t_alloc) for symbol in trend_group]
```

Include `"trend_rule": rule` in candidate metadata.

- [ ] **Step 4: Run tests**

Run:

```bash
python3 -m unittest tests/verification/test_hybrid_frontier_probe_sample.py tests/verification/test_hybrid_frontier_wide_search.py
python3 -m py_compile scripts/hybrid_martingale_frontier_probe.py scripts/hybrid_frontier_wide_search.py
```

Expected: all pass.

- [ ] **Step 5: Commit**

```bash
git add scripts/hybrid_frontier_wide_search.py tests/verification/test_hybrid_frontier_wide_search.py
git commit -m "feat: 修复思路 扩展宽搜索趋势规则"
```

## Task 3: Enhanced Wide Search Report

**Files:**
- Create: `docs/superpowers/reports/2026-06-30-hybrid-frontier-trend-rules-search.md`

- [ ] **Step 1: Run enhanced search**

Run:

```bash
python3 scripts/hybrid_frontier_wide_search.py \
  --profiles conservative \
  --limit 1200 \
  --market-data /home/bumblebee/Project/grid_binance/data/market_data_full.db \
  --funding-data /home/bumblebee/Project/grid_binance/data/funding_rates.db \
  --out-json /tmp/hybrid_frontier_trend_rules_conservative.json \
  --out-md /tmp/hybrid_frontier_trend_rules_conservative.md
python3 scripts/hybrid_frontier_wide_search.py \
  --profiles balanced \
  --limit 1200 \
  --market-data /home/bumblebee/Project/grid_binance/data/market_data_full.db \
  --funding-data /home/bumblebee/Project/grid_binance/data/funding_rates.db \
  --out-json /tmp/hybrid_frontier_trend_rules_balanced.json \
  --out-md /tmp/hybrid_frontier_trend_rules_balanced.md
python3 scripts/hybrid_frontier_wide_search.py \
  --profiles aggressive \
  --limit 1200 \
  --market-data /home/bumblebee/Project/grid_binance/data/market_data_full.db \
  --funding-data /home/bumblebee/Project/grid_binance/data/funding_rates.db \
  --out-json /tmp/hybrid_frontier_trend_rules_aggressive.json \
  --out-md /tmp/hybrid_frontier_trend_rules_aggressive.md
```

- [ ] **Step 2: Generate combined report**

Use Python to combine `/tmp/hybrid_frontier_trend_rules_*.json` into `docs/superpowers/reports/2026-06-30-hybrid-frontier-trend-rules-search.md`, listing passes, best ann segment/cap, best DD segment/cap, and top near misses.

- [ ] **Step 3: Commit report**

```bash
git add docs/superpowers/reports/2026-06-30-hybrid-frontier-trend-rules-search.md
git commit -m "docs: 修复思路 记录趋势规则增强搜索结果"
```

