/** Shared API response types for the backtest / martingale domain. */

export interface MartingaleEquityPoint {
  ts: number;
  equity: number;
  drawdown?: number;
}

export interface MartingaleBacktestCandidateSummary {
  score?: number;
  total_return_pct?: number;
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
  overfitting_risk?: boolean;
  data_quality_score?: number;
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