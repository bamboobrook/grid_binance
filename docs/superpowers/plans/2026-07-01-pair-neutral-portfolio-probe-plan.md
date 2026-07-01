# Pair-Neutral Portfolio Probe Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Build a research-only multi-pair pair-neutral portfolio probe to test whether diversification closes the original martingale/grid gates.

**Architecture:** Add one isolated Python script that reuses the existing pair-neutral stream builder and hybrid gate evaluator. Add focused unittest coverage for portfolio generation, stream combination, row conversion, gap ranking, and research-only output. Keep the search bounded and offline-only.

**Tech Stack:** Python 3 standard library (`argparse`, `importlib`, `itertools`, `json`, `pathlib`), `unittest`, existing local SQLite market data, existing research helper scripts.

---

## File Map

- Create `scripts/pair_neutral_portfolio_probe.py`
  - Owns multi-pair portfolio construction, overlap filtering, candidate ranking, CLI, and report writing.
- Create `tests/verification/test_pair_neutral_portfolio_probe.py`
  - Owns deterministic unit tests for the new probe helpers.
- Create `docs/superpowers/reports/2026-07-01-pair-neutral-portfolio-probe.md`
  - Final bounded search report.
- Modify `scripts/martingale_frontier_evidence_audit.py`
  - Add the new report to the evidence index after results exist.

Do not modify production trading code or live-trading code.

---

## Task 1: Core Portfolio Helpers

**Files:**
- Create: `tests/verification/test_pair_neutral_portfolio_probe.py`
- Create: `scripts/pair_neutral_portfolio_probe.py`

- [ ] **Step 1: Write failing tests for overlap filtering, stream combination, and row conversion**

Create `tests/verification/test_pair_neutral_portfolio_probe.py` with tests that import `scripts/pair_neutral_portfolio_probe.py` and check:

```python
def test_symbol_overlap_limit_rejects_reused_symbol():
    streams = [
        make_stream("ab", ["A", "B"], [100.0, 110.0], 500.0),
        make_stream("ac", ["A", "C"], [100.0, 108.0], 500.0),
    ]
    assert probe.symbol_overlap_ok(streams, max_symbol_uses=1) is False
```

Also include deterministic tests for:

- `portfolio_key()` includes both stream names;
- `build_portfolio()` combines equity to `200, 215` for two simple streams;
- `row_from_report()` includes `pairs`, `symbols`, `portfolio_size`, `live_parity_status`, and `gap_score`.

- [ ] **Step 2: Run tests and confirm RED**

Run:

```bash
python3 -m unittest tests/verification/test_pair_neutral_portfolio_probe.py
```

Expected: fail because `scripts/pair_neutral_portfolio_probe.py` does not exist.

- [ ] **Step 3: Implement minimal helpers**

Create `scripts/pair_neutral_portfolio_probe.py` with:

- imports for `pair_neutral_grid_probe.py` and `hybrid_martingale_frontier_probe.py`;
- `LIVE_PARITY_STATUS = "research_only"`;
- `symbols_for_stream(stream)`;
- `symbol_overlap_ok(streams, max_symbol_uses)`;
- `portfolio_key(streams)`;
- `build_portfolio(streams, budget)`;
- `gap_score(row)`;
- `row_from_report(profile, streams, report, meta)`.

- [ ] **Step 4: Run tests and confirm GREEN**

Run:

```bash
python3 -m unittest tests/verification/test_pair_neutral_portfolio_probe.py
python3 -m py_compile scripts/pair_neutral_portfolio_probe.py
```

Expected: tests pass and compile succeeds.

- [ ] **Step 5: Commit helper implementation**

Run:

```bash
git add scripts/pair_neutral_portfolio_probe.py tests/verification/test_pair_neutral_portfolio_probe.py
git commit -m "feat: 修复思路 增加多pair中性组合助手"
```

---

## Task 2: Search CLI And Report Writer

**Files:**
- Modify: `scripts/pair_neutral_portfolio_probe.py`
- Modify: `tests/verification/test_pair_neutral_portfolio_probe.py`

- [ ] **Step 1: Add failing tests for summary and markdown output**

Append tests that:

- create three fake rows and assert `summarize()` returns profile pass counts and nearest rows;
- call `write_outputs()` into a temp directory and assert markdown contains `research-only`, `live_parity_status`, row count, pass count, and no `live_parity_passed`.

- [ ] **Step 2: Run tests and confirm RED**

Run:

```bash
python3 -m unittest tests/verification/test_pair_neutral_portfolio_probe.py
```

Expected: fail because `summarize()` and `write_outputs()` are missing.

- [ ] **Step 3: Implement bounded search and output**

Add:

- `parse_csv()`, `parse_ints()`, `parse_floats()`;
- `build_pair_streams(args)` to create and preselect streams by profile-neutral annualized return;
- `run_search(args)` to combine stream portfolios for requested sizes and profiles;
- `summarize(rows)`;
- `write_outputs(result, out_json, out_md)`;
- `parse_args()` and `main()`.

The script must print JSON with `rows` and `passes`.

- [ ] **Step 4: Run tests and compile**

Run:

```bash
python3 -m unittest tests/verification/test_pair_neutral_portfolio_probe.py
python3 -m py_compile scripts/pair_neutral_portfolio_probe.py
```

Expected: pass.

- [ ] **Step 5: Commit CLI/report implementation**

Run:

```bash
git add scripts/pair_neutral_portfolio_probe.py tests/verification/test_pair_neutral_portfolio_probe.py
git commit -m "feat: 修复思路 增加多pair中性组合搜索"
```

---

## Task 3: Run Bounded Search And Index Evidence

**Files:**
- Create: `docs/superpowers/reports/2026-07-01-pair-neutral-portfolio-probe.md`
- Modify: `scripts/martingale_frontier_evidence_audit.py`
- Modify: `docs/superpowers/reports/2026-07-01-final-martingale-verdict-and-external-check.md`
- Modify: `docs/superpowers/reports/2026-07-01-martingale-frontier-evidence-audit.md`
- Modify: `docs/superpowers/reports/2026-07-01-martingale-goal-completion-audit.md`

- [ ] **Step 1: Run bounded search**

Run:

```bash
python3 scripts/pair_neutral_portfolio_probe.py \
  --profiles conservative,balanced,aggressive \
  --symbols BTCUSDT,ETHUSDT,BNBUSDT,SOLUSDT,XRPUSDT,ADAUSDT,DOGEUSDT,LINKUSDT \
  --allocations 500,1000,1500 \
  --lookbacks 20,40,80 \
  --entry-zs 1.0,1.5,2.0 \
  --portfolio-sizes 2,3,4 \
  --max-streams 24 \
  --max-portfolios 5000 \
  --out-json /tmp/pair_neutral_portfolio_probe.json \
  --out-md docs/superpowers/reports/2026-07-01-pair-neutral-portfolio-probe.md
```

Expected: exits 0 and reports pass count.

- [ ] **Step 2: Index report**

Modify `scripts/martingale_frontier_evidence_audit.py` to include:

```python
("pair_neutral_portfolio", "docs/superpowers/reports/2026-07-01-pair-neutral-portfolio-probe.md")
```

- [ ] **Step 3: Regenerate audits**

Run:

```bash
python3 scripts/martingale_frontier_evidence_audit.py --out-json /tmp/martingale_frontier_evidence_audit.json --out-md docs/superpowers/reports/2026-07-01-martingale-frontier-evidence-audit.md
python3 scripts/martingale_goal_completion_audit.py --out-json /tmp/martingale_goal_completion_audit.json --out-md docs/superpowers/reports/2026-07-01-martingale-goal-completion-audit.md
```

- [ ] **Step 4: Update final verdict**

Add one concise paragraph to `docs/superpowers/reports/2026-07-01-final-martingale-verdict-and-external-check.md` under the pair-neutral section summarizing the multi-pair result and whether it found any pass.

- [ ] **Step 5: Verify**

Run:

```bash
python3 -m unittest tests/verification/test_pair_neutral_portfolio_probe.py tests/verification/test_martingale_frontier_evidence_audit.py tests/verification/test_martingale_goal_completion_audit.py tests/verification/test_martingale_target_gap_audit.py
python3 -m py_compile scripts/pair_neutral_portfolio_probe.py scripts/martingale_frontier_evidence_audit.py
rg -n "TODO|TBD|placeholder|Fill this section|live_parity_passed" scripts/pair_neutral_portfolio_probe.py tests/verification/test_pair_neutral_portfolio_probe.py docs/superpowers/reports/2026-07-01-pair-neutral-portfolio-probe.md
git diff --check
```

Expected: tests pass, compile succeeds, scans return no unexpected matches, and diff check is clean.

- [ ] **Step 6: Commit final results**

Run:

```bash
git add scripts/pair_neutral_portfolio_probe.py tests/verification/test_pair_neutral_portfolio_probe.py scripts/martingale_frontier_evidence_audit.py docs/superpowers/reports/2026-07-01-pair-neutral-portfolio-probe.md docs/superpowers/reports/2026-07-01-final-martingale-verdict-and-external-check.md docs/superpowers/reports/2026-07-01-martingale-frontier-evidence-audit.md docs/superpowers/reports/2026-07-01-martingale-goal-completion-audit.md
git commit -m "docs: 修复思路 验证多pair中性网格组合"
```

---

## Self-Review

- Spec coverage: all design goals map to Tasks 1-3.
- Placeholder scan: no placeholder language is intentionally left for implementers.
- Type consistency: row fields use `profile`, `pairs`, `symbols`, `portfolio_size`, `ann`, `dd`, `cap`, `pos`, `c2426`, `pass`, `gap_score`, and `live_parity_status` throughout.
