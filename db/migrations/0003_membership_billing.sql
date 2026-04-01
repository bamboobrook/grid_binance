CREATE TABLE IF NOT EXISTS membership_plans (
    code TEXT PRIMARY KEY,
    name TEXT NOT NULL,
    duration_days INTEGER NOT NULL,
    is_active BOOLEAN NOT NULL DEFAULT TRUE,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE TABLE IF NOT EXISTS membership_plan_prices (
    plan_code TEXT NOT NULL REFERENCES membership_plans(code) ON DELETE CASCADE,
    chain TEXT NOT NULL,
    asset TEXT NOT NULL,
    amount TEXT NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    PRIMARY KEY (plan_code, chain, asset)
);

CREATE TABLE IF NOT EXISTS membership_orders (
    order_id BIGINT PRIMARY KEY,
    user_email TEXT NOT NULL,
    chain TEXT NOT NULL,
    plan_code TEXT NOT NULL,
    amount TEXT NOT NULL,
    requested_at TIMESTAMPTZ NOT NULL,
    assigned_address TEXT NOT NULL,
    address_expires_at TIMESTAMPTZ NOT NULL,
    status TEXT NOT NULL DEFAULT 'pending',
    paid_at TIMESTAMPTZ,
    tx_hash TEXT,
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE INDEX IF NOT EXISTS idx_membership_orders_user_email
    ON membership_orders ((lower(user_email)));

CREATE INDEX IF NOT EXISTS idx_membership_orders_chain_address
    ON membership_orders (chain, assigned_address);

CREATE TABLE IF NOT EXISTS membership_entitlements (
    user_email TEXT PRIMARY KEY,
    source_order_id BIGINT REFERENCES membership_orders(order_id) ON DELETE SET NULL,
    activated_at TIMESTAMPTZ,
    active_until TIMESTAMPTZ,
    grace_until TIMESTAMPTZ,
    override_status TEXT,
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE TABLE IF NOT EXISTS deposit_address_pool (
    chain TEXT NOT NULL,
    address TEXT NOT NULL,
    is_enabled BOOLEAN NOT NULL DEFAULT TRUE,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    PRIMARY KEY (chain, address)
);

CREATE TABLE IF NOT EXISTS deposit_address_allocations (
    allocation_id BIGSERIAL PRIMARY KEY,
    order_id BIGINT NOT NULL REFERENCES membership_orders(order_id) ON DELETE CASCADE,
    chain TEXT NOT NULL,
    address TEXT NOT NULL,
    expires_at TIMESTAMPTZ NOT NULL,
    released_at TIMESTAMPTZ,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    UNIQUE (order_id)
);

CREATE TABLE IF NOT EXISTS deposit_transactions (
    tx_hash TEXT PRIMARY KEY,
    chain TEXT NOT NULL,
    order_id BIGINT REFERENCES membership_orders(order_id) ON DELETE SET NULL,
    observed_at TIMESTAMPTZ NOT NULL,
    status TEXT NOT NULL,
    raw_payload JSONB NOT NULL DEFAULT '{}'::jsonb,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE TABLE IF NOT EXISTS deposit_order_queue (
    queue_id BIGSERIAL PRIMARY KEY,
    order_id BIGINT NOT NULL UNIQUE REFERENCES membership_orders(order_id) ON DELETE CASCADE,
    chain TEXT NOT NULL,
    enqueued_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE TABLE IF NOT EXISTS fund_sweep_jobs (
    sweep_job_id BIGSERIAL PRIMARY KEY,
    chain TEXT NOT NULL,
    asset TEXT NOT NULL,
    status TEXT NOT NULL,
    requested_by TEXT NOT NULL,
    requested_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    completed_at TIMESTAMPTZ
);

CREATE TABLE IF NOT EXISTS fund_sweep_transfers (
    transfer_id BIGSERIAL PRIMARY KEY,
    sweep_job_id BIGINT NOT NULL REFERENCES fund_sweep_jobs(sweep_job_id) ON DELETE CASCADE,
    from_address TEXT NOT NULL,
    to_address TEXT NOT NULL,
    amount TEXT NOT NULL,
    tx_hash TEXT,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now()
);
