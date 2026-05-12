# Martingale Auto Portfolio Search Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Convert the martingale backtest flow from manual unified parameters to risk-profile-driven per-symbol Top 5 search with automatic time ranges and a portfolio basket UI.

**Architecture:** Keep the existing `feature/full-v1` backtest APIs and worker. Extend the wizard payload with `risk_profile`, `per_symbol_top_n`, automatic time windows, and per-symbol search intent; extend the worker to group candidate outputs by symbol; extend the UI copy and review panel to make trailing TP semantics clear and let users stage a weighted basket from Top 5 candidates.

**Tech Stack:** Rust backtest worker/engine, Next.js React components, Node verification tests, Cargo unit tests.

---

## File Map

- Modify `apps/web/components/backtest/backtest-wizard.tsx`: automatic time range, risk-profile payload, per-symbol Top 5, payload helper tests via source contract.
- Modify `apps/web/components/backtest/martingale-parameter-editor.tsx`: rename trailing copy to moving take-profit retracement and explain it is not stop loss.
- Modify `apps/web/components/backtest/backtest-console.tsx`: pass selected candidates to basket/review area if needed and show grouped Top 5 metadata.
- Modify `apps/web/components/backtest/portfolio-candidate-review.tsx`: add weighted basket UI for selected candidates and weight total indicator.
- Modify `apps/backtest-worker/src/main.rs`: parse `per_symbol_top_n`/`risk_profile`, group ranked candidates by symbol, enrich summary metadata with rank/recommended weight/leverage.
- Modify `tests/verification/backtest_console_contract.test.mjs`: assert automatic time, Top 5, risk profile, trailing wording, and basket UI contracts.
- Modify `apps/backtest-worker/src/main.rs` tests: assert per-symbol Top 5 grouping and summary metadata.

---

### Task 1: Frontend Contract Tests

**Files:**
- Modify: `tests/verification/backtest_console_contract.test.mjs`

- [ ] **Step 1: Add failing assertions for auto-search behavior**

Add these assertions inside `backtest wizard is a real editable launcher, not a static template`:

```js
  assert.match(wizardSource, /per_symbol_top_n: 5/);
  assert.match(wizardSource, /risk_profile: form\.parameterPreset/);
  assert.match(wizardSource, /lastDayOfPreviousMonth/);
  assert.match(wizardSource, /trainStart: "2023-01-01"/);
  assert.match(wizardSource, /portfolio_basket/);
```

Add these source reads and assertions near the end of the same test:

```js
  const parameterSource = readFileSync(
    "apps/web/components/backtest/martingale-parameter-editor.tsx",
    "utf8",
  );
  assert.match(parameterSource, /移动止盈回撤|Moving take-profit retracement/);
  assert.match(parameterSource, /不是止损|not a stop loss/i);

  const reviewSource = readFileSync(
    "apps/web/components/backtest/portfolio-candidate-review.tsx",
    "utf8",
  );
  assert.match(reviewSource, /组合篮子|Portfolio basket/);
  assert.match(reviewSource, /权重合计|Weight total/);
  assert.match(reviewSource, /recommended_weight_pct/);
  assert.match(reviewSource, /recommended_leverage/);
```

- [ ] **Step 2: Run test to verify it fails**

Run: `node --test tests/verification/backtest_console_contract.test.mjs`

Expected: FAIL because the current source does not contain `per_symbol_top_n`, `lastDayOfPreviousMonth`, corrected trailing wording, or basket UI.

---

### Task 2: Worker Contract Tests

**Files:**
- Modify: `apps/backtest-worker/src/main.rs`

- [ ] **Step 1: Add failing worker unit test**

Inside the existing `#[cfg(test)] mod tests`, add:

```rust
    #[test]
    fn candidate_outputs_keep_top_five_per_symbol_and_enrich_summary() {
        let outputs = vec![
            candidate_output("BTCUSDT", "btc-1", 1, 90.0, 3),
            candidate_output("BTCUSDT", "btc-2", 2, 80.0, 3),
            candidate_output("BTCUSDT", "btc-3", 3, 70.0, 3),
            candidate_output("BTCUSDT", "btc-4", 4, 60.0, 3),
            candidate_output("BTCUSDT", "btc-5", 5, 50.0, 3),
            candidate_output("BTCUSDT", "btc-6", 6, 40.0, 3),
            candidate_output("ETHUSDT", "eth-1", 1, 30.0, 2),
        ];

        let selected = select_top_outputs_per_symbol(outputs, 5, "balanced");

        assert_eq!(selected.len(), 6);
        assert!(selected.iter().any(|output| output.candidate_id == "btc-5"));
        assert!(!selected.iter().any(|output| output.candidate_id == "btc-6"));
        let btc_first = selected.iter().find(|output| output.candidate_id == "btc-1").unwrap();
        assert_eq!(btc_first.summary["symbol"], "BTCUSDT");
        assert_eq!(btc_first.summary["parameter_rank_for_symbol"], 1);
        assert_eq!(btc_first.summary["recommended_weight_pct"], 20.0);
        assert_eq!(btc_first.summary["recommended_leverage"], 3);
        assert_eq!(btc_first.summary["risk_profile"], "balanced");
    }
```

Also add test helper:

```rust
    fn candidate_output(symbol: &str, id: &str, rank: usize, score: f64, leverage: u32) -> CandidateOutput {
        CandidateOutput {
            candidate_id: id.to_owned(),
            rank,
            score,
            config: serde_json::json!({
                "strategies": [{
                    "symbol": symbol,
                    "leverage": leverage,
                    "spacing": { "fixed_percent": { "step_bps": 100 } },
                    "sizing": { "multiplier": { "first_order_quote": "10", "multiplier": "1.5", "max_legs": 4 } },
                    "take_profit": { "percent": { "bps": 80 } }
                }]
            }),
            summary: serde_json::json!({}),
            artifact_path: format!("/tmp/{id}.json"),
            checksum_sha256: "sha256".to_owned(),
            used_trade_refinement: false,
            total_return_pct: score,
            max_drawdown_pct: 5.0,
            trade_count: 10,
        }
    }
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p backtest-worker candidate_outputs_keep_top_five_per_symbol -- --nocapture`

Expected: FAIL because `summary` does not exist on `CandidateOutput` and `select_top_outputs_per_symbol` is not implemented.

---

### Task 3: Implement Frontend Payload and Copy

**Files:**
- Modify: `apps/web/components/backtest/backtest-wizard.tsx`
- Modify: `apps/web/components/backtest/martingale-parameter-editor.tsx`

- [ ] **Step 1: Update automatic date helpers**

Change `resolveAutoTimeSplit()` so `trainStart` is always `2023-01-01`, `testEnd` is last day of previous month, and train/validate/test split the span approximately 70/15/15.

- [ ] **Step 2: Add payload fields**

In `buildWizardPayload()`, add:

```ts
    risk_profile: form.parameterPreset,
    per_symbol_top_n: 5,
    portfolio_basket: {
      mode: "manual_selection_after_backtest",
      weight_total_pct: 100,
      selection: [],
    },
```

- [ ] **Step 3: Update copy**

In `martingale-parameter-editor.tsx`, rename trailing field label to “移动止盈回撤 / Moving take-profit retracement” and add helper copy: “达到整体止盈后才激活，不是止损 / Activates only after take-profit, not a stop loss.”

- [ ] **Step 4: Run frontend contract test**

Run: `node --test tests/verification/backtest_console_contract.test.mjs`

Expected: PASS for frontend source contract after Task 4 basket UI is also complete; may still fail before Task 4.

---

### Task 4: Implement Portfolio Basket UI

**Files:**
- Modify: `apps/web/components/backtest/portfolio-candidate-review.tsx`

- [ ] **Step 1: Add basket state**

Add selected basket state derived from current candidate list or selected candidate. First version can stage only selected candidates visible in review, with editable `weightPct` and `leverage` fields.

- [ ] **Step 2: Render basket card**

Add a card titled “组合篮子 / Portfolio basket” with rows showing symbol, candidate id, parameters, `recommended_weight_pct`, `recommended_leverage`, editable weight, editable leverage, and “权重合计 / Weight total”.

- [ ] **Step 3: Weight total color**

Show green when total is exactly 100 within 0.01 tolerance, amber otherwise.

- [ ] **Step 4: Run frontend contract test**

Run: `node --test tests/verification/backtest_console_contract.test.mjs`

Expected: PASS.

---

### Task 5: Implement Worker Grouping and Summary Metadata

**Files:**
- Modify: `apps/backtest-worker/src/main.rs`

- [ ] **Step 1: Extend config**

Add to `WorkerTaskConfig`:

```rust
    #[serde(default = "default_per_symbol_top_n")]
    per_symbol_top_n: usize,
    #[serde(default = "default_risk_profile")]
    risk_profile: String,
```

Add helpers:

```rust
fn default_per_symbol_top_n() -> usize { 5 }
fn default_risk_profile() -> String { "balanced".to_owned() }
```

- [ ] **Step 2: Extend CandidateOutput**

Add:

```rust
    summary: serde_json::Value,
```

- [ ] **Step 3: Implement selection helper**

Add `select_top_outputs_per_symbol(outputs, per_symbol_top_n, risk_profile)` that sorts by score descending, keeps up to N per symbol, computes equal recommended weight per selected output, and enriches summary.

- [ ] **Step 4: Use helper before saving**

Before `save_candidates_and_artifacts`, replace `outputs` with:

```rust
let outputs = select_top_outputs_per_symbol(outputs, task.config.per_symbol_top_n.max(1), &task.config.risk_profile);
```

- [ ] **Step 5: Include summary in save**

When building `NewBacktestCandidateRecord`, merge `output.summary` into existing summary JSON.

- [ ] **Step 6: Run worker test**

Run: `cargo test -p backtest-worker candidate_outputs_keep_top_five_per_symbol -- --nocapture`

Expected: PASS.

---

### Task 6: Full Verification and Service Restart

**Files:**
- No source files expected.

- [ ] **Step 1: Run focused tests**

Run:

```bash
node --test tests/verification/backtest_console_contract.test.mjs tests/verification/backtest_worker_contract.test.mjs tests/verification/martingale_portfolio_contract.test.mjs
cargo test -p backtest-worker -- --nocapture
npm run build
```

Expected: all pass. If `npm run build` fails in sandbox due Turbopack binding port, rerun with escalation.

- [ ] **Step 2: Commit implementation**

Commit message must include `问题描述` / `复现路径` / `修复思路` per AGENTS.md.

- [ ] **Step 3: Restart grid services only**

Use docker compose for this repo only:

```bash
docker compose -f deploy/docker/docker-compose.yml --env-file .env up -d --build api-server web backtest-worker
```

Do not touch the unrelated host port 3000 service.

- [ ] **Step 4: Run one real backtest smoke**

Create a small martingale task through backend/API or existing test fixture using 2 symbols and `per_symbol_top_n: 5`. Verify task succeeds and candidates include `parameter_rank_for_symbol`, `recommended_weight_pct`, and `recommended_leverage`.

- [ ] **Step 5: Push branch**

Run:

```bash
git push origin feature/full-v1
```

Expected: remote `origin/feature/full-v1` advances to the implementation commit.
