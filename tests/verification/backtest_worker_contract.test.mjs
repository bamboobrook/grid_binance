import test from "node:test";
import assert from "node:assert/strict";
import { readFileSync } from "node:fs";

test("backtest worker uses read-only market data instead of synthetic candidates", () => {
  const worker = readFileSync("apps/backtest-worker/src/main.rs", "utf8");
  assert.match(worker, /BACKTEST_MARKET_DATA_DB_PATH is required/);
  assert.match(worker, /SqliteMarketDataSource::open_readonly/);
  assert.match(worker, /MarketDataContext::load/);
  assert.match(worker, /run_kline_screening\(candidate\.config\.clone\(\), &bars\)/);
  assert.match(worker, /run_trade_refinement\(candidate\.config\.clone\(\), &trades\)/);
  assert.doesNotMatch(worker, /deterministic_result/);
});

test("api task creation does not create placeholder publish candidates", () => {
  const service = readFileSync("apps/api-server/src/services/backtest_service.rs", "utf8");
  const flow = readFileSync("apps/api-server/tests/martingale_backtest_flow.rs", "utf8");
  assert.doesNotMatch(service, /api_placeholder|default_candidate_config/);
  assert.match(flow, /task_creation_does_not_publish_unverified_placeholder_candidates/);
  assert.match(flow, /save_ready_candidate/);
  assert.match(flow, /worker_verified/);
});

test("backtest worker contains profit-first staged auto-search flow contract", () => {
  const worker = readFileSync("apps/backtest-worker/src/main.rs", "utf8");
  assert.match(worker, /per_symbol_top_n/);
  assert.match(worker, /portfolio_top_n/);
  assert.match(worker, /run_profit_first_staged_search\(\s*&market_context,/);
  assert.doesNotMatch(worker, /let random_candidates = apply_task_overrides\(\s*random_search\(/);
  assert.match(worker, /relax_drawdown_limit/);
  assert.match(worker, /reject_negative_return|positive_return/);
  assert.match(worker, /build_portfolio_top3/);
  assert.match(worker, /interval.*1m|"1m"/);
  assert.match(worker, /usd_m_futures|futures/);
});

test("backtest worker persists real portfolio Top 3 from outputs", () => {
  const worker = readFileSync("apps/backtest-worker/src/main.rs", "utf8");
  assert.match(worker, /portfolio_candidates_from_outputs/);
  assert.match(worker, /update_task_summary\(/);
  assert.match(worker, /portfolio_top3_artifact_path/);
  assert.doesNotMatch(worker, /max_drawdown_pct:\s*0\.0/);
});
