ALTER TABLE strategy_runtime_positions
    ADD COLUMN IF NOT EXISTS exposure_side TEXT;

UPDATE strategy_runtime_positions
SET exposure_side = COALESCE(exposure_side, direction);

ALTER TABLE strategy_runtime_positions
    ALTER COLUMN exposure_side SET DEFAULT 'Buy';

UPDATE strategy_runtime_positions
SET exposure_side = 'Buy'
WHERE exposure_side IS NULL;

ALTER TABLE strategy_runtime_positions
    ALTER COLUMN exposure_side SET NOT NULL;

DO $$
BEGIN
    IF NOT EXISTS (
        SELECT 1
        FROM pg_constraint
        WHERE conname = 'strategy_runtime_positions_exposure_side_check'
    ) THEN
        ALTER TABLE strategy_runtime_positions
            ADD CONSTRAINT strategy_runtime_positions_exposure_side_check
            CHECK (exposure_side IN ('Buy', 'Sell'));
    END IF;
END $$;
