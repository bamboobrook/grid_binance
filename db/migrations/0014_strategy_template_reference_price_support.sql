ALTER TABLE strategy_templates
    ADD COLUMN IF NOT EXISTS reference_price TEXT;
