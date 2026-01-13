-- Daily snapshots per wallet + transaction log ingestion + cursors
CREATE TABLE IF NOT EXISTS portfolio_daily_snapshots (
    id UUID PRIMARY KEY,
    wallet_id UUID NOT NULL REFERENCES wallets(id) ON DELETE CASCADE,
    day DATE NOT NULL,
    total_usd_value NUMERIC NOT NULL,
    positions JSONB NOT NULL DEFAULT '[]'::jsonb,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    UNIQUE (wallet_id, day)
);

CREATE TABLE IF NOT EXISTS wallet_transactions (
    id UUID PRIMARY KEY,
    wallet_id UUID NOT NULL REFERENCES wallets(id) ON DELETE CASCADE,
    chain_id BIGINT NOT NULL,
    tx_hash TEXT NOT NULL,
    block_number BIGINT NOT NULL,
    log_index BIGINT NOT NULL,
    asset_symbol TEXT NOT NULL,
    amount NUMERIC NOT NULL,
    usd_value NUMERIC NOT NULL,
    direction TEXT NOT NULL,
    from_address TEXT NOT NULL,
    to_address TEXT NOT NULL,
    block_timestamp TIMESTAMPTZ NOT NULL,
    raw JSONB,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    UNIQUE (wallet_id, tx_hash, log_index)
);

CREATE INDEX IF NOT EXISTS idx_wallet_transactions_wallet ON wallet_transactions (wallet_id, block_number DESC);
CREATE INDEX IF NOT EXISTS idx_wallet_transactions_hash ON wallet_transactions (tx_hash);

CREATE TABLE IF NOT EXISTS wallet_sync_cursors (
    wallet_id UUID PRIMARY KEY REFERENCES wallets(id) ON DELETE CASCADE,
    chain_id BIGINT NOT NULL,
    last_tx_block BIGINT NOT NULL DEFAULT 0,
    last_daily_snapshot DATE,
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);
