ALTER TABLE strategy_templates
    ADD COLUMN IF NOT EXISTS amount_mode TEXT NOT NULL DEFAULT 'Quote';

ALTER TABLE strategy_templates
    ADD COLUMN IF NOT EXISTS futures_margin_mode TEXT;

ALTER TABLE strategy_templates
    ADD COLUMN IF NOT EXISTS leverage INTEGER;
