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
  assert.match(worker, /drawdown_limits_for_direction_mode\(/);
  assert.match(worker, /score\.survival_valid/);
  assert.match(worker, /total_return_pct\s*<=\s*0\.0/);
  assert.match(worker, /build_portfolio_top3/);
  assert.match(worker, /interval.*1m|"1m"/);
  assert.match(worker, /usd_m_futures|futures/);
});

test("backtest worker persists real portfolio Top 3 from outputs", () => {
  const worker = readFileSync("apps/backtest-worker/src/main.rs", "utf8");
  assert.match(worker, /portfolio_candidates_from_outputs/);
  assert.match(worker, /update_task_summary\(/);
  assert.match(worker, /portfolio_top3_artifact_path/);
  assert.match(worker, /source_candidate_id/);
  assert.match(worker, /"symbol"/);
  assert.match(worker, /"trade_count"/);
  assert.match(worker, /persisted_candidates/);
  assert.doesNotMatch(worker, /max_drawdown_pct:\s*0\.0/);
});

test("backtest worker uses directions_from_mode for multi-direction search", () => {
  const worker = readFileSync("apps/backtest-worker/src/main.rs", "utf8");
  assert.match(worker, /directions_from_mode/);
  assert.match(worker, /Long, Short/);
  assert.doesNotMatch(worker, /fn build_long_short_config/);
  assert.doesNotMatch(worker, /fn combine_leg_results/);
  assert.doesNotMatch(worker, /fn merge_equity_curves_by_timestamp/);
  assert.doesNotMatch(worker, /fn max_drawdown_from_curve/);
});

test("backtest worker supports mixed_best mode across long short and long_short", () => {
  const worker = readFileSync("apps/backtest-worker/src/main.rs", "utf8");
  assert.match(worker, /is_mixed_best_mode/);
  assert.match(worker, /search_direction_modes_for_task/);
  assert.match(worker, /"long_only"/);
  assert.match(worker, /"short_only"/);
  assert.match(worker, /"long_short"/);
  assert.match(worker, /task_config_for_direction_mode/);
  assert.match(worker, /mixed_best_direction_modes/);
});

test("backtest worker applies task overrides before screening and refinement", () => {
  const worker = readFileSync("apps/backtest-worker/src/main.rs", "utf8");
  assert.match(worker, /apply_task_overrides_to_candidate\((candidate\.clone\(\)|candidate), task\)/);
  assert.match(worker, /run_candidate_kline_screening\(&overridden_candidate, market_context\)/);
  assert.match(worker, /run_candidate_trade_refinement\(&overridden_candidate, market_context\)/);
});

test("worker emits complete martingale artifacts and true portfolio combinations", () => {
  const worker = readFileSync("apps/backtest-worker/src/main.rs", "utf8");
  const portfolio = readFileSync("apps/backtest-engine/src/portfolio_search.rs", "utf8");
  const metrics = readFileSync("apps/backtest-engine/src/martingale/metrics.rs", "utf8");

  assert.match(worker, /annualized_return_pct/);
  assert.match(worker, /drawdown_curve/);
  assert.match(worker, /trades_preview|trade_details|trades/);
  assert.match(worker, /eligible_candidates|eligible_candidate_count/);
  assert.match(worker, /long_short|long_and_short|LongAndShort|MartingaleDirectionMode::LongAndShort/);
  assert.match(worker, /planned_margin_quote/);
  assert.match(worker, /max_leverage_used|leverage/);

  assert.match(portfolio, /PortfolioMember/);
  assert.match(portfolio, /allocation_pct/);
  assert.match(portfolio, /combine_equity_curves|weighted_portfolio_equity/);
  assert.match(portfolio, /member_count/);
  assert.doesNotMatch(portfolio, /ranked\.iter\(\)\.take\(3\).*cloned/s);

  assert.match(metrics, /calculate_annualized_return_pct/);
  assert.match(metrics, /planned_margin_quote/);
  assert.match(metrics, /notional_quote/);

  // P1: portfolio preserves full candidate pool before display truncation
  assert.match(worker, /portfolio_candidates_from_outputs|eligible_pool|eligible_candidates/);

  // P1: each portfolio has a unique, non-template ID (not just portfolio-{member_count})
  assert.doesNotMatch(worker, /"portfolio_id":\s*format!\("portfolio-\{\}"\s*,\s*portfolio\.member_count\)/);

  // P1: long_short candidates carry per-leg weight
  assert.match(worker, /long_weight_pct|short_weight_pct/);

  // P1: WeightedPortfolio carries trades_preview
  assert.match(worker, /trades_preview/);
});

test("worker cannot complete martingale tasks with zero selected candidates silently", () => {
  const worker = readFileSync("apps/backtest-worker/src/main.rs", "utf8");
  assert.match(worker, /ensure_non_empty_selection_for_task/);
  assert.match(worker, /no martingale candidates selected/);
  assert.match(worker, /screened_count/);
  assert.match(worker, /selected_count=0/);
  assert.match(worker, /select_candidates_or_best_fallback_for_task/);
});

test("worker records rejection diagnostics when martingale selection is empty", () => {
  const worker = readFileSync("apps/backtest-worker/src/main.rs", "utf8");
  assert.match(worker, /CandidateRejectionDiagnostics/);
  assert.match(worker, /rejection_diagnostics/);
  assert.match(worker, /negative_return_count/);
  assert.match(worker, /drawdown_rejected_count/);
  assert.match(worker, /best_by_return/);
  assert.match(worker, /lowest_drawdown/);
});

test("long_short worker path does not substitute single-direction candidates", () => {
  const worker = readFileSync("apps/backtest-worker/src/main.rs", "utf8");
  const fnMatch = worker.match(/fn run_long_short_staged_search[\s\S]*?\n}\n\nfn /);
  assert.ok(fnMatch, "run_long_short_staged_search should exist");
  const body = fnMatch[0];
  assert.doesNotMatch(body, /generate_staged_candidates_for_symbol\([^\)]*"long"/);
  assert.doesNotMatch(body, /generate_staged_candidates_for_symbol\([^\)]*"short"/);
  assert.doesNotMatch(body, /\blet long_candidates\b/);
  assert.doesNotMatch(body, /\blet short_candidates\b/);
});

test("long_short worker rejects negative-only smoke instead of reporting success", () => {
  const worker = readFileSync("apps/backtest-worker/src/main.rs", "utf8");
  assert.match(worker, /no martingale candidates selected/);
  assert.match(worker, /negative_return/);
  assert.doesNotMatch(worker, /single_direction_candidates|LongOnly.*fallback|ShortOnly.*fallback/);
});

test("worker summary exposes eligible symbols and portfolio unique symbol count", () => {
  const worker = readFileSync("apps/backtest-worker/src/main.rs", "utf8");
  assert.match(worker, /eligible_symbols/);
  assert.match(worker, /unique_eligible_symbol_count/);
  assert.match(worker, /portfolio_unique_symbol_count/);
  assert.match(worker, /portfolio_symbols/);
});

test("worker persists every candidate referenced by auto portfolios before publishing", () => {
  const worker = readFileSync("apps/backtest-worker/src/main.rs", "utf8");

  assert.match(worker, /persistable_outputs/);
  assert.match(worker, /merge_candidate_outputs_for_persistence/);
  assert.match(worker, /save_candidates_and_artifacts\(&task\.task_id, evaluated_count, &persistable_outputs\)/);
  assert.doesNotMatch(
    worker,
    /save_candidates_and_artifacts\(&task\.task_id, evaluated_count, &display_outputs\)/,
    "saving only display_outputs leaves portfolio pool candidate_ids unpublished",
  );
});
