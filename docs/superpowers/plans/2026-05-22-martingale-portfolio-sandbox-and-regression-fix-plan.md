# Martingale Portfolio Sandbox and Regression Fix Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Fix martingale backtest regressions, restore high-yield candidate discovery, add Top50 recommended symbols, interactive wide charts, portfolio sandbox recalculation, and verified live publish handoff in this release.

**Architecture:** Keep single-strategy backtest as the source of truth. Persist enough per-candidate curves/config to power both automatic portfolio search and user-edited portfolio sandbox recalculation. Add a backend recalculation endpoint that combines already-backtested candidates by user-selected weights/leverage, then let the frontend display and publish the resulting final portfolio.

**Tech Stack:** Rust `api-server`, `backtest-worker`, `backtest-engine`, `shared-db`; Next.js/React frontend; PostgreSQL task/candidate/portfolio storage; SQLite read-only local market data.

---

## File Map

- Modify: `apps/backtest-engine/src/sqlite_market_data.rs` — change recommended liquid symbols limit/query semantics to Top50 futures symbols with 2023-01-01 coverage and recent data.
- Modify: `apps/api-server/src/routes/backtest.rs` — return Top50 recommended symbols and expose portfolio recalculation route.
- Modify: `apps/api-server/src/services/backtest_service.rs` — implement portfolio sandbox recalculation service, candidate ownership validation, curve loading/normalization, and response models.
- Modify: `apps/api-server/tests/martingale_backtest_flow.rs` — add API-level tests for recommended symbols and sandbox recalculation.
- Modify: `apps/backtest-worker/src/main.rs` — fix market inheritance, candidate retention, candidate summary fields, and result diagnostics.
- Modify: `apps/backtest-worker/src/main.rs` tests section — add unit tests for futures market enforcement, per-symbol retention, and summary leverage fields.
- Modify: `apps/backtest-engine/src/portfolio_search.rs` — tune portfolio objective to maximize return/drawdown and use member count as soft reward, not a hard target.
- Modify: `apps/backtest-engine/tests/search_scoring_time_splits.rs` — add/adjust portfolio objective regression tests.
- Modify: `apps/web/components/backtest/backtest-wizard.tsx` — change recommended button wording and Top50 behavior.
- Modify: `apps/web/components/backtest/backtest-charts.tsx` — replace narrow sparkline with wide hoverable SVG chart component.
- Modify: `apps/web/components/backtest/backtest-console.tsx` — wire portfolio sandbox state, edit selected portfolio, add candidate, recalculate, display sandbox result.
- Modify: `apps/web/components/backtest/backtest-result-table.tsx` — show candidate leverage/legs/market clearly and add edit-combination action.
- Modify: `apps/web/components/backtest/portfolio-candidate-review.tsx` — publish sandbox final payload and show publish validation.
- Modify: `apps/web/lib/api-types.ts` — add sandbox recalculation response and richer candidate/portfolio fields.
- Create/modify: `apps/web/app/api/user/backtest/portfolios/recalculate/route.ts` — Next proxy route to backend.
- Modify: `apps/api-server/src/services/martingale_publish_service.rs` — verify custom sandbox payload preserves complete `parameter_snapshot` and confirm-start keeps it.
- Modify: relevant service tests in `apps/api-server/src/services/martingale_publish_service.rs` — add publish handoff regression tests.

---

## Task 1: Top50 Recommended Symbol Pool

**Files:**
- Modify: `apps/backtest-engine/src/sqlite_market_data.rs`
- Modify: `apps/api-server/src/routes/backtest.rs`
- Modify: `apps/web/components/backtest/backtest-wizard.tsx`
- Test: `apps/backtest-engine/src/sqlite_market_data.rs`

- [ ] **Step 1: Update recommended symbol query contract**

In `apps/backtest-engine/src/sqlite_market_data.rs`, change `recommended_liquid_symbols` callers to pass `limit=50`, and ensure query returns all matching symbols up to 50.

For Discord schema query, keep these exact conditions:

```sql
SELECT u.symbol
FROM market_universe u
WHERE u.market_type = 'futures_usdt_perp'
  AND u.quote_asset = 'USDT'
  AND EXISTS (
    SELECT 1 FROM klines k
    WHERE k.symbol = u.symbol
      AND k.market_type = 'futures_usdt_perp'
      AND k.timeframe = '1m'
      AND k.open_time BETWEEN ?1 AND ?2
    LIMIT 1
  )
  AND EXISTS (
    SELECT 1 FROM klines k
    WHERE k.symbol = u.symbol
      AND k.market_type = 'futures_usdt_perp'
      AND k.timeframe = '1m'
      AND k.open_time >= ?3
    LIMIT 1
  )
ORDER BY COALESCE(u.volume_24h, 0) DESC, u.symbol ASC
LIMIT ?4
```

- [ ] **Step 2: Expand fixture test to prove Top50-style ordering**

Add test rows to `discord_c2im_fixture_db()`:

```sql
('SOLUSDT', 'futures_usdt_perp', 'SOL', 'USDT', 1200.0, '2026-05-22'),
('BNBUSDT', 'futures_usdt_perp', 'BNB', 'USDT', 800.0, '2026-05-22')
```

Add corresponding early and recent 1m klines for `SOLUSDT` and `BNBUSDT`.

Update/add test:

```rust
#[test]
fn recommended_liquid_symbols_are_futures_usdt_ranked_by_volume_with_coverage() {
    let file = discord_c2im_fixture_db();
    let source = SqliteMarketDataSource::open_readonly(file.path())
        .expect("open readonly discord schema");

    let symbols = source
        .recommended_liquid_symbols(1000, 4000, 50)
        .expect("recommended symbols");

    assert_eq!(symbols, vec!["SOLUSDT", "ETHUSDT", "BNBUSDT"]);
    assert!(!symbols.contains(&"OLDUSDT".to_owned()));
    assert!(!symbols.contains(&"SPOTUSDT".to_owned()));
}
```

Expected before fix: existing test only proves a small static subset and route still returns 18. Expected after fix: test passes and ordering follows `volume_24h`.

- [ ] **Step 3: Update API limit and response metadata**

In `apps/api-server/src/routes/backtest.rs`, change:

```rust
const LIMIT: usize = 18;
```

to:

```rust
const LIMIT: usize = 50;
```

Change response metadata to include `limit: 50` and rename UI text source if needed:

```rust
#[derive(Debug, Serialize)]
struct RecommendedSymbolsResponse {
    symbols: Vec<String>,
    source: String,
    min_start_date: String,
    market_type: String,
    interval: String,
    limit: usize,
}
```

Return `limit: LIMIT`.

- [ ] **Step 4: Update frontend wording**

In `apps/web/components/backtest/backtest-wizard.tsx`, rename button text:

```tsx
{recommendedPending ? pickText(lang, "查询中…", "Loading...") : pickText(lang, "查询并填入推荐 Top50 币种", "Load recommended Top 50 symbols")}
```

Update help text:

```tsx
{pickText(
  lang,
  "推荐池来自本地 1m 行情库：覆盖 2023-01-01 起数据、近期仍有数据、按 USDT 合约成交活跃度取前 50；可手动删减。",
  "The recommended pool uses local 1m data coverage since 2023-01-01, recent futures data, and the top 50 USDT futures by liquidity; you can edit manually.",
)}
```

- [ ] **Step 5: Run tests**

Run:

```bash
cargo test -p backtest-engine recommended_liquid_symbols -- --nocapture
cargo check -p api-server
cd apps/web && npx tsc --noEmit --pretty false
```

Expected: all pass.

- [ ] **Step 6: Commit**

```bash
git add apps/backtest-engine/src/sqlite_market_data.rs apps/api-server/src/routes/backtest.rs apps/web/components/backtest/backtest-wizard.tsx
git commit -m "fix: 推荐回测币种改为Top50动态池" \
  -m "问题描述：推荐币种入口错误固定为18个，不能从2023-01-01起有数据且成交活跃的前50合约中选择。" \
  -m "修复思路：按本地行情库覆盖范围和volume_24h排序返回Top50，并更新前端按钮与说明。"
```

---

## Task 2: Fix Candidate Retention and Futures Market Inheritance

**Files:**
- Modify: `apps/backtest-worker/src/main.rs`
- Test: `apps/backtest-worker/src/main.rs`

- [ ] **Step 1: Add failing tests for market inheritance**

In the `#[cfg(test)]` section of `apps/backtest-worker/src/main.rs`, add a test that builds a `WorkerTaskConfig` with:

```rust
market: Some("usd_m_futures".to_owned()),
margin_mode: Some("isolated".to_owned()),
direction_mode: Some("long_and_short".to_owned()),
```

Generate a candidate via the same helper used by existing staged-search tests. Assert every strategy has:

```rust
assert_eq!(strategy.market, MartingaleMarketKind::UsdMFutures);
assert_eq!(strategy.margin_mode, MartingaleMarginMode::Isolated);
assert!(strategy.leverage.unwrap_or(0) >= 1);
```

Expected before fix: a fallback or generated candidate can contain `Spot` or empty leverage. Expected after fix: all legs match task market.

- [ ] **Step 2: Enforce task market/margin/leverage after candidate generation**

Add helper:

```rust
fn enforce_task_execution_model(mut candidate: SearchCandidate, task: &WorkerTaskConfig) -> SearchCandidate {
    let market = market_kind(task.market.as_deref());
    let margin_mode = margin_mode(task.margin_mode.as_deref());
    let default_leverage = task
        .leverage_range
        .map(|range| range[0].max(1))
        .unwrap_or(1);

    for strategy in &mut candidate.config.strategies {
        if let Some(market) = market {
            strategy.market = market;
        }
        if let Some(margin_mode) = margin_mode {
            strategy.margin_mode = margin_mode;
        }
        if matches!(strategy.market, MartingaleMarketKind::UsdMFutures) && strategy.leverage.is_none() {
            strategy.leverage = Some(default_leverage);
        }
    }
    candidate
}
```

Call this helper in every path before screening/refinement:

```rust
let overridden = enforce_task_execution_model(apply_task_overrides_to_candidate(candidate.clone(), task), task);
```

Also call it before serializing `overridden_candidate.config` into `CandidateOutput`.

- [ ] **Step 3: Preserve per-symbol candidates before strict display filtering**

Find `select_top_outputs_per_symbol`. Current logic removes candidates if:

```rust
if output.total_return_pct <= 0.0 { return false; }
if output.max_drawdown_pct > output.used_drawdown_limit_pct { return false; }
```

Change behavior:

- Final publishable candidate list: positive return and within drawdown limit.
- Diagnostic candidate list: best positive candidate per missing symbol even if drawdown is above limit, marked `risk_relaxed=true` or `publishable=false`.
- Do not silently drop symbols. If no positive result exists for a symbol, write symbol diagnostics into task summary.

Implement by adding `publishable` to `CandidateOutput.summary`:

```rust
"publishable": output.total_return_pct > 0.0 && output.max_drawdown_pct <= output.used_drawdown_limit_pct,
"candidate_warning": if output.max_drawdown_pct > output.used_drawdown_limit_pct { "drawdown_above_limit" } else { "" },
```

Keep non-publishable diagnostics visible but disable publish/add-to-basket in frontend later.

- [ ] **Step 4: Ensure display saves up to Top10 per symbol**

Replace display selection logic with:

```rust
let display_outputs = select_display_outputs_per_symbol(
    outputs.clone(),
    &task.config.symbols,
    task.config.per_symbol_top_n.max(10),
    &task.config.risk_profile,
);
```

`select_display_outputs_per_symbol` must:

1. Group by symbol.
2. Sort each group by publishable first, annualized return descending, drawdown ascending.
3. Take up to Top10 per symbol.
4. If a symbol has zero outputs, it remains absent from candidates but appears in `missing_symbol_diagnostics` summary.

Add summary fields:

```rust
"requested_symbols": task.config.symbols,
"display_candidate_symbol_count": display_symbols.len(),
"missing_symbols": missing_symbols,
"missing_symbol_diagnostics": diagnostics.by_symbol,
```

- [ ] **Step 5: Add summary fields for leverage and legs**

In candidate summary merge, include:

```rust
"market": output_market(&output),
"max_leverage_used": output.max_leverage_used.unwrap_or(recommended_leverage as f64),
"long_short_legs": long_short_leg_summary_from_config(&output.config),
"publishable": publishable,
```

Add helper:

```rust
fn output_market(output: &CandidateOutput) -> String {
    let strategies = output.config.pointer("/strategies").and_then(Value::as_array);
    let has_futures = strategies
        .into_iter()
        .flatten()
        .any(|s| s.get("market").and_then(Value::as_str) == Some("usd_m_futures"));
    if has_futures { "usd_m_futures".to_owned() } else { "spot".to_owned() }
}
```

- [ ] **Step 6: Run targeted worker tests**

Run:

```bash
cargo test -p backtest-worker market_inheritance -- --nocapture
cargo test -p backtest-worker candidate_retention -- --nocapture
cargo check -p backtest-worker
```

Expected: tests pass; no candidate with `spot` when task market is futures.

- [ ] **Step 7: Commit**

```bash
git add apps/backtest-worker/src/main.rs
git commit -m "fix: 修复马丁候选保留与合约市场继承" \
  -m "问题描述：long+short合约回测只保存少数币种候选且混入spot，导致前端看不到每币种Top10和杠杆信息。" \
  -m "修复思路：候选生成后强制继承任务market/margin/leverage，显示层按币种保留Top10并输出缺失诊断。"
```

---

## Task 3: Restore Profit-Oriented Search and Portfolio Objective

**Files:**
- Modify: `apps/backtest-worker/src/main.rs`
- Modify: `apps/backtest-engine/src/portfolio_search.rs`
- Test: `apps/backtest-engine/tests/search_scoring_time_splits.rs`

- [ ] **Step 1: Add regression test for high-yield candidate not being demoted**

In `apps/backtest-engine/tests/search_scoring_time_splits.rs`, add a portfolio/candidate ranking test with candidates:

- `BTCUSDT`: annualized 68.0, drawdown 19.8, return 230.0
- `XRPUSDT`: annualized 60.0, drawdown 15.2, return 200.0
- `DOGEUSDT`: annualized 22.0, drawdown 24.0, return 99.0
- `LINKUSDT`: annualized 18.0, drawdown 10.0, return 73.0

Assert that within `max_drawdown_pct=20`, BTC/XRP are included before lower-return stabilizers unless correlation/concentration rules force a different allocation.

- [ ] **Step 2: Adjust candidate sorting for balanced/conservative**

Current `sort_outputs_for_profile` uses `score` for non-aggressive profiles. Change to use a profit-aware score under hard drawdown limit:

```rust
fn output_rank_score(output: &CandidateOutput, risk_profile: &str) -> f64 {
    let annualized = output.annualized_return_pct.unwrap_or(output.total_return_pct);
    let drawdown = output.max_drawdown_pct.max(1.0);
    let ratio = output.return_drawdown_ratio.unwrap_or(annualized / drawdown);
    let stability_penalty = if output.risk_relaxed { 8.0 } else { 0.0 };
    let conservative_penalty = if risk_profile == "conservative" { drawdown * 0.35 } else { drawdown * 0.2 };
    annualized * 1.2 + ratio * 8.0 - conservative_penalty - stability_penalty
}
```

Sort by `output_rank_score` for all profiles, with aggressive using slightly higher annualized weight.

- [ ] **Step 3: Change portfolio objective to soft member-count reward**

In `apps/backtest-engine/src/portfolio_search.rs`, locate v2 scoring. Ensure scoring resembles:

```rust
let annualized = portfolio.annualized_return_pct.unwrap_or(portfolio.return_pct);
let drawdown = portfolio.max_drawdown_pct.max(1.0);
let return_dd = annualized / drawdown;
let member_bonus = (portfolio.member_count as f64).ln() * 1.5;
let unique_bonus = (portfolio.unique_symbol_count as f64).ln() * 2.0;
let concentration_penalty = max_single_symbol_weight_pct.max(0.0) * 0.03;
let correlation_penalty = average_pairwise_correlation.max(0.0) * 4.0;
portfolio.score = annualized * 1.0 + return_dd * 12.0 + member_bonus + unique_bonus - concentration_penalty - correlation_penalty;
```

Do not require 10 members. Search member counts from 2 to `min(10, candidate_count)`, but let score decide.

- [ ] **Step 4: Keep high-return + stabilizer combinations**

When selecting portfolio pool, include both:

- Top qualified candidates within drawdown limit.
- Top high-return positive candidates near the limit.
- Low-drawdown stabilizers.

Do not drop candidates solely because they are not in final display Top10 if they are needed for portfolio optimization.

- [ ] **Step 5: Run portfolio tests**

Run:

```bash
cargo test -p backtest-engine portfolio_search -- --nocapture
cargo test -p backtest-engine search_scoring_time_splits -- --nocapture
```

Expected: existing portfolio tests pass; new high-yield test passes.

- [ ] **Step 6: Commit**

```bash
git add apps/backtest-worker/src/main.rs apps/backtest-engine/src/portfolio_search.rs apps/backtest-engine/tests/search_scoring_time_splits.rs
git commit -m "fix: 恢复收益优先的马丁组合搜索" \
  -m "问题描述：当前组合器偏向低收益或固定成员数，历史年化50%+低回撤候选无法进入优先结果。" \
  -m "修复思路：在最大回撤硬约束内提高年化和收益回撤比权重，成员数仅作为软奖励并保留高收益候选池。"
```

---

## Task 4: Portfolio Sandbox Recalculation API

**Files:**
- Modify: `apps/api-server/src/services/backtest_service.rs`
- Modify: `apps/api-server/src/routes/backtest.rs`
- Create: `apps/web/app/api/user/backtest/portfolios/recalculate/route.ts`
- Test: `apps/api-server/tests/martingale_backtest_flow.rs`

- [ ] **Step 1: Define request/response types**

In `apps/api-server/src/services/backtest_service.rs`, add:

```rust
#[derive(Debug, Deserialize)]
pub struct RecalculatePortfolioRequest {
    pub task_id: String,
    pub max_drawdown_pct: Option<f64>,
    pub items: Vec<RecalculatePortfolioItemRequest>,
}

#[derive(Debug, Deserialize)]
pub struct RecalculatePortfolioItemRequest {
    pub candidate_id: String,
    pub symbol: String,
    pub weight_pct: f64,
    pub leverage: f64,
    #[serde(default = "default_true")]
    pub enabled: bool,
}

#[derive(Debug, Serialize)]
pub struct RecalculatePortfolioResponse {
    pub portfolio_id: String,
    pub member_count: usize,
    pub total_return_pct: f64,
    pub annualized_return_pct: Option<f64>,
    pub max_drawdown_pct: f64,
    pub return_drawdown_ratio: Option<f64>,
    pub trade_count: u64,
    pub satisfies_drawdown_limit: bool,
    pub concentration_warnings: Vec<String>,
    pub members: Vec<Value>,
    pub equity_curve: Vec<Value>,
    pub drawdown_curve: Vec<Value>,
    pub trades_preview: Vec<Value>,
}
```

- [ ] **Step 2: Add validation**

Implement `BacktestService::recalculate_portfolio(owner, request)`.

Validation rules:

```rust
if enabled_items.is_empty() { return Err(BacktestError::bad_request("enabled items are required")); }
if (enabled_weight_sum - 100.0).abs() > 0.01 { return Err(BacktestError::bad_request("enabled item weights must sum to 100")); }
if item.leverage <= 0.0 { return Err(BacktestError::bad_request("leverage must be positive")); }
```

For every candidate:

- Load via `repo.get_candidate(candidate_id)`.
- Verify candidate belongs to `task_id`.
- Verify task belongs to `owner`.
- Verify `summary.equity_curve` and `summary.drawdown_curve` exist.
- Reject if missing curves.

- [ ] **Step 3: Implement curve combination**

Use each candidate `summary.equity_curve` normalized by first equity value.

For each sampled timestamp index:

```rust
combined_equity = initial_capital * sum(weight_fraction * member_equity_ratio)
```

Use `initial_capital = 10_000.0` for normalized portfolio display.

Compute drawdown:

```rust
peak = peak.max(combined_equity);
drawdown_pct = if peak > 0.0 { (peak - combined_equity) / peak * 100.0 } else { 0.0 };
```

Return 500 sampled points max.

- [ ] **Step 4: Compute metrics**

Use first/last equity and date range from timestamps:

```rust
total_return_pct = (last_equity / first_equity - 1.0) * 100.0;
annualized_return_pct = if years > 0.0 { Some(((last_equity / first_equity).powf(1.0 / years) - 1.0) * 100.0) } else { None };
max_drawdown_pct = max(drawdown_curve.drawdown_pct);
return_drawdown_ratio = annualized_return_pct.map(|ann| ann / max_drawdown_pct.max(1.0));
```

Combine trade previews by timestamp, include member symbol/candidate_id, sample first 100.

- [ ] **Step 5: Add backend route**

In `apps/api-server/src/routes/backtest.rs`, add route:

```rust
.route("/backtest/portfolios/recalculate", post(recalculate_portfolio))
```

Handler:

```rust
async fn recalculate_portfolio(
    State(auth): State<AuthService>,
    State(service): State<BacktestService>,
    headers: HeaderMap,
    Json(request): Json<RecalculatePortfolioRequest>,
) -> Result<Json<RecalculatePortfolioResponse>, TaskBacktestError> {
    let session = require_user_session(&auth, &headers).map_err(TaskBacktestError::from)?;
    Ok(Json(service.recalculate_portfolio(&session.email, request)?))
}
```

- [ ] **Step 6: Add Next proxy route**

Create `apps/web/app/api/user/backtest/portfolios/recalculate/route.ts`:

```ts
import { proxyBacktestRequest } from "../../proxy";

export async function POST(request: Request) {
  return proxyBacktestRequest(request, {
    backendPath: "/backtest/portfolios/recalculate",
    method: "POST",
  });
}
```

- [ ] **Step 7: Add API test**

In `apps/api-server/tests/martingale_backtest_flow.rs`, add test:

- Create user/session.
- Create succeeded task.
- Save two candidates with summary equity curves:

```json
"equity_curve": [
  {"timestamp_ms": 1672531200000, "equity_quote": 100.0},
  {"timestamp_ms": 1672617600000, "equity_quote": 110.0}
],
"drawdown_curve": [
  {"timestamp_ms": 1672531200000, "drawdown_pct": 0.0},
  {"timestamp_ms": 1672617600000, "drawdown_pct": 2.0}
]
```

- POST `/backtest/portfolios/recalculate` with 50/50 weights.
- Assert response has `equity_curve.len()==2`, `total_return_pct > 0`, `max_drawdown_pct >= 0`.

- [ ] **Step 8: Run tests**

Run:

```bash
cargo test -p api-server --test martingale_backtest_flow recalculate -- --nocapture
cargo check -p api-server
```

Expected: passes.

- [ ] **Step 9: Commit**

```bash
git add apps/api-server/src/services/backtest_service.rs apps/api-server/src/routes/backtest.rs apps/api-server/tests/martingale_backtest_flow.rs apps/web/app/api/user/backtest/portfolios/recalculate/route.ts
git commit -m "feat: 增加人工组合沙盒重算API" \
  -m "问题描述：自动组合无法人工加入已回测策略并重算组合表现，用户无法确认最终组合再发布实盘。" \
  -m "修复思路：新增组合重算接口，基于候选资金曲线按权重合成组合收益、回撤、交易预览和风险提示。"
```

---

## Task 5: Wide Hoverable Charts

**Files:**
- Modify: `apps/web/components/backtest/backtest-charts.tsx`
- Modify: `apps/web/components/backtest/backtest-console.tsx`

- [ ] **Step 1: Replace sparkline with interactive chart component**

In `apps/web/components/backtest/backtest-charts.tsx`, create reusable chart:

```tsx
function InteractiveLineChart({
  title,
  points,
  valueKey,
  valueLabel,
  valueFormatter,
  stroke,
}: {
  title: string;
  points: Array<{ timestamp_ms: number; equity?: number; drawdown?: number }>;
  valueKey: "equity" | "drawdown";
  valueLabel: string;
  valueFormatter: (value: number) => string;
  stroke: string;
}) {
  const [hoverIndex, setHoverIndex] = useState<number | null>(null);
  // Use SVG viewBox 0 0 1000 260.
  // On mouse move, compute nearest index by x ratio.
  // Render vertical guide line, focus dot, and absolute tooltip.
}
```

Tooltip must show:

```tsx
日期: {new Date(point.timestamp_ms).toLocaleDateString()}
{valueLabel}: {valueFormatter(value)}
```

For equity chart also show return from first point:

```tsx
收益: {((point.equity / firstEquity - 1) * 100).toFixed(2)}%
```

For drawdown chart show:

```tsx
回撤: {point.drawdown.toFixed(2)}%
```

- [ ] **Step 2: Make chart layout wide**

Change root chart layout to:

```tsx
<div className="space-y-5 rounded-2xl border border-border bg-card p-4 lg:p-5">
```

Each chart container:

```tsx
<div className="w-full rounded-xl border border-border bg-background p-3">
```

SVG should use `className="h-64 w-full"`.

- [ ] **Step 3: Ensure summary mapping supports portfolio and sandbox**

Keep existing `normalizeEquityCurve` and `normalizeDrawdownCurve`, but ensure they accept:

- `timestamp_ms + equity_quote`
- `timestamp_ms + equity`
- `ts + equity`
- `drawdown_pct`
- `drawdown`

- [ ] **Step 4: Use chart full width in console**

In `apps/web/components/backtest/backtest-console.tsx`, change chart/detail area from narrow column to full row if currently inside cramped grid. Use:

```tsx
<section className="rounded-2xl border border-border bg-card p-4 shadow-sm">
  <h2>...</h2>
  <BacktestCharts summary={selectedSummary} />
</section>
```

Ensure result table and chart can stack vertically on large screens if needed.

- [ ] **Step 5: Type-check**

Run:

```bash
cd apps/web && npx tsc --noEmit --pretty false
```

Expected: pass.

- [ ] **Step 6: Commit**

```bash
git add apps/web/components/backtest/backtest-charts.tsx apps/web/components/backtest/backtest-console.tsx
git commit -m "feat: 增加宽版悬停回测图表" \
  -m "问题描述：资金曲线和回撤曲线挤在小区域且鼠标悬停无法查看日期和数值。" \
  -m "修复思路：改为宽版交互SVG折线图，支持候选、自动组合和沙盒组合的日期/数值tooltip。"
```

---

## Task 6: Frontend Portfolio Sandbox UX

**Files:**
- Modify: `apps/web/lib/api-types.ts`
- Modify: `apps/web/components/backtest/backtest-result-table.tsx`
- Modify: `apps/web/components/backtest/backtest-console.tsx`
- Modify: `apps/web/components/backtest/portfolio-candidate-review.tsx`

- [ ] **Step 1: Add API types**

In `apps/web/lib/api-types.ts`, add:

```ts
export type PortfolioRecalculateResponse = {
  portfolio_id: string;
  member_count: number;
  total_return_pct: number;
  annualized_return_pct?: number | null;
  max_drawdown_pct: number;
  return_drawdown_ratio?: number | null;
  trade_count: number;
  satisfies_drawdown_limit: boolean;
  concentration_warnings: string[];
  members: unknown[];
  equity_curve: unknown[];
  drawdown_curve: unknown[];
  trades_preview: unknown[];
};
```

Also add optional `publishable`, `candidate_warning`, `long_short_legs`, `max_leverage_used`, `market` to `MartingaleBacktestCandidateSummary`.

- [ ] **Step 2: Add edit combination button**

In `apps/web/components/backtest/backtest-result-table.tsx`, add prop:

```ts
onEditPortfolio?: (portfolio: PortfolioTop3Row) => void;
```

For every portfolio card, add button:

```tsx
<button onClick={() => onEditPortfolio?.(entry)} type="button">
  {pickText(lang, "编辑组合", "Edit portfolio")}
</button>
```

- [ ] **Step 3: Add sandbox state in console**

In `apps/web/components/backtest/backtest-console.tsx`, add state:

```ts
const [sandboxItems, setSandboxItems] = useState<PortfolioBasketItem[]>([]);
const [sandboxResult, setSandboxResult] = useState<PortfolioRecalculateResponse | null>(null);
const [sandboxPending, setSandboxPending] = useState(false);
const [sandboxFeedback, setSandboxFeedback] = useState("");
```

When edit portfolio clicked, initialize `sandboxItems` from portfolio members:

```ts
function editPortfolio(portfolio: PortfolioTop3Row) {
  setSandboxItems(portfolio.members.map((member) => ({
    localId: `${member.candidate_id}-${Date.now()}`,
    candidateId: member.candidate_id,
    taskId: selectedTaskId,
    selectedTaskId,
    symbol: member.symbol,
    market: "usd_m_futures",
    direction: member.direction,
    riskProfile: selectedTask?.summary?.risk_profile ?? "balanced",
    parameters: "From auto portfolio",
    recommended_weight_pct: member.allocation_pct,
    recommended_leverage: member.leverage ?? 1,
    weightPct: String(member.allocation_pct),
    leverage: String(member.leverage ?? 1),
    enabled: true,
    parameterSnapshot: {},
    metricsSnapshot: {},
  })));
  setSandboxResult(portfolioSummaryToRecalculateResponse(portfolio));
}
```

- [ ] **Step 4: Allow adding selected candidate to sandbox**

Add button near candidate actions:

```tsx
<button onClick={() => addCandidateToSandbox(selectedCandidate)} type="button">
  {pickText(lang, "加入沙盒组合", "Add to sandbox")}
</button>
```

If candidate `summary.publishable === false`, disable and show warning.

- [ ] **Step 5: Add recalculate action**

Implement:

```ts
async function recalculateSandbox() {
  const enabledItems = sandboxItems.filter((item) => item.enabled);
  const totalWeight = enabledItems.reduce((sum, item) => sum + Number(item.weightPct || 0), 0);
  if (Math.abs(totalWeight - 100) > 0.01) {
    setSandboxFeedback(pickText(lang, "启用项权重合计必须为100%。", "Enabled weights must sum to 100%."));
    return;
  }
  setSandboxPending(true);
  const response = await requestBacktestApi("/api/user/backtest/portfolios/recalculate", {
    method: "POST",
    headers: { "content-type": "application/json" },
    body: JSON.stringify({
      task_id: selectedTaskId,
      max_drawdown_pct: selectedTask?.config?.scoring?.max_drawdown_pct,
      items: enabledItems.map((item) => ({
        candidate_id: item.candidateId,
        symbol: item.symbol,
        weight_pct: Number(item.weightPct),
        leverage: Number(item.leverage),
        enabled: item.enabled,
      })),
    }),
  });
  setSandboxPending(false);
  if (!response.ok) {
    setSandboxFeedback(response.message);
    return;
  }
  setSandboxResult(response.data as PortfolioRecalculateResponse);
}
```

- [ ] **Step 6: Display sandbox result with same chart**

When `sandboxResult` exists, allow selecting it as chart summary:

```ts
const selectedSummary = sandboxResult
  ? portfolioSandboxSummaryForCharts(sandboxResult)
  : selectedPortfolio
    ? portfolioSummaryForCharts(selectedPortfolio)
    : selectedCandidate?.summary ?? {};
```

Show metric cards: annualized, max drawdown, return/dd, satisfies drawdown limit.

- [ ] **Step 7: Publish sandbox final state**

Reuse `PortfolioCandidateReview` basket publish. Add a button:

```tsx
<button onClick={() => setBasketItems(sandboxItems)} type="button">
  {pickText(lang, "使用沙盒组合作为发布篮子", "Use sandbox as publish basket")}
</button>
```

Ensure `parameterSnapshot` is filled from candidate config when adding candidate to sandbox. If edit portfolio member lacks config, resolve candidate from current `candidates` list by `candidate_id`.

- [ ] **Step 8: Type-check**

Run:

```bash
cd apps/web && npx tsc --noEmit --pretty false
```

Expected: pass.

- [ ] **Step 9: Commit**

```bash
git add apps/web/lib/api-types.ts apps/web/components/backtest/backtest-result-table.tsx apps/web/components/backtest/backtest-console.tsx apps/web/components/backtest/portfolio-candidate-review.tsx
git commit -m "feat: 增加人工组合沙盒交互" \
  -m "问题描述：用户无法在自动组合基础上手动加入已回测策略、调整权重杠杆并重算组合表现。" \
  -m "修复思路：前端新增组合沙盒状态、编辑组合、加入候选、重算组合和作为发布篮子的交互。"
```

---

## Task 7: Publish Handoff Verification

**Files:**
- Modify: `apps/api-server/src/services/martingale_publish_service.rs`
- Test: `apps/api-server/src/services/martingale_publish_service.rs`

- [ ] **Step 1: Add test for complete parameter snapshot preservation**

In `martingale_publish_service.rs` tests, create a candidate config with two strategies:

```json
{
  "strategies": [
    {
      "symbol": "BTCUSDT",
      "market": "usd_m_futures",
      "direction": "long",
      "margin_mode": "isolated",
      "leverage": 5,
      "spacing": {"fixed_percent": {"step_bps": 120}},
      "sizing": {"multiplier": {"first_order_quote": "10", "multiplier": "2", "max_legs": 6}},
      "take_profit": {"percent": {"bps": 100}},
      "stop_loss": {"strategy_drawdown_pct": {"pct_bps": 2000}}
    },
    {
      "symbol": "BTCUSDT",
      "market": "usd_m_futures",
      "direction": "short",
      "margin_mode": "isolated",
      "leverage": 5,
      "spacing": {"fixed_percent": {"step_bps": 180}},
      "sizing": {"multiplier": {"first_order_quote": "10", "multiplier": "1.8", "max_legs": 5}},
      "take_profit": {"percent": {"bps": 90}},
      "stop_loss": {"strategy_drawdown_pct": {"pct_bps": 1800}}
    }
  ]
}
```

Publish portfolio with this candidate. Assert created item has:

```rust
assert_eq!(item.parameter_snapshot["strategies"].as_array().unwrap().len(), 2);
assert_eq!(item.parameter_snapshot["strategies"][0]["market"], "usd_m_futures");
assert_eq!(item.parameter_snapshot["strategies"][1]["direction"], "short");
```

- [ ] **Step 2: Add confirm-start preservation test**

After publish, call:

```rust
let running = service.confirm_start_portfolio("user@example.com", &response.portfolio_id).unwrap();
```

Assert:

```rust
assert_eq!(running.status, "running");
assert_eq!(running.items[0].parameter_snapshot["strategies"].as_array().unwrap().len(), 2);
```

- [ ] **Step 3: Ensure request item snapshot is canonical candidate config**

If frontend sends empty `parameter_snapshot`, backend should fallback to candidate.config instead of saving empty object.

In publish map:

```rust
let parameter_snapshot = if item.parameter_snapshot.as_object().map(|m| m.is_empty()).unwrap_or(false) {
    candidate.config.clone()
} else {
    item.parameter_snapshot.clone()
};
```

Save `parameter_snapshot`.

- [ ] **Step 4: Run tests**

Run:

```bash
cargo test -p api-server martingale_publish_service -- --nocapture
```

Expected: pass.

- [ ] **Step 5: Commit**

```bash
git add apps/api-server/src/services/martingale_publish_service.rs
git commit -m "test: 验证马丁组合发布参数传递" \
  -m "问题描述：自定义组合发布到实盘后，需要确认完整long/short参数、杠杆、市场和风控快照不会丢失。" \
  -m "修复思路：增加发布和confirm-start参数快照回归测试，并为空快照提供候选config兜底。"
```

---

## Task 8: End-to-End Verification and Deployment

**Files:**
- No source file changes unless previous tasks reveal failures.

- [ ] **Step 1: Run full local verification**

Run:

```bash
cargo fmt --check
cargo test -p backtest-engine recommended_liquid_symbols -- --nocapture
cargo test -p backtest-engine portfolio_search -- --nocapture
cargo test -p api-server --test martingale_backtest_flow recalculate -- --nocapture
cargo test -p api-server martingale_publish_service -- --nocapture
cargo check -p backtest-worker
cargo check -p api-server
cd apps/web && npx tsc --noEmit --pretty false
```

Expected: all pass. Existing unrelated warnings are acceptable only if tests/checks exit 0.

- [ ] **Step 2: Build and deploy services**

Because Worker changes are required, deploy all affected services:

```bash
docker compose --env-file /home/bumblebee/Project/grid_binance/.worktrees/full-v1/.env \
  -f deploy/docker/docker-compose.yml build api-server web backtest-worker

docker compose --env-file /home/bumblebee/Project/grid_binance/.worktrees/full-v1/.env \
  -f deploy/docker/docker-compose.yml up -d --no-deps --force-recreate api-server web backtest-worker
```

- [ ] **Step 3: Health check**

Run:

```bash
curl -sS http://127.0.0.1:8080/nginx-health

docker compose --env-file /home/bumblebee/Project/grid_binance/.worktrees/full-v1/.env \
  -f deploy/docker/docker-compose.yml ps api-server web backtest-worker
```

Expected: nginx returns `ok`; services are up/healthy where healthcheck exists.

- [ ] **Step 4: Create validation backtest task**

Create one new flyingkid-owned validation task using the API or direct DB insert matching normal task schema:

- owner: `flyingkid2022@outlook.com`
- symbols: BTCUSDT, ETHUSDT, BNBUSDT, SOLUSDT, DOGEUSDT, XRPUSDT, ADAUSDT
- market: `usd_m_futures`
- direction_mode: `long_and_short`
- risk_profile: `aggressive` for high-yield regression, and max drawdown hard limit 20 if available.
- search_mode: `profit_optimized_v2`
- per_symbol_top_n: 10
- portfolio_top_n: 10

Track status until completion:

```sql
SELECT task_id,status,summary->>'stage_label',summary->>'progress_pct',error_message
FROM backtest_tasks
WHERE task_id='<new_task_id>';
```

- [ ] **Step 5: Validate candidate distribution**

After completion run:

```sql
SELECT summary->>'symbol' AS symbol,
       count(*) AS candidates,
       max((summary->>'annualized_return_pct')::numeric) AS max_ann,
       min((summary->>'max_drawdown_pct')::numeric) AS min_dd,
       bool_or(config::text LIKE '%"spot"%') AS has_spot
FROM backtest_candidate_summaries
WHERE task_id='<new_task_id>'
GROUP BY summary->>'symbol'
ORDER BY symbol;
```

Expected:

- More than one symbol appears.
- No `has_spot=true`.
- Leverage fields exist.
- Any missing symbols are listed in task summary diagnostics.

- [ ] **Step 6: Validate portfolio curves**

Run:

```sql
WITH p AS (
  SELECT jsonb_array_elements(summary->'portfolio_top10') AS item
  FROM backtest_tasks
  WHERE task_id='<new_task_id>'
)
SELECT item->>'portfolio_rank' AS rank,
       item->>'member_count' AS members,
       item->>'annualized_return_pct' AS annualized,
       item->>'max_drawdown_pct' AS dd,
       jsonb_array_length(item->'equity_curve') AS equity_points,
       jsonb_array_length(item->'drawdown_curve') AS dd_points
FROM p
ORDER BY (item->>'portfolio_rank')::int;
```

Expected: Top portfolios have equity/drawdown curve points and member counts decided by score, not forced to 10.

- [ ] **Step 7: Validate sandbox recalculation**

Use API or DB-backed HTTP session if available to POST `/api/user/backtest/portfolios/recalculate` with two candidate IDs from the completed task and weights 50/50.

Expected response:

- `total_return_pct` finite.
- `max_drawdown_pct` finite.
- `equity_curve.length > 0`.
- `drawdown_curve.length > 0`.

- [ ] **Step 8: Validate publish handoff**

Publish a small sandbox/basket portfolio with enabled weights summing to 100.

Run DB checks:

```sql
SELECT portfolio_id, owner, status, market, direction, total_weight_pct
FROM martingale_portfolios
WHERE owner='flyingkid2022@outlook.com'
ORDER BY created_at DESC
LIMIT 1;

SELECT symbol, weight_pct, leverage,
       jsonb_array_length(parameter_snapshot->'strategies') AS strategy_count,
       parameter_snapshot->'strategies'->0->>'market' AS market0
FROM martingale_portfolio_items
WHERE portfolio_id='<portfolio_id>';
```

Expected: status `pending_confirmation`, strategy_count >= 1, market0 `usd_m_futures`.

- [ ] **Step 9: Final commit if verification generated docs/scripts**

If only runtime DB tasks were created, do not commit runtime data. If source fixes were needed, commit them with a message containing problem and fix.

- [ ] **Step 10: Push**

```bash
git push origin main
```

Expected: remote updated.

---

## Self-Review Checklist

- Spec coverage: Top50, candidate retention, futures-only, high-yield regression, soft-count portfolio objective, sandbox recalculation, hover charts, publish handoff are covered by Tasks 1-8.
- Placeholder scan: No `TBD` or unspecified implementation remains; every task has file paths, code-level instructions, and commands.
- Type consistency: Backend route `/backtest/portfolios/recalculate` maps to frontend proxy `/api/user/backtest/portfolios/recalculate`; response maps to `PortfolioRecalculateResponse`; publish uses existing `PublishPortfolioRequest` with fallback parameter snapshot.
