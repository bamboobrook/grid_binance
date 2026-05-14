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
  assert.match(worker, /allocation_curve/);
  assert.match(worker, /regime_timeline/);
  assert.match(worker, /portfolio_top_n/);
  assert.match(worker, /dynamic_allocation_rules/);
  assert.match(worker, /max_drawdown_limit_pct/);
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
