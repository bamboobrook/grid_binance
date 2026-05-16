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

CREATE INDEX IF NOT EXISTS idx_martingale_portfolios_owner_created ON martingale_portfolios(owner, created_at DESC);
CREATE INDEX IF NOT EXISTS idx_martingale_portfolio_items_portfolio ON martingale_portfolio_items(portfolio_id);
CREATE INDEX IF NOT EXISTS idx_martingale_portfolio_items_candidate ON martingale_portfolio_items(candidate_id);
