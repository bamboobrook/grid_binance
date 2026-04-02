CREATE TABLE IF NOT EXISTS strategies (
    id TEXT PRIMARY KEY,
    sequence_id BIGINT NOT NULL UNIQUE,
    owner_email TEXT NOT NULL,
    name TEXT NOT NULL,
    symbol TEXT NOT NULL,
    budget TEXT NOT NULL,
    grid_spacing_bps INTEGER NOT NULL,
    status TEXT NOT NULL,
    source_template_id TEXT,
    membership_ready BOOLEAN NOT NULL DEFAULT FALSE,
    exchange_ready BOOLEAN NOT NULL DEFAULT FALSE,
    permissions_ready BOOLEAN NOT NULL DEFAULT FALSE,
    withdrawals_disabled BOOLEAN NOT NULL DEFAULT TRUE,
    hedge_mode_ready BOOLEAN NOT NULL DEFAULT FALSE,
    symbol_ready BOOLEAN NOT NULL DEFAULT FALSE,
    filters_ready BOOLEAN NOT NULL DEFAULT FALSE,
    margin_ready BOOLEAN NOT NULL DEFAULT FALSE,
    conflict_ready BOOLEAN NOT NULL DEFAULT FALSE,
    balance_ready BOOLEAN NOT NULL DEFAULT FALSE,
    market TEXT NOT NULL DEFAULT 'Spot',
    mode TEXT NOT NULL DEFAULT 'SpotClassic',
    archived_at TIMESTAMPTZ,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE INDEX IF NOT EXISTS idx_strategies_owner_email
    ON strategies ((lower(owner_email)));

CREATE TABLE IF NOT EXISTS strategy_revisions (
    revision_id BIGSERIAL PRIMARY KEY,
    strategy_id TEXT NOT NULL REFERENCES strategies(id) ON DELETE CASCADE,
    revision_kind TEXT NOT NULL,
    config JSONB NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE TABLE IF NOT EXISTS strategy_grid_levels (
    level_id BIGSERIAL PRIMARY KEY,
    strategy_id TEXT NOT NULL REFERENCES strategies(id) ON DELETE CASCADE,
    revision_id BIGINT REFERENCES strategy_revisions(revision_id) ON DELETE SET NULL,
    level_index INTEGER NOT NULL,
    entry_price TEXT NOT NULL,
    quantity TEXT NOT NULL,
    take_profit_bps INTEGER NOT NULL,
    take_profit_price TEXT NOT NULL,
    trailing_bps INTEGER,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE TABLE IF NOT EXISTS strategy_runtime_positions (
    position_id BIGSERIAL PRIMARY KEY,
    strategy_id TEXT NOT NULL REFERENCES strategies(id) ON DELETE CASCADE,
    market_type TEXT NOT NULL,
    direction TEXT NOT NULL,
    quantity TEXT NOT NULL,
    average_entry_price TEXT NOT NULL,
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE TABLE IF NOT EXISTS strategy_orders (
    order_id TEXT PRIMARY KEY,
    strategy_id TEXT NOT NULL REFERENCES strategies(id) ON DELETE CASCADE,
    exchange_order_id TEXT,
    side TEXT NOT NULL,
    order_type TEXT NOT NULL,
    price TEXT,
    quantity TEXT NOT NULL,
    status TEXT NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE TABLE IF NOT EXISTS strategy_fills (
    fill_id TEXT PRIMARY KEY,
    strategy_id TEXT NOT NULL REFERENCES strategies(id) ON DELETE CASCADE,
    order_id TEXT REFERENCES strategy_orders(order_id) ON DELETE SET NULL,
    price TEXT NOT NULL,
    quantity TEXT NOT NULL,
    fee_amount TEXT,
    fee_asset TEXT,
    realized_pnl TEXT,
    filled_at TIMESTAMPTZ NOT NULL
);

CREATE TABLE IF NOT EXISTS strategy_events (
    event_id BIGSERIAL PRIMARY KEY,
    strategy_id TEXT NOT NULL REFERENCES strategies(id) ON DELETE CASCADE,
    event_type TEXT NOT NULL,
    payload JSONB NOT NULL DEFAULT '{}'::jsonb,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE TABLE IF NOT EXISTS strategy_profit_snapshots (
    snapshot_id BIGSERIAL PRIMARY KEY,
    strategy_id TEXT NOT NULL REFERENCES strategies(id) ON DELETE CASCADE,
    realized_pnl TEXT NOT NULL,
    unrealized_pnl TEXT NOT NULL,
    fees TEXT NOT NULL,
    captured_at TIMESTAMPTZ NOT NULL
);

CREATE TABLE IF NOT EXISTS account_profit_snapshots (
    snapshot_id BIGSERIAL PRIMARY KEY,
    user_email TEXT NOT NULL,
    exchange TEXT NOT NULL,
    realized_pnl TEXT NOT NULL,
    unrealized_pnl TEXT NOT NULL,
    fees TEXT NOT NULL,
    funding TEXT,
    captured_at TIMESTAMPTZ NOT NULL
);

CREATE TABLE IF NOT EXISTS exchange_wallet_snapshots (
    snapshot_id BIGSERIAL PRIMARY KEY,
    user_email TEXT NOT NULL,
    exchange TEXT NOT NULL,
    wallet_type TEXT NOT NULL,
    balances JSONB NOT NULL,
    captured_at TIMESTAMPTZ NOT NULL
);

CREATE TABLE IF NOT EXISTS exchange_account_trade_history (
    trade_id TEXT PRIMARY KEY,
    user_email TEXT NOT NULL,
    exchange TEXT NOT NULL,
    symbol TEXT NOT NULL,
    side TEXT NOT NULL,
    quantity TEXT NOT NULL,
    price TEXT NOT NULL,
    fee_amount TEXT,
    fee_asset TEXT,
    traded_at TIMESTAMPTZ NOT NULL
);

CREATE TABLE IF NOT EXISTS user_exchange_symbol_metadata (
    symbol_metadata_id BIGSERIAL PRIMARY KEY,
    user_email TEXT NOT NULL,
    exchange TEXT NOT NULL,
    market TEXT NOT NULL,
    symbol TEXT NOT NULL,
    status TEXT NOT NULL,
    base_asset TEXT NOT NULL,
    quote_asset TEXT NOT NULL,
    price_precision INTEGER NOT NULL,
    quantity_precision INTEGER NOT NULL,
    min_quantity TEXT NOT NULL,
    min_notional TEXT NOT NULL,
    keywords JSONB NOT NULL DEFAULT '[]'::jsonb,
    metadata JSONB NOT NULL DEFAULT '{}'::jsonb,
    synced_at TIMESTAMPTZ NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    UNIQUE (user_email, exchange, market, symbol)
);

CREATE INDEX IF NOT EXISTS idx_user_exchange_symbol_metadata_user_exchange
    ON user_exchange_symbol_metadata ((lower(user_email)), exchange);

CREATE INDEX IF NOT EXISTS idx_user_exchange_symbol_metadata_symbol_lower
    ON user_exchange_symbol_metadata ((lower(symbol)));
