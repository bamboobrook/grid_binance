# Martingale Backtest Usability Rebuild Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Rebuild the martingale backtest flow into a usable automatic search → per-symbol Top 5 → charts → portfolio basket → batch live publish workflow.

**Architecture:** Keep the existing route shape but replace the user-facing default flow with an automatic-search wizard. Persist martingale portfolio publish records in PostgreSQL instead of the current in-memory-only service, and expose a batch publish API that creates one portfolio with multiple strategy instances. Worker continues to produce candidate records, but summary/artifact contracts must include all UI fields needed for grouped Top 5 results and charts.

**Tech Stack:** Rust Axum API, `shared-db` SQLx/PostgreSQL repository layer, Rust `backtest-worker`, Next.js/React frontend, Node contract tests, Cargo tests, Docker Compose runtime.

---

## File Structure

- `docs/superpowers/specs/2026-05-12-martingale-backtest-usability-rebuild-design.md` — confirmed spec for this rebuild.
- `crates/shared-db/src/postgres/migrations.rs` — add persistent martingale portfolio tables.
- `crates/shared-db/src/backtest.rs` — add repository records/methods for portfolios, portfolio items, and candidate artifact lookup used by publish API.
- `apps/api-server/src/services/martingale_publish_service.rs` — replace in-memory publish state with DB-backed single/batch publish, validation, list/detail, lifecycle status changes.
- `apps/api-server/src/services/backtest_service.rs` — add `publish_portfolio_from_candidates()` and enforce candidate/task ownership.
- `apps/api-server/src/routes/backtest.rs` — add `POST /backtest/portfolios/publish`.
- `apps/api-server/src/routes/martingale_portfolios.rs` — ensure list/detail/lifecycle routes return DB-backed records with items.
- `apps/web/app/api/user/backtest/portfolios/publish/route.ts` — proxy batch publish API.
- `apps/web/app/api/user/martingale-portfolios/...` — add/verify proxies for list/detail/lifecycle operations if missing.
- `apps/web/lib/api-types.ts` — add automatic search, candidate chart, basket, batch publish, portfolio detail types.
- `apps/web/components/backtest/backtest-wizard.tsx` — default automatic-search form only: symbols, market, direction, risk profile, auto date display.
- `apps/web/components/backtest/backtest-console.tsx` — orchestrate selected task, progress polling, grouped results, charts, basket, publish success flow.
- `apps/web/components/backtest/backtest-task-list.tsx` — show clean empty state and human-readable task progress.
- `apps/web/components/backtest/backtest-result-table.tsx` — grouped per-symbol Top 5 table with add-to-basket actions.
- `apps/web/components/backtest/backtest-charts.tsx` — render real candidate equity/drawdown/comparison charts; show explicit missing-artifact state.
- `apps/web/components/backtest/portfolio-candidate-review.tsx` — portfolio basket, editable weights/leverage, batch publish button.
- `apps/web/components/backtest/live-portfolio-controls.tsx` — show published portfolio with strategy instances, lifecycle controls, disabled reasons.
- `apps/backtest-worker/src/main.rs` — ensure per-symbol Top 5, progress summary, chart artifact metadata, human risk summary, current date auto range support.
- `tests/verification/martingale_backtest_rebuild_contract.test.mjs` — source-level frontend/API contract checks.
- `tests/verification/martingale_portfolio_contract.test.mjs` — extend publish/list/detail contract checks.

---

## Task 1: Contract Tests for the Broken UX

**Files:**
- Create: `tests/verification/martingale_backtest_rebuild_contract.test.mjs`
- Modify: `tests/verification/martingale_portfolio_contract.test.mjs`

- [ ] **Step 1: Add source contract test for automatic-only default form**

Create `tests/verification/martingale_backtest_rebuild_contract.test.mjs` with:

```js
import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import test from "node:test";

const read = (path) => readFileSync(path, "utf8");

test("martingale wizard defaults to automatic search and current previous-month range", () => {
  const source = read("apps/web/components/backtest/backtest-wizard.tsx");
  assert.match(source, /AUTO_SEARCH_START_DATE\s*=\s*["']2023-01-01["']/);
  assert.match(source, /getLastDayOfPreviousMonth/);
  assert.match(source, /2026-04-30|lastDayOfPreviousMonth/);
  assert.match(source, /per_symbol_top_n:\s*5/);
  assert.match(source, /time_range_mode:\s*["']auto_previous_month_end["']/);
  assert.match(source, /risk_profile:\s*form\.parameterPreset/);
  assert.match(source, /开始自动搜索 Top 5|Start automatic Top 5 search/);
  assert.match(source, /高级参数搜索范围|Advanced parameter search space/);
  assert.doesNotMatch(source, /默认.*加仓间距|Default.*spacing/i);
});

test("backtest console exposes progress, grouped top five, charts, and basket publish", () => {
  const consoleSource = read("apps/web/components/backtest/backtest-console.tsx");
  const tableSource = read("apps/web/components/backtest/backtest-result-table.tsx");
  const chartSource = read("apps/web/components/backtest/backtest-charts.tsx");
  const basketSource = read("apps/web/components/backtest/portfolio-candidate-review.tsx");

  assert.match(consoleSource, /selectedTaskId/);
  assert.match(consoleSource, /poll|setInterval|refreshTask/i);
  assert.match(consoleSource, /groupCandidatesBySymbol|candidatesBySymbol/);
  assert.match(tableSource, /parameter_rank_for_symbol/);
  assert.match(tableSource, /加入组合|Add to basket/);
  assert.match(chartSource, /equity_curve|drawdown_curve/);
  assert.match(chartSource, /图表数据缺失|chart data is missing/i);
  assert.match(basketSource, /批量发布实盘组合|Batch publish live portfolio/);
  assert.match(basketSource, /weightTotal/);
  assert.match(basketSource, /publishPortfolio/);
});

test("frontend has proxy route for batch portfolio publish", () => {
  const route = read("apps/web/app/api/user/backtest/portfolios/publish/route.ts");
  assert.match(route, /backendPath:\s*["']\/backtest\/portfolios\/publish["']/);
  assert.match(route, /POST/);
});
```

- [ ] **Step 2: Extend portfolio contract for batch publish API**

Append to `tests/verification/martingale_portfolio_contract.test.mjs`:

```js
test("batch publish API and live portfolio UI expose multiple strategy instances", () => {
  const routeSource = readFileSync("apps/api-server/src/routes/backtest.rs", "utf8");
  const serviceSource = readFileSync("apps/api-server/src/services/martingale_publish_service.rs", "utf8");
  const uiSource = readFileSync("apps/web/components/backtest/live-portfolio-controls.tsx", "utf8");

  assert.match(routeSource, /\/backtest\/portfolios\/publish/);
  assert.match(serviceSource, /PublishPortfolioRequest/);
  assert.match(serviceSource, /PublishPortfolioItemRequest/);
  assert.match(serviceSource, /strategy_instance_id/);
  assert.match(serviceSource, /total_weight_pct/);
  assert.match(serviceSource, /candidate_id/);
  assert.match(uiSource, /strategy_instances|items/);
  assert.match(uiSource, /来源候选|Source candidate/);
});
```

- [ ] **Step 3: Run failing contract tests**

Run:

```bash
node --test tests/verification/martingale_backtest_rebuild_contract.test.mjs tests/verification/martingale_portfolio_contract.test.mjs
```

Expected: FAIL because batch publish proxy, DB-backed publish types, grouped result actions, and automatic-only default form are incomplete.

---

## Task 2: DB-Backed Portfolio Publish Model

**Files:**
- Modify: `crates/shared-db/src/postgres/migrations.rs`
- Modify: `crates/shared-db/src/backtest.rs`

- [ ] **Step 1: Add migration for persistent portfolio publish tables**

Add a migration block in `crates/shared-db/src/postgres/migrations.rs` that creates these tables if absent:

```sql
CREATE TABLE IF NOT EXISTS martingale_portfolios (
  portfolio_id TEXT PRIMARY KEY,
  owner TEXT NOT NULL,
  name TEXT NOT NULL,
  status TEXT NOT NULL,
  source_task_id TEXT NOT NULL REFERENCES backtest_tasks(task_id) ON DELETE CASCADE,
  market TEXT NOT NULL,
  direction TEXT NOT NULL,
  risk_profile TEXT NOT NULL,
  total_weight_pct NUMERIC NOT NULL,
  config JSONB NOT NULL DEFAULT '{}'::jsonb,
  risk_summary JSONB NOT NULL DEFAULT '{}'::jsonb,
  created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
  updated_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE TABLE IF NOT EXISTS martingale_portfolio_items (
  strategy_instance_id TEXT PRIMARY KEY,
  portfolio_id TEXT NOT NULL REFERENCES martingale_portfolios(portfolio_id) ON DELETE CASCADE,
  candidate_id TEXT NOT NULL REFERENCES backtest_candidate_summaries(candidate_id) ON DELETE RESTRICT,
  symbol TEXT NOT NULL,
  weight_pct NUMERIC NOT NULL,
  leverage INTEGER NOT NULL,
  enabled BOOLEAN NOT NULL DEFAULT true,
  status TEXT NOT NULL DEFAULT 'pending_confirmation',
  parameter_snapshot JSONB NOT NULL DEFAULT '{}'::jsonb,
  metrics_snapshot JSONB NOT NULL DEFAULT '{}'::jsonb,
  created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
  updated_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE INDEX IF NOT EXISTS idx_martingale_portfolios_owner_created
  ON martingale_portfolios(owner, created_at DESC);
CREATE INDEX IF NOT EXISTS idx_martingale_portfolio_items_portfolio
  ON martingale_portfolio_items(portfolio_id);
CREATE INDEX IF NOT EXISTS idx_martingale_portfolio_items_candidate
  ON martingale_portfolio_items(candidate_id);
```

If existing table names conflict, extend the existing tables rather than duplicating them. Keep old `martingale_portfolio_candidates` / `martingale_portfolio_publish_records` compatibility only if already used by migrations.

- [ ] **Step 2: Add DB records**

In `crates/shared-db/src/backtest.rs`, define:

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MartingalePortfolioRecord {
    pub portfolio_id: String,
    pub owner: String,
    pub name: String,
    pub status: String,
    pub source_task_id: String,
    pub market: String,
    pub direction: String,
    pub risk_profile: String,
    pub total_weight_pct: rust_decimal::Decimal,
    pub config: serde_json::Value,
    pub risk_summary: serde_json::Value,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub items: Vec<MartingalePortfolioItemRecord>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MartingalePortfolioItemRecord {
    pub strategy_instance_id: String,
    pub portfolio_id: String,
    pub candidate_id: String,
    pub symbol: String,
    pub weight_pct: rust_decimal::Decimal,
    pub leverage: i32,
    pub enabled: bool,
    pub status: String,
    pub parameter_snapshot: serde_json::Value,
    pub metrics_snapshot: serde_json::Value,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}
```

Use existing imports/types already present in the file; do not introduce a second decimal crate.

- [ ] **Step 3: Add repository methods**

Add methods on `BacktestRepository`:

```rust
pub fn create_martingale_portfolio(
    &self,
    portfolio: NewMartingalePortfolioRecord,
    items: Vec<NewMartingalePortfolioItemRecord>,
) -> Result<MartingalePortfolioRecord, SharedDbError>;

pub fn list_martingale_portfolios(
    &self,
    owner: &str,
) -> Result<Vec<MartingalePortfolioRecord>, SharedDbError>;

pub fn get_martingale_portfolio(
    &self,
    owner: &str,
    portfolio_id: &str,
) -> Result<Option<MartingalePortfolioRecord>, SharedDbError>;

pub fn set_martingale_portfolio_status(
    &self,
    owner: &str,
    portfolio_id: &str,
    status: &str,
) -> Result<Option<MartingalePortfolioRecord>, SharedDbError>;
```

Runtime SQL must run portfolio + items insertion in one transaction. Ephemeral backend must mirror behavior using the in-memory state so existing unit tests can still run.

- [ ] **Step 4: Add repository unit test**

In the existing `#[cfg(test)]` section, add an ephemeral test:

```rust
#[test]
fn martingale_portfolio_repository_round_trips_multiple_items() {
    let repo = BacktestRepository::ephemeral();
    repo.insert_task(NewBacktestTaskRecord {
        owner: "user@example.com".into(),
        strategy_type: "martingale_grid".into(),
        config: json!({}),
        summary: json!({}),
    }).unwrap();
    // Insert two candidate records for BTCUSDT with different candidate ids.
    // Create one portfolio containing both candidate ids.
    // Assert list/get return one portfolio with two distinct strategy_instance_id values.
}
```

Use the exact existing helper constructors in `backtest.rs`; do not invent fields that conflict with current structs.

- [ ] **Step 5: Run DB tests**

Run:

```bash
cargo test -p shared-db martingale_portfolio_repository_round_trips_multiple_items -- --nocapture
```

Expected: PASS.

---

## Task 3: Batch Publish API

**Files:**
- Modify: `apps/api-server/src/services/martingale_publish_service.rs`
- Modify: `apps/api-server/src/services/backtest_service.rs`
- Modify: `apps/api-server/src/routes/backtest.rs`
- Modify: `apps/api-server/src/routes/martingale_portfolios.rs`

- [ ] **Step 1: Add request/response types**

In `martingale_publish_service.rs`, add serializable types:

```rust
#[derive(Debug, Clone, Deserialize)]
pub struct PublishPortfolioRequest {
    pub name: String,
    pub task_id: String,
    pub market: String,
    pub direction: String,
    pub risk_profile: String,
    pub total_weight_pct: Decimal,
    pub items: Vec<PublishPortfolioItemRequest>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct PublishPortfolioItemRequest {
    pub candidate_id: String,
    pub symbol: String,
    pub weight_pct: Decimal,
    pub leverage: i32,
    #[serde(default = "default_enabled")]
    pub enabled: bool,
    pub parameter_snapshot: Value,
}

#[derive(Debug, Clone, Serialize)]
pub struct PublishPortfolioResponse {
    pub portfolio_id: String,
    pub status: String,
    pub source_task_id: String,
    pub items: Vec<PublishedStrategyInstance>,
    pub risk_summary: Value,
}

#[derive(Debug, Clone, Serialize)]
pub struct PublishedStrategyInstance {
    pub strategy_instance_id: String,
    pub candidate_id: String,
    pub symbol: String,
    pub weight_pct: Decimal,
    pub leverage: i32,
    pub status: String,
}
```

- [ ] **Step 2: Replace memory-only state for new publish path**

Change `MartingalePublishService` to store `BacktestRepository` or `SharedDb` repository handle. Keep `create_pending_portfolio()` for single-candidate compatibility, but internally persist to the DB-backed model with one item.

- [ ] **Step 3: Implement validation**

Add `publish_portfolio(owner, request, candidates)` that validates:

```rust
if request.items.is_empty() { return Err(PublishError::bad_request("portfolio must contain at least one strategy")); }
if request.total_weight_pct != Decimal::new(100, 0) { return Err(PublishError::bad_request("total weight must equal 100%")); }
let item_sum = request.items.iter().map(|i| i.weight_pct).sum::<Decimal>();
if item_sum != Decimal::new(100, 0) { return Err(PublishError::bad_request("item weights must sum to 100%")); }
for item in &request.items {
    if item.weight_pct <= Decimal::ZERO { return Err(PublishError::bad_request("item weight must be positive")); }
    if item.leverage < 1 || item.leverage > 125 { return Err(PublishError::bad_request("leverage must be between 1 and 125")); }
}
```

Also validate every candidate belongs to `request.task_id` and owner, and candidate config/summary has enough fields to form `parameter_snapshot`.

- [ ] **Step 4: Generate stable IDs and snapshots**

Use IDs:

```rust
let portfolio_id = format!("mp_{}", Uuid::new_v4().simple());
let strategy_instance_id = format!("msi_{}", Uuid::new_v4().simple());
```

For each item persist `parameter_snapshot` from request plus candidate config fallback, and `metrics_snapshot` from candidate summary/result fields.

- [ ] **Step 5: Wire route**

In `routes/backtest.rs`, import `PublishPortfolioRequest` and add:

```rust
.route("/backtest/portfolios/publish", post(publish_portfolio))
```

Handler:

```rust
async fn publish_portfolio(
    State(auth): State<AuthService>,
    State(service): State<BacktestService>,
    headers: HeaderMap,
    Json(request): Json<PublishPortfolioRequest>,
) -> Result<Json<PublishPortfolioResponse>, TaskBacktestError> {
    let session = require_user_session(&auth, &headers).map_err(TaskBacktestError::from)?;
    Ok(Json(service.publish_portfolio(&session.email, request)?))
}
```

- [ ] **Step 6: Update portfolio routes to DB-backed records**

Make `martingale_portfolios.rs` list/detail/lifecycle handlers call the DB-backed `MartingalePublishService` methods and return records with `items`, not only in-memory `config.strategies`.

- [ ] **Step 7: Add service tests**

Add tests proving:

- Two BTCUSDT candidate items can be published into one portfolio with two distinct `strategy_instance_id`s.
- Weight sum 90% returns 400.
- Candidate from another task/owner returns not found or bad request.

- [ ] **Step 8: Run API tests**

Run:

```bash
cargo test -p api-server martingale -- --nocapture
cargo test -p shared-db martingale -- --nocapture
```

Expected: PASS.

---

## Task 4: Worker Output Completeness

**Files:**
- Modify: `apps/backtest-worker/src/main.rs`

- [ ] **Step 1: Add/extend worker tests**

Add tests in `apps/backtest-worker/src/main.rs`:

```rust
#[test]
fn selected_outputs_include_ui_required_summary_fields() {
    let selected = select_top_outputs_per_symbol(
        vec![candidate_output("BTCUSDT", "btc-1", 1, 90.0, 3)],
        5,
        "balanced",
    );
    let summary = &selected[0].summary;
    assert_eq!(summary["symbol"], "BTCUSDT");
    assert!(summary.get("direction").is_some());
    assert!(summary.get("spacing_bps").is_some());
    assert!(summary.get("first_order_quote").is_some());
    assert!(summary.get("order_multiplier").is_some());
    assert!(summary.get("max_legs").is_some());
    assert!(summary.get("take_profit_bps").is_some());
    assert!(summary.get("trailing_take_profit_bps").is_some());
    assert!(summary.get("recommended_weight_pct").is_some());
    assert!(summary.get("recommended_leverage").is_some());
    assert!(summary.get("parameter_rank_for_symbol").is_some());
    assert!(summary.get("risk_summary_human").is_some());
    assert!(summary.get("equity_curve").is_some() || summary.get("artifact_path").is_some());
}
```

- [ ] **Step 2: Enrich summary consistently**

Update candidate summary enrichment so every saved candidate contains:

```json
{
  "symbol": "BTCUSDT",
  "direction": "long",
  "spacing_bps": 100,
  "first_order_quote": 10,
  "order_multiplier": 1.6,
  "max_legs": 5,
  "take_profit_bps": 100,
  "trailing_take_profit_bps": 40,
  "recommended_weight_pct": 20,
  "recommended_leverage": 3,
  "parameter_rank_for_symbol": 1,
  "risk_profile": "balanced",
  "total_return_pct": 12.3,
  "max_drawdown_pct": 4.5,
  "score": 87.6,
  "overfit_flag": false,
  "risk_summary_human": "BTCUSDT 在验证区间回撤可控，建议小权重试运行。",
  "artifact_path": "/...jsonl"
}
```

Do not fabricate chart curves. If only artifact path exists, UI/API must use that path or show missing-artifact state.

- [ ] **Step 3: Preserve per-symbol Top 5**

Ensure the refinement limit is at least `symbols.len() * per_symbol_top_n`, and final selection groups by symbol before truncating. Add assertion that two symbols each retain up to 5.

- [ ] **Step 4: Run worker tests**

Run:

```bash
cargo test -p backtest-worker -- --nocapture
```

Expected: PASS.

---

## Task 5: Frontend API Types and Proxies

**Files:**
- Modify: `apps/web/lib/api-types.ts`
- Create: `apps/web/app/api/user/backtest/portfolios/publish/route.ts`
- Verify/Create: `apps/web/app/api/user/martingale-portfolios/route.ts`
- Verify/Create: `apps/web/app/api/user/martingale-portfolios/[id]/route.ts`
- Verify/Create: lifecycle proxy routes under `apps/web/app/api/user/martingale-portfolios/[id]/...`

- [ ] **Step 1: Add TypeScript types**

In `api-types.ts`, add or extend:

```ts
export type MartingaleBacktestCandidateSummary = {
  symbol?: string;
  direction?: string;
  spacing_bps?: number;
  first_order_quote?: number;
  order_multiplier?: number;
  max_legs?: number;
  take_profit_bps?: number;
  trailing_take_profit_bps?: number;
  recommended_weight_pct?: number;
  recommended_leverage?: number;
  parameter_rank_for_symbol?: number;
  risk_profile?: string;
  total_return_pct?: number;
  max_drawdown_pct?: number;
  score?: number;
  overfit_flag?: boolean;
  risk_summary_human?: string;
  equity_curve?: Array<{ t: number | string; equity: number }>;
  drawdown_curve?: Array<{ t: number | string; drawdown: number }>;
  artifact_path?: string;
};

export type PublishPortfolioItemRequest = {
  candidate_id: string;
  symbol: string;
  weight_pct: number;
  leverage: number;
  enabled: boolean;
  parameter_snapshot: Record<string, unknown>;
};

export type PublishPortfolioRequest = {
  name: string;
  task_id: string;
  market: string;
  direction: string;
  risk_profile: string;
  total_weight_pct: number;
  items: PublishPortfolioItemRequest[];
};
```

Also add response/detail types with `strategy_instance_id` and `items`.

- [ ] **Step 2: Create batch publish proxy**

Create `apps/web/app/api/user/backtest/portfolios/publish/route.ts`:

```ts
import { proxyBacktestRequest } from "../../proxy";

export async function POST(request: Request) {
  return proxyBacktestRequest(request, {
    backendPath: "/backtest/portfolios/publish",
    method: "POST",
  });
}
```

Adjust relative import depth to compile in the actual folder.

- [ ] **Step 3: Verify portfolio proxies**

If missing, add proxy routes for:

- `GET /api/user/martingale-portfolios`
- `GET /api/user/martingale-portfolios/[id]`
- `POST /api/user/martingale-portfolios/[id]/confirm-start`
- `POST /api/user/martingale-portfolios/[id]/pause`
- `POST /api/user/martingale-portfolios/[id]/stop`

Use the same auth proxy pattern as existing backtest routes.

- [ ] **Step 4: Run TypeScript/source tests**

Run:

```bash
node --test tests/verification/martingale_backtest_rebuild_contract.test.mjs tests/verification/martingale_portfolio_contract.test.mjs
```

Expected: API proxy/type portions PASS; UI portions may still fail until Tasks 6–8.

---

## Task 6: Automatic Search Wizard UX

**Files:**
- Modify: `apps/web/components/backtest/backtest-wizard.tsx`
- Modify: `apps/web/components/backtest/search-config-editor.tsx`
- Modify: `apps/web/components/backtest/time-split-editor.tsx`
- Modify: `apps/web/components/backtest/martingale-parameter-editor.tsx`

- [ ] **Step 1: Fix date helper**

In `backtest-wizard.tsx`, define:

```ts
const AUTO_SEARCH_START_DATE = "2023-01-01";

function getLastDayOfPreviousMonth(now = new Date()): string {
  const year = now.getFullYear();
  const month = now.getMonth();
  const lastDay = new Date(year, month, 0);
  return `${lastDay.getFullYear()}-${String(lastDay.getMonth() + 1).padStart(2, "0")}-${String(lastDay.getDate()).padStart(2, "0")}`;
}
```

For current date `2026-05-12`, this returns `2026-04-30`. Add an exported/testable helper if existing tests import helpers.

- [ ] **Step 2: Replace default visible fields**

Default form shows only:

- whitelist symbols textarea/input, max 20.
- optional blacklist collapsed or secondary.
- market select.
- direction select.
- risk profile select.
- automatic date display.
- `开始自动搜索 Top 5` button.

Move manual martingale parameters into a collapsed card titled `高级参数搜索范围`.

- [ ] **Step 3: Build correct payload**

Ensure submit payload includes:

```ts
{
  strategy_type: "martingale_grid",
  symbols,
  config: {
    time_range_mode: "auto_previous_month_end",
    train_start: "2023-01-01",
    test_end: getLastDayOfPreviousMonth(),
    risk_profile: form.parameterPreset,
    per_symbol_top_n: 5,
    market: form.market,
    direction: form.direction,
    symbol_blacklist: blacklist,
    search_space_mode: "risk_profile_auto",
    portfolio_config: buildAutoPortfolioConfig(...)
  }
}
```

Use the exact existing `CreateBacktestTaskRequest` shape; if current API expects top-level `config`, keep new fields under `config` and duplicate critical worker fields only where existing worker reads them.

- [ ] **Step 4: Humanize trailing copy**

In `martingale-parameter-editor.tsx`, ensure wording says `移动止盈回撤` and `达到整体止盈后才激活，不是止损`.

- [ ] **Step 5: Add unit/source test for date helper**

If helpers can be imported, add a direct test. Otherwise keep source contract assertion and add a small inline comment-free helper that the contract can identify.

- [ ] **Step 6: Run frontend contract test**

Run:

```bash
node --test tests/verification/martingale_backtest_rebuild_contract.test.mjs
```

Expected: wizard assertions PASS.

---

## Task 7: Task Progress and Grouped Top 5 Results

**Files:**
- Modify: `apps/web/components/backtest/backtest-console.tsx`
- Modify: `apps/web/components/backtest/backtest-task-list.tsx`
- Modify: `apps/web/components/backtest/backtest-result-table.tsx`

- [ ] **Step 1: Keep user in current task context**

After task creation, set `selectedTaskId` to the returned task id and immediately show progress. Poll:

```ts
useEffect(() => {
  if (!selectedTaskId) return;
  const timer = window.setInterval(() => refreshTask(selectedTaskId), 3000);
  return () => window.clearInterval(timer);
}, [selectedTaskId]);
```

Stop polling only when status is `succeeded`, `failed`, or `cancelled`.

- [ ] **Step 2: Render human-readable progress**

Use task `summary` fields when available. Show fallbacks:

- status label.
- current phase.
- completed symbols / total symbols.
- evaluated candidates.
- elapsed time.
- last update.
- error message.

No raw JSON in the main view.

- [ ] **Step 3: Group candidates by symbol**

Implement:

```ts
function groupCandidatesBySymbol(candidates: BacktestCandidate[]): Record<string, BacktestCandidate[]> {
  return candidates.reduce((groups, candidate) => {
    const symbol = candidate.summary?.symbol ?? candidate.config?.strategies?.[0]?.symbol ?? "UNKNOWN";
    groups[symbol] = [...(groups[symbol] ?? []), candidate].sort((a, b) =>
      (a.summary?.parameter_rank_for_symbol ?? a.rank ?? 999) -
      (b.summary?.parameter_rank_for_symbol ?? b.rank ?? 999),
    ).slice(0, 5);
    return groups;
  }, {} as Record<string, BacktestCandidate[]>);
}
```

- [ ] **Step 4: Add `加入组合` action**

`backtest-result-table.tsx` must accept `onAddToBasket(candidate)` and show one button per candidate. The row must display parameter values and metrics in Chinese labels.

- [ ] **Step 5: Empty/error states**

If no tasks exist after cleanup, show `暂无回测任务，选择币种后开始自动搜索 Top 5`.
If task succeeded but no candidates, show `回测完成但没有可用候选：请检查行情数据覆盖范围或风险过滤条件`.

- [ ] **Step 6: Run source test**

Run:

```bash
node --test tests/verification/martingale_backtest_rebuild_contract.test.mjs
```

Expected: grouped/progress assertions PASS except chart/basket if not yet done.

---

## Task 8: Real Chart Rendering and Candidate Detail

**Files:**
- Modify: `apps/web/components/backtest/backtest-charts.tsx`
- Modify: `apps/web/components/backtest/backtest-console.tsx`
- Modify: `apps/web/components/backtest/backtest-result-table.tsx`

- [ ] **Step 1: Normalize chart data**

Add a helper:

```ts
function normalizeCandidateChartData(candidate: BacktestCandidate) {
  const summary = candidate.summary ?? {};
  return {
    equityCurve: Array.isArray(summary.equity_curve) ? summary.equity_curve : [],
    drawdownCurve: Array.isArray(summary.drawdown_curve) ? summary.drawdown_curve : [],
    artifactPath: typeof summary.artifact_path === "string" ? summary.artifact_path : undefined,
  };
}
```

- [ ] **Step 2: Render charts only from real data**

If `equityCurve.length > 0`, render equity chart. If `drawdownCurve.length > 0`, render drawdown chart. If both are empty but `artifactPath` exists, show `图表数据需要从 artifact 加载：<path>` and do not fake lines. If nothing exists, show `图表数据缺失：该候选没有保存资金曲线或回撤曲线`.

- [ ] **Step 3: Candidate comparison chart**

Add a simple candidate comparison using real candidate metrics:

- X: candidate rank/name.
- Bars/values: total_return_pct and max_drawdown_pct.

- [ ] **Step 4: Risk summary**

Candidate detail must show `risk_summary_human` if present. Fallback:

```ts
const fallbackRisk = `${symbol} 候选 #${rank}：收益 ${returnPct}%，最大回撤 ${drawdownPct}%，请结合验证区间和过拟合标记谨慎使用。`;
```

- [ ] **Step 5: Run frontend test**

Run:

```bash
node --test tests/verification/martingale_backtest_rebuild_contract.test.mjs
```

Expected: chart assertions PASS.

---

## Task 9: Portfolio Basket and Batch Publish UI

**Files:**
- Modify: `apps/web/components/backtest/portfolio-candidate-review.tsx`
- Modify: `apps/web/components/backtest/request-client.ts`
- Modify: `apps/web/components/backtest/backtest-console.tsx`

- [ ] **Step 1: Model basket items by strategy instance draft**

Add type:

```ts
type BasketItem = {
  localId: string;
  candidateId: string;
  taskId: string;
  symbol: string;
  weightPct: number;
  leverage: number;
  enabled: boolean;
  parameterSnapshot: Record<string, unknown>;
  metricsSnapshot: Record<string, unknown>;
};
```

Allow the same symbol multiple times as long as `candidateId` differs.

- [ ] **Step 2: Add item from candidate**

When user clicks `加入组合`, create item:

```ts
{
  localId: `${candidate.candidate_id}-${Date.now()}`,
  candidateId: candidate.candidate_id,
  taskId: candidate.task_id,
  symbol,
  weightPct: candidate.summary?.recommended_weight_pct ?? 0,
  leverage: candidate.summary?.recommended_leverage ?? 1,
  enabled: true,
  parameterSnapshot: buildParameterSnapshot(candidate),
  metricsSnapshot: candidate.summary ?? {},
}
```

Do not block adding BTCUSDT twice if candidate ids differ.

- [ ] **Step 3: Basket editing**

Render editable fields:

- portfolio name.
- enabled checkbox.
- weight percent.
- leverage.
- remove button.

Show weight total live. Disable publish unless enabled item weights sum to 100 within `0.01`.

- [ ] **Step 4: Batch publish client**

Add request function:

```ts
export async function publishMartingalePortfolio(payload: PublishPortfolioRequest) {
  return requestJson<PublishPortfolioResponse>("/api/user/backtest/portfolios/publish", {
    method: "POST",
    body: JSON.stringify(payload),
  });
}
```

Use existing request helper naming/style.

- [ ] **Step 5: Publish action**

On click, submit:

```ts
{
  name,
  task_id: selectedTaskId,
  market,
  direction,
  risk_profile,
  total_weight_pct: 100,
  items: enabledBasketItems.map(...),
}
```

Show loading, success with `portfolio_id`, partial/validation errors as Chinese messages, and link to `/zh/app/martingale-portfolios/{portfolio_id}` or current locale equivalent.

- [ ] **Step 6: Run contract test**

Run:

```bash
node --test tests/verification/martingale_backtest_rebuild_contract.test.mjs tests/verification/martingale_portfolio_contract.test.mjs
```

Expected: basket/publish assertions PASS.

---

## Task 10: Portfolio Detail UI for Published Instances

**Files:**
- Modify: `apps/web/components/backtest/live-portfolio-controls.tsx`
- Modify: `apps/web/app/[locale]/app/martingale-portfolios/page.tsx`
- Modify: `apps/web/app/[locale]/app/martingale-portfolios/[id]/page.tsx`

- [ ] **Step 1: Show portfolio list**

List must display:

- portfolio name/id.
- status.
- market/direction/risk profile.
- item count.
- total weight.
- created time.
- link to detail.

If list empty, show `暂无实盘马丁组合，可先从回测结果篮子批量发布`.

- [ ] **Step 2: Show strategy instances**

Detail page must display each item:

- `strategy_instance_id`.
- symbol.
- source candidate.
- weight.
- leverage.
- status.
- parameter summary.
- metrics snapshot.

- [ ] **Step 3: Lifecycle controls with clear disabled reasons**

Buttons:

- Start / confirm-start.
- Pause.
- Stop.

If real exchange execution is not wired, label the start action as `确认启用组合记录` or show `实盘自动下单需连接策略执行器后启用` depending on backend capability. Never silently no-op.

- [ ] **Step 4: Statistics hierarchy labels**

Add visible labels for:

- 组合级统计。
- 币种级统计。
- 策略实例级统计。

If live stats are unavailable, show planned/empty state rather than fake numbers.

- [ ] **Step 5: Run frontend build**

Run:

```bash
npm run build
```

Expected: PASS.

---

## Task 11: Cleanup Command and Runtime Smoke Test

**Files:**
- No code changes unless cleanup script already exists and needs a safe backtest-only command.

- [ ] **Step 1: Verify old tasks are empty**

Run against Docker PostgreSQL:

```bash
docker compose -f deploy/docker/docker-compose.yml --env-file .env exec -T postgres psql -U postgres -d grid_binance -c "SELECT 'backtest_tasks' AS table_name, count(*) FROM backtest_tasks UNION ALL SELECT 'backtest_candidate_summaries', count(*) FROM backtest_candidate_summaries UNION ALL SELECT 'backtest_artifacts', count(*) FROM backtest_artifacts UNION ALL SELECT 'backtest_task_events', count(*) FROM backtest_task_events;"
```

Expected: all counts are `0` before the new smoke run.

- [ ] **Step 2: Rebuild/restart services**

Run:

```bash
docker compose -f deploy/docker/docker-compose.yml --env-file .env up -d --build api-server web backtest-worker
```

Expected: `api-server`, `web`, and `backtest-worker` are running. Do not touch unrelated host port 3000 service.

- [ ] **Step 3: Create a real two-symbol backtest**

Use authenticated UI if available. If not practical in CLI, insert through the API with a valid session or use a controlled DB/API smoke helper that creates a task equivalent to the UI payload:

```json
{
  "strategy_type": "martingale_grid",
  "symbols": ["BTCUSDT", "ETHUSDT"],
  "config": {
    "time_range_mode": "auto_previous_month_end",
    "train_start": "2023-01-01",
    "test_end": "2026-04-30",
    "risk_profile": "balanced",
    "per_symbol_top_n": 5,
    "market": "usdm_futures",
    "direction": "long_short",
    "search_space_mode": "risk_profile_auto"
  }
}
```

- [ ] **Step 4: Wait for completion**

Poll task until `succeeded` or `failed`. If failed, collect worker/API logs and fix root cause before proceeding.

- [ ] **Step 5: Verify Top 5 output**

Query candidates and assert:

- BTCUSDT has 5 candidates, unless market data truly lacks enough valid candidates; if fewer, UI must say why.
- ETHUSDT has 5 candidates, same exception.
- Each candidate has required summary fields.
- At least one candidate has chart data or explicit artifact reference.

- [ ] **Step 6: Verify batch publish**

Select at least two candidates, one BTCUSDT and one ETHUSDT, with weights totaling 100. Call `POST /backtest/portfolios/publish` or use the UI. Assert response includes:

- `portfolio_id`.
- at least 2 `strategy_instance_id`s.
- items reference candidate ids.
- portfolio detail page/API returns the same items.

---

## Task 12: Full Verification and Commit

**Files:**
- All changed files.

- [ ] **Step 1: Run all targeted verification**

Run:

```bash
node --test tests/verification/backtest_console_contract.test.mjs tests/verification/backtest_worker_contract.test.mjs tests/verification/martingale_portfolio_contract.test.mjs tests/verification/martingale_backtest_rebuild_contract.test.mjs
cargo test -p shared-db martingale -- --nocapture
cargo test -p api-server martingale -- --nocapture
cargo test -p backtest-worker -- --nocapture
npm run build
```

Expected: PASS. If unrelated warnings appear, note them but do not fix unrelated code.

- [ ] **Step 2: Manual UI verification**

Open `http://127.0.0.1:8080/zh/app/backtest` and verify:

- Default page shows only symbols/market/direction/risk profile/date.
- Date shows `2023-01-01 → 2026-04-30` on 2026-05-12.
- Task progress is visible.
- Grouped Top 5 and chart states are visible.
- Basket add/edit/publish works.
- Portfolio detail page shows items.

- [ ] **Step 3: Check git status**

Run:

```bash
git status --short
```

Expected: only intentional files changed.

- [ ] **Step 4: Commit**

Commit with required Chinese log fields:

```bash
git add docs/superpowers/specs/2026-05-12-martingale-backtest-usability-rebuild-design.md docs/superpowers/plans/2026-05-12-martingale-backtest-usability-rebuild-plan.md tests/verification/martingale_backtest_rebuild_contract.test.mjs tests/verification/martingale_portfolio_contract.test.mjs crates/shared-db/src/postgres/migrations.rs crates/shared-db/src/backtest.rs apps/api-server/src/services/martingale_publish_service.rs apps/api-server/src/services/backtest_service.rs apps/api-server/src/routes/backtest.rs apps/api-server/src/routes/martingale_portfolios.rs apps/web apps/backtest-worker/src/main.rs
git commit -m "feat: 问题描述 重构马丁回测自动搜索与批量发布"
```

- [ ] **Step 5: Push only after tests pass**

Run:

```bash
git push origin feature/full-v1
```

Expected: push succeeds.
