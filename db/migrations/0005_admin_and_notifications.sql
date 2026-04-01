CREATE TABLE IF NOT EXISTS strategy_templates (
    id TEXT PRIMARY KEY,
    sequence_id BIGINT NOT NULL UNIQUE,
    name TEXT NOT NULL,
    symbol TEXT NOT NULL,
    budget TEXT NOT NULL,
    grid_spacing_bps INTEGER NOT NULL,
    membership_ready BOOLEAN NOT NULL DEFAULT FALSE,
    exchange_ready BOOLEAN NOT NULL DEFAULT FALSE,
    symbol_ready BOOLEAN NOT NULL DEFAULT FALSE,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE TABLE IF NOT EXISTS system_configs (
    config_key TEXT PRIMARY KEY,
    config_value JSONB NOT NULL DEFAULT '{}'::jsonb,
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE TABLE IF NOT EXISTS audit_logs (
    audit_id BIGSERIAL PRIMARY KEY,
    actor_email TEXT NOT NULL,
    action TEXT NOT NULL,
    target_type TEXT NOT NULL,
    target_id TEXT NOT NULL,
    payload JSONB NOT NULL DEFAULT '{}'::jsonb,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE TABLE IF NOT EXISTS notification_logs (
    notification_id BIGSERIAL PRIMARY KEY,
    user_email TEXT NOT NULL,
    channel TEXT NOT NULL,
    template_key TEXT,
    title TEXT NOT NULL,
    body TEXT NOT NULL,
    status TEXT NOT NULL,
    payload JSONB NOT NULL DEFAULT '{}'::jsonb,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    delivered_at TIMESTAMPTZ
);

CREATE INDEX IF NOT EXISTS idx_notification_logs_user_email
    ON notification_logs ((lower(user_email)), created_at DESC);
