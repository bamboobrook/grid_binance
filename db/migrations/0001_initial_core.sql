BEGIN;

CREATE TABLE IF NOT EXISTS shared_sequences (
    name TEXT PRIMARY KEY,
    value INTEGER NOT NULL
);

CREATE TABLE IF NOT EXISTS auth_users (
    email TEXT PRIMARY KEY,
    user_id INTEGER NOT NULL UNIQUE,
    password_hash TEXT NOT NULL,
    email_verified INTEGER NOT NULL DEFAULT 0,
    verification_code TEXT,
    reset_code TEXT,
    totp_secret TEXT
);

CREATE TABLE IF NOT EXISTS auth_sessions (
    session_token TEXT PRIMARY KEY,
    email TEXT NOT NULL,
    sid INTEGER NOT NULL,
    FOREIGN KEY (email) REFERENCES auth_users(email) ON DELETE CASCADE
);

CREATE INDEX IF NOT EXISTS idx_auth_sessions_email
    ON auth_sessions(email);

CREATE TABLE IF NOT EXISTS billing_orders (
    order_id INTEGER PRIMARY KEY,
    email TEXT NOT NULL,
    chain TEXT NOT NULL,
    plan_code TEXT NOT NULL,
    amount TEXT NOT NULL,
    requested_at TEXT NOT NULL,
    address TEXT NOT NULL,
    expires_at TEXT NOT NULL,
    paid_at TEXT,
    tx_hash TEXT
);

CREATE INDEX IF NOT EXISTS idx_billing_orders_chain_address
    ON billing_orders(chain, address);

CREATE INDEX IF NOT EXISTS idx_billing_orders_email
    ON billing_orders(email);

CREATE TABLE IF NOT EXISTS seen_transfers (
    tx_hash TEXT PRIMARY KEY,
    chain TEXT NOT NULL,
    observed_at TEXT NOT NULL
);

CREATE TABLE IF NOT EXISTS membership_records (
    email TEXT PRIMARY KEY,
    activated_at TEXT,
    active_until TEXT,
    grace_until TEXT,
    override_status TEXT
);

CREATE TABLE IF NOT EXISTS strategy_templates (
    id TEXT PRIMARY KEY,
    sequence_id INTEGER NOT NULL UNIQUE,
    name TEXT NOT NULL,
    symbol TEXT NOT NULL,
    budget TEXT NOT NULL,
    grid_spacing_bps INTEGER NOT NULL,
    membership_ready INTEGER NOT NULL,
    exchange_ready INTEGER NOT NULL,
    symbol_ready INTEGER NOT NULL
);

CREATE TABLE IF NOT EXISTS strategies (
    id TEXT PRIMARY KEY,
    sequence_id INTEGER NOT NULL UNIQUE,
    owner_email TEXT NOT NULL,
    name TEXT NOT NULL,
    symbol TEXT NOT NULL,
    budget TEXT NOT NULL,
    grid_spacing_bps INTEGER NOT NULL,
    status TEXT NOT NULL,
    source_template_id TEXT,
    membership_ready INTEGER NOT NULL,
    exchange_ready INTEGER NOT NULL,
    symbol_ready INTEGER NOT NULL,
    FOREIGN KEY (source_template_id) REFERENCES strategy_templates(id)
);

CREATE INDEX IF NOT EXISTS idx_strategies_status
    ON strategies(status);

COMMIT;
