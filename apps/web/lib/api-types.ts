/** Shared API response types for the backtest / martingale domain. */

export interface MartingaleEquityPoint {
  ts: number;
  equity: number;
  drawdown?: number;
}

export interface MartingaleAllocationPoint {
  timestamp_ms: number;
  symbol?: string;
  long_weight_pct: ApiDecimal;
  short_weight_pct: ApiDecimal;
  action?: string;
  reason?: string;
  in_cooldown?: boolean;
}

export interface MartingaleRegimePoint {
  timestamp_ms?: number;
  btc_regime?: string;
  symbol_regime?: string;
  symbol?: string;
}

export interface MartingaleCostSummary {
  fee_quote?: ApiDecimal;
  slippage_quote?: ApiDecimal;
  stop_loss_quote?: ApiDecimal;
  forced_exit_quote?: ApiDecimal;
}

export interface MartingaleBacktestCandidateSummary {
  symbol?: string;
  direction?: string;
  strategy_legs?: Array<{ direction?: string; spacing_bps?: number; take_profit_bps?: number }>;
  spacing_bps?: number;
  first_order_quote?: number;
  order_multiplier?: number;
  max_legs?: number;
  total_margin_budget_quote?: number;
  take_profit_bps?: number;
  trailing_take_profit_bps?: number;
  recommended_weight_pct?: number;
  recommended_leverage?: number;
  parameter_rank_for_symbol?: number;
  risk_profile?: string;
  total_return_pct?: number;
  annualized_return_pct?: number;
  backtest_years?: number;
  max_drawdown_pct?: number;
  score?: number;
  max_drawdown?: number;
  trade_count?: number;
  stop_count?: number;
  max_capital_used_quote?: number;
  survival_passed?: boolean;
  rejection_reasons?: string[];
  stress_window_scores?: Record<string, number>;
  equity_curve?: MartingaleEquityPoint[];
  stop_loss_events?: { ts: number; symbol: string; reason: string; loss_pct: number }[];
  train_return_pct?: number;
  validate_return_pct?: number;
  test_return_pct?: number;
  stress_return_pct?: number;
  overfit_flag?: boolean;
  overfitting_risk?: boolean;
  data_quality_score?: number;
  risk_summary_human?: string;
  drawdown_curve?: MartingaleEquityPoint[];
  trade_events?: MartingaleTradeEvent[];
  sampled_trade_events?: MartingaleTradeEvent[];
  data_coverage?: MartingaleDataCoverage;
  artifact_path?: string;
  artifact?: { allocation_curve?: MartingaleAllocationPoint[] };
  candidate_artifact?: { allocation_curve?: MartingaleAllocationPoint[] };
  portfolio_group_key?: string;
  allocation_curve?: MartingaleAllocationPoint[];
  regime_timeline?: MartingaleRegimePoint[];
  cost_summary?: MartingaleCostSummary;
  return_drawdown_ratio?: number;
  profit_drawdown_ratio?: number;
  rebalance_count?: number;
  forced_exit_count?: number;
  average_allocation_hold_hours?: number;
  live_recommended?: boolean;
  can_recommend_live?: boolean;
  max_drawdown_limit_passed?: boolean;
  discarded_symbols_from_portfolio_top10?: string[];
  portfolio_top10_discarded_symbols?: string[];
  portfolio_candidate_id?: string;
  items?: Array<{ candidate_id?: string; symbol?: string; weight_pct?: ApiDecimal; recommended_leverage?: number; return_contribution_pct?: ApiDecimal; drawdown_contribution_pct?: ApiDecimal }>;
  symbols?: string[];
  symbol_weights?: Record<string, ApiDecimal>;
  package_contributions?: Array<{ candidate_id?: string; symbol?: string; weight_pct?: ApiDecimal; return_contribution_pct?: ApiDecimal; drawdown_contribution_pct?: ApiDecimal }>;
  cost_burden_quote?: ApiDecimal;
  average_correlation?: ApiDecimal | null;
}

export interface MartingaleTradeEvent {
  ts: number;
  type: string;
  symbol: string;
  strategy_instance_id?: string;
  cycle_id?: string | null;
  detail?: string;
}

export interface MartingaleDataCoverage {
  interval?: string;
  requested_start_ms?: number;
  requested_end_ms?: number;
  first_bar_ms?: number;
  last_bar_ms?: number;
  bar_count?: number;
  agg_trade_count?: number;
  used_full_minute_coverage?: boolean;
}

export interface MartingaleRiskSummary {
  strategy_count?: number;
  symbols?: string[];
  max_leverage?: number;
  requires_futures?: boolean;
  max_drawdown?: number;
  liquidation_distance_pct?: number;
  funding_fee_estimate?: string;
  total_budget_quote?: number;
  max_single_strategy_budget?: number;
  warnings?: string[];
}

export type ApiDecimal = number | string;

export interface PublishPortfolioItemRequest {
  candidate_id: string;
  symbol: string;
  weight_pct: ApiDecimal;
  leverage: number;
  enabled?: boolean;
  parameter_snapshot: unknown;
}

export interface PublishPortfolioRequest {
  name: string;
  task_id: string;
  market: string;
  direction: string;
  risk_profile: string;
  total_weight_pct: ApiDecimal;
  items: PublishPortfolioItemRequest[];
}

export interface PublishedStrategyInstance {
  strategy_instance_id: string;
  candidate_id: string;
  symbol: string;
  weight_pct: ApiDecimal;
  leverage: number;
  status: string;
}

export interface PublishPortfolioResponse {
  portfolio_id: string;
  status: string;
  source_task_id: string;
  items: PublishedStrategyInstance[];
  risk_summary: unknown;
}

export interface MartingalePortfolioItem {
  strategy_instance_id: string;
  portfolio_id: string;
  candidate_id: string;
  symbol: string;
  weight_pct: ApiDecimal;
  leverage: number;
  enabled: boolean;
  status: string;
  parameter_snapshot: unknown;
  metrics_snapshot: unknown;
  created_at: string;
  updated_at: string;
}

export interface MartingalePortfolioDetail {
  portfolio_id: string;
  owner: string;
  name: string;
  status: string;
  source_task_id: string;
  market: string;
  direction: string;
  risk_profile: string;
  total_weight_pct: ApiDecimal;
  config: unknown;
  risk_summary: unknown;
  created_at: string;
  updated_at: string;
  items: MartingalePortfolioItem[];
}

export type MartingalePortfolioList = MartingalePortfolioDetail[];
