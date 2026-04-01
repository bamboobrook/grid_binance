CREATE UNIQUE INDEX IF NOT EXISTS idx_deposit_address_allocations_active
    ON deposit_address_allocations (chain, address)
    WHERE released_at IS NULL;

DO $$
BEGIN
    IF EXISTS (
        SELECT 1
        FROM information_schema.table_constraints
        WHERE table_name = 'deposit_transactions'
          AND constraint_type = 'PRIMARY KEY'
          AND constraint_name = 'deposit_transactions_pkey'
    ) THEN
        ALTER TABLE deposit_transactions DROP CONSTRAINT deposit_transactions_pkey;
    END IF;
END $$;

ALTER TABLE deposit_transactions
    DROP CONSTRAINT IF EXISTS deposit_transactions_chain_tx_hash_key;

ALTER TABLE deposit_transactions
    ADD CONSTRAINT deposit_transactions_pkey PRIMARY KEY (chain, tx_hash);
