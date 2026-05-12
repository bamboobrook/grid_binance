/** Shared API response types for the backtest / martingale domain. */

export interface MartingaleEquityPoint {
  ts: number;
  equity: number;
  drawdown?: number;
}

export interface MartingaleBacktestCandidateSummary {
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
  artifact_path?: string;
  portfolio_group_key?: string;
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
