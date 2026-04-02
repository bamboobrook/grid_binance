ALTER TABLE strategies
    ADD COLUMN IF NOT EXISTS permissions_ready BOOLEAN,
    ADD COLUMN IF NOT EXISTS withdrawals_disabled BOOLEAN,
    ADD COLUMN IF NOT EXISTS hedge_mode_ready BOOLEAN,
    ADD COLUMN IF NOT EXISTS filters_ready BOOLEAN,
    ADD COLUMN IF NOT EXISTS margin_ready BOOLEAN,
    ADD COLUMN IF NOT EXISTS conflict_ready BOOLEAN,
    ADD COLUMN IF NOT EXISTS balance_ready BOOLEAN,
    ADD COLUMN IF NOT EXISTS market TEXT,
    ADD COLUMN IF NOT EXISTS mode TEXT,
    ADD COLUMN IF NOT EXISTS archived_at TIMESTAMPTZ;

UPDATE strategies
SET permissions_ready = COALESCE(permissions_ready, exchange_ready, FALSE),
    withdrawals_disabled = COALESCE(withdrawals_disabled, TRUE),
    hedge_mode_ready = COALESCE(hedge_mode_ready, FALSE),
    filters_ready = COALESCE(filters_ready, TRUE),
    margin_ready = COALESCE(margin_ready, TRUE),
    conflict_ready = COALESCE(conflict_ready, TRUE),
    balance_ready = COALESCE(balance_ready, TRUE),
    market = COALESCE(market, 'Spot'),
    mode = COALESCE(mode, 'SpotClassic');

ALTER TABLE strategies
    ALTER COLUMN permissions_ready SET DEFAULT FALSE,
    ALTER COLUMN permissions_ready SET NOT NULL,
    ALTER COLUMN withdrawals_disabled SET DEFAULT TRUE,
    ALTER COLUMN withdrawals_disabled SET NOT NULL,
    ALTER COLUMN hedge_mode_ready SET DEFAULT FALSE,
    ALTER COLUMN hedge_mode_ready SET NOT NULL,
    ALTER COLUMN filters_ready SET DEFAULT FALSE,
    ALTER COLUMN filters_ready SET NOT NULL,
    ALTER COLUMN margin_ready SET DEFAULT FALSE,
    ALTER COLUMN margin_ready SET NOT NULL,
    ALTER COLUMN conflict_ready SET DEFAULT FALSE,
    ALTER COLUMN conflict_ready SET NOT NULL,
    ALTER COLUMN balance_ready SET DEFAULT FALSE,
    ALTER COLUMN balance_ready SET NOT NULL,
    ALTER COLUMN market SET DEFAULT 'Spot',
    ALTER COLUMN market SET NOT NULL,
    ALTER COLUMN mode SET DEFAULT 'SpotClassic',
    ALTER COLUMN mode SET NOT NULL;

ALTER TABLE strategy_grid_levels
    ADD COLUMN IF NOT EXISTS take_profit_bps INTEGER;

WITH derived_take_profit AS (
    SELECT
        grid_levels.level_id,
        COALESCE(
            NULLIF((revisions.config -> 'levels' -> grid_levels.level_index ->> 'take_profit_bps'), '')::INTEGER,
            CASE
                WHEN grid_levels.take_profit_price IS NOT NULL
                    AND grid_levels.entry_price <> '0'
                THEN ROUND(
                    (
                        (
                            grid_levels.take_profit_price::NUMERIC
                            - grid_levels.entry_price::NUMERIC
                        ) / grid_levels.entry_price::NUMERIC
                    ) * 10000
                )::INTEGER
                ELSE 0
            END
        ) AS take_profit_bps
    FROM strategy_grid_levels AS grid_levels
    LEFT JOIN strategy_revisions AS revisions
        ON revisions.revision_id = grid_levels.revision_id
)
UPDATE strategy_grid_levels AS grid_levels
SET take_profit_bps = derived_take_profit.take_profit_bps
FROM derived_take_profit
WHERE grid_levels.level_id = derived_take_profit.level_id
  AND grid_levels.take_profit_bps IS NULL;

UPDATE strategy_grid_levels
SET take_profit_bps = 0
WHERE take_profit_bps IS NULL;

UPDATE strategy_grid_levels
SET take_profit_price = (
    entry_price::NUMERIC
    * (1 + (take_profit_bps::NUMERIC / 10000))
)::TEXT
WHERE take_profit_price IS NULL
   OR take_profit_price = entry_price;

ALTER TABLE strategy_grid_levels
    ALTER COLUMN take_profit_bps SET DEFAULT 0,
    ALTER COLUMN take_profit_bps SET NOT NULL;

UPDATE strategy_grid_levels
SET take_profit_price = entry_price
WHERE take_profit_price IS NULL;

ALTER TABLE strategy_grid_levels
    ALTER COLUMN take_profit_price SET NOT NULL;

DO $$
BEGIN
    IF NOT EXISTS (
        SELECT 1
        FROM pg_constraint
        WHERE conname = 'strategy_grid_levels_take_profit_bps_nonnegative'
    ) THEN
        ALTER TABLE strategy_grid_levels
            ADD CONSTRAINT strategy_grid_levels_take_profit_bps_nonnegative
            CHECK (take_profit_bps >= 0);
    END IF;
END $$;

DO $$
BEGIN
    IF NOT EXISTS (
        SELECT 1
        FROM pg_constraint
        WHERE conname = 'strategy_grid_levels_trailing_within_take_profit'
    ) THEN
        ALTER TABLE strategy_grid_levels
            ADD CONSTRAINT strategy_grid_levels_trailing_within_take_profit
            CHECK (trailing_bps IS NULL OR (trailing_bps >= 0 AND trailing_bps <= take_profit_bps));
    END IF;
END $$;
