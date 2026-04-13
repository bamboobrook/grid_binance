ALTER TABLE strategy_templates
    ADD COLUMN IF NOT EXISTS strategy_type TEXT NOT NULL DEFAULT 'ordinary_grid',
    ADD COLUMN IF NOT EXISTS reference_price_source TEXT NOT NULL DEFAULT 'manual';
