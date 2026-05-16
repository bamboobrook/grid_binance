CREATE TABLE IF NOT EXISTS backtest_quota_policies (
    owner TEXT PRIMARY KEY,
    policy JSONB NOT NULL DEFAULT '{}'::jsonb,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE TABLE IF NOT EXISTS backtest_tasks (
    task_id TEXT PRIMARY KEY,
    owner TEXT NOT NULL,
    status TEXT NOT NULL,
    strategy_type TEXT NOT NULL,
    config JSONB NOT NULL DEFAULT '{}'::jsonb,
    summary JSONB NOT NULL DEFAULT '{}'::jsonb,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    started_at TIMESTAMPTZ,
    completed_at TIMESTAMPTZ,
    error_message TEXT
);

CREATE TABLE IF NOT EXISTS backtest_task_events (
    event_id BIGSERIAL PRIMARY KEY,
    task_id TEXT NOT NULL REFERENCES backtest_tasks(task_id) ON DELETE CASCADE,
    event_type TEXT NOT NULL,
    payload JSONB NOT NULL DEFAULT '{}'::jsonb,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE TABLE IF NOT EXISTS backtest_candidate_summaries (
    candidate_id TEXT PRIMARY KEY,
    task_id TEXT NOT NULL REFERENCES backtest_tasks(task_id) ON DELETE CASCADE,
    status TEXT NOT NULL,
    rank INTEGER NOT NULL DEFAULT 0,
    config JSONB NOT NULL DEFAULT '{}'::jsonb,
    summary JSONB NOT NULL DEFAULT '{}'::jsonb,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE TABLE IF NOT EXISTS backtest_artifacts (
    artifact_id TEXT PRIMARY KEY,
    candidate_id TEXT NOT NULL REFERENCES backtest_candidate_summaries(candidate_id) ON DELETE CASCADE,
    artifact_type TEXT NOT NULL,
    uri TEXT NOT NULL,
    metadata JSONB NOT NULL DEFAULT '{}'::jsonb,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE TABLE IF NOT EXISTS martingale_portfolio_candidates (
    portfolio_candidate_id TEXT PRIMARY KEY,
    candidate_id TEXT NOT NULL REFERENCES backtest_candidate_summaries(candidate_id) ON DELETE CASCADE,
    owner TEXT NOT NULL,
    status TEXT NOT NULL,
    config JSONB NOT NULL DEFAULT '{}'::jsonb,
    summary JSONB NOT NULL DEFAULT '{}'::jsonb,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE TABLE IF NOT EXISTS martingale_portfolio_publish_records (
    publish_id TEXT PRIMARY KEY,
    candidate_id TEXT NOT NULL REFERENCES backtest_candidate_summaries(candidate_id) ON DELETE CASCADE,
    portfolio_id TEXT,
    owner TEXT NOT NULL,
    status TEXT NOT NULL,
    request JSONB NOT NULL DEFAULT '{}'::jsonb,
    result JSONB NOT NULL DEFAULT '{}'::jsonb,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE TABLE IF NOT EXISTS martingale_live_portfolios (
    portfolio_id TEXT PRIMARY KEY,
    owner TEXT NOT NULL,
    status TEXT NOT NULL,
    config JSONB NOT NULL DEFAULT '{}'::jsonb,
    summary JSONB NOT NULL DEFAULT '{}'::jsonb,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    started_at TIMESTAMPTZ,
    stopped_at TIMESTAMPTZ
);

CREATE TABLE IF NOT EXISTS martingale_live_strategy_instances (
    instance_id TEXT PRIMARY KEY,
    portfolio_id TEXT NOT NULL REFERENCES martingale_live_portfolios(portfolio_id) ON DELETE CASCADE,
    owner TEXT NOT NULL,
    status TEXT NOT NULL,
    strategy_id TEXT NOT NULL,
    config JSONB NOT NULL DEFAULT '{}'::jsonb,
    summary JSONB NOT NULL DEFAULT '{}'::jsonb,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE TABLE IF NOT EXISTS martingale_orphan_orders (
    orphan_order_id TEXT PRIMARY KEY,
    portfolio_id TEXT REFERENCES martingale_live_portfolios(portfolio_id) ON DELETE SET NULL,
    owner TEXT NOT NULL,
    status TEXT NOT NULL,
    exchange TEXT NOT NULL,
    symbol TEXT NOT NULL,
    order_ref TEXT NOT NULL,
    summary JSONB NOT NULL DEFAULT '{}'::jsonb,
    detected_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    resolved_at TIMESTAMPTZ
);

CREATE INDEX IF NOT EXISTS idx_backtest_quota_policies_owner ON backtest_quota_policies(owner);
CREATE INDEX IF NOT EXISTS idx_backtest_tasks_owner ON backtest_tasks(owner);
CREATE INDEX IF NOT EXISTS idx_backtest_tasks_status ON backtest_tasks(status);
CREATE INDEX IF NOT EXISTS idx_backtest_task_events_task_id ON backtest_task_events(task_id);
CREATE INDEX IF NOT EXISTS idx_backtest_candidate_summaries_task_id ON backtest_candidate_summaries(task_id);
CREATE INDEX IF NOT EXISTS idx_backtest_candidate_summaries_status ON backtest_candidate_summaries(status);
CREATE INDEX IF NOT EXISTS idx_backtest_artifacts_candidate_id ON backtest_artifacts(candidate_id);
CREATE INDEX IF NOT EXISTS idx_martingale_portfolio_candidates_owner ON martingale_portfolio_candidates(owner);
CREATE INDEX IF NOT EXISTS idx_martingale_portfolio_candidates_status ON martingale_portfolio_candidates(status);
CREATE INDEX IF NOT EXISTS idx_martingale_portfolio_candidates_candidate_id ON martingale_portfolio_candidates(candidate_id);
CREATE INDEX IF NOT EXISTS idx_martingale_portfolio_publish_records_owner ON martingale_portfolio_publish_records(owner);
CREATE INDEX IF NOT EXISTS idx_martingale_portfolio_publish_records_status ON martingale_portfolio_publish_records(status);
CREATE INDEX IF NOT EXISTS idx_martingale_portfolio_publish_records_candidate_id ON martingale_portfolio_publish_records(candidate_id);
CREATE INDEX IF NOT EXISTS idx_martingale_portfolio_publish_records_portfolio_id ON martingale_portfolio_publish_records(portfolio_id);
CREATE INDEX IF NOT EXISTS idx_martingale_live_portfolios_owner ON martingale_live_portfolios(owner);
CREATE INDEX IF NOT EXISTS idx_martingale_live_portfolios_status ON martingale_live_portfolios(status);
CREATE INDEX IF NOT EXISTS idx_martingale_live_strategy_instances_owner ON martingale_live_strategy_instances(owner);
CREATE INDEX IF NOT EXISTS idx_martingale_live_strategy_instances_status ON martingale_live_strategy_instances(status);
CREATE INDEX IF NOT EXISTS idx_martingale_live_strategy_instances_portfolio_id ON martingale_live_strategy_instances(portfolio_id);
CREATE INDEX IF NOT EXISTS idx_martingale_orphan_orders_owner ON martingale_orphan_orders(owner);
CREATE INDEX IF NOT EXISTS idx_martingale_orphan_orders_status ON martingale_orphan_orders(status);
CREATE INDEX IF NOT EXISTS idx_martingale_orphan_orders_portfolio_id ON martingale_orphan_orders(portfolio_id);
