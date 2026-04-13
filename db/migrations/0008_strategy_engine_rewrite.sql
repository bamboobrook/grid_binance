ALTER TABLE strategies
    ADD COLUMN IF NOT EXISTS strategy_type TEXT NOT NULL DEFAULT 'ordinary_grid',
    ADD COLUMN IF NOT EXISTS runtime_phase TEXT NOT NULL DEFAULT 'draft',
    ADD COLUMN IF NOT EXISTS runtime_controls JSONB NOT NULL DEFAULT '{}'::jsonb;

ALTER TABLE strategy_revisions
    ADD COLUMN IF NOT EXISTS strategy_type TEXT NOT NULL DEFAULT 'ordinary_grid',
    ADD COLUMN IF NOT EXISTS reference_price_source TEXT NOT NULL DEFAULT 'manual';

CREATE TABLE IF NOT EXISTS strategy_runtime_level_lots (
    lot_id BIGSERIAL PRIMARY KEY,
    strategy_id TEXT NOT NULL REFERENCES strategies(id) ON DELETE CASCADE,
    revision_id BIGINT REFERENCES strategy_revisions(revision_id) ON DELETE SET NULL,
    level_index INTEGER NOT NULL,
    lot_index INTEGER NOT NULL DEFAULT 0,
    quantity TEXT NOT NULL,
    metadata JSONB NOT NULL DEFAULT '{}'::jsonb,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    UNIQUE (strategy_id, revision_id, level_index, lot_index)
);
