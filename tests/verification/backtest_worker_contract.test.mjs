import test from "node:test";
import assert from "node:assert/strict";
import { readFileSync } from "node:fs";

test("backtest worker uses read-only market data instead of synthetic candidates", () => {
  const worker = readFileSync("apps/backtest-worker/src/main.rs", "utf8");
  assert.match(worker, /BACKTEST_MARKET_DATA_DB_PATH is required/);
  assert.match(worker, /SqliteMarketDataSource::open_readonly/);
  assert.match(worker, /MarketDataContext::load/);
  assert.match(worker, /run_kline_screening\(candidate\.config\.clone\(\), bars\.as_ref\(\)\)/);
  assert.match(worker, /run_trade_refinement\(candidate\.config\.clone\(\), trades\.as_ref\(\)\)/);
  assert.match(worker, /MAX_TRADE_REFINEMENT_ROWS/);
  assert.match(worker, /aggregate_bars\(bars, 15 \* 60 \* 1_000\)/);
  assert.match(worker, /allocation_curve/);
  assert.match(worker, /regime_timeline/);
  assert.match(worker, /portfolio_top_n/);
  assert.match(worker, /dynamic_allocation_rules/);
  assert.match(worker, /max_drawdown_limit_pct/);
  assert.doesNotMatch(worker, /deterministic_result/);
});

test("backtest worker persists candidate artifact and task portfolio summary contracts", () => {
  const worker = readFileSync("apps/backtest-worker/src/main.rs", "utf8");
  const requiredCandidateArtifactFields = [
    "portfolio_top_n",
    "allocation_curve",
    "regime_timeline",
    "cost_summary",
    "rebalance_count",
    "forced_exit_count",
    "average_allocation_hold_hours",
    "dynamic_allocation_rules",
    "risk_summary_human",
    "per_symbol_rank",
    "equity_curve",
  ];

  assert.match(worker, /fn candidate_summary_artifact_row/);
  for (const field of requiredCandidateArtifactFields) {
    assert.match(worker, new RegExp(`"${field}"`));
  }
  assert.match(worker, /write_candidate_summary_artifact\(artifact_root, task_id, output\)/);
  assert.match(worker, /save_candidate_with_artifact/);
  assert.match(worker, /fn task_portfolio_summary/);
  assert.match(worker, /"portfolio_top_n": PORTFOLIO_TOP_N/);
  assert.match(worker, /"portfolio_candidates": portfolio_optimization\.candidates/);
});

test("backtest worker keeps portfolio optimization on real equity curves with downgrade warnings", () => {
  const worker = readFileSync("apps/backtest-worker/src/main.rs", "utf8");
  assert.match(worker, /fn output_equity_curve/);
  assert.match(worker, /\["equity_quote", "equity", "capital", "value"\]/);
  assert.doesNotMatch(worker, /vec!\[0\.0, output\.total_return_pct\]/);
  assert.match(worker, /invalid max_drawdown_pct/);
  assert.match(worker, /PortfolioOptimizationOutput \{\s*candidates: Vec::new\(\),\s*warning: Some/s);
  assert.match(worker, /portfolio_optimizer_warning/);
});

test("api task creation does not create placeholder publish candidates", () => {
  const service = readFileSync("apps/api-server/src/services/backtest_service.rs", "utf8");
  const flow = readFileSync("apps/api-server/tests/martingale_backtest_flow.rs", "utf8");
  assert.doesNotMatch(service, /api_placeholder|default_candidate_config/);
  assert.match(flow, /task_creation_does_not_publish_unverified_placeholder_candidates/);
  assert.match(flow, /save_ready_candidate/);
  assert.match(flow, /worker_verified/);
});
