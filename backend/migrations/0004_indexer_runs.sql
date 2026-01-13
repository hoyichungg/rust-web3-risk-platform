-- Track indexer sync attempts per wallet for observability
CREATE TABLE IF NOT EXISTS portfolio_indexer_runs (
    id UUID PRIMARY KEY,
    wallet_id UUID NOT NULL REFERENCES wallets(id) ON DELETE CASCADE,
    status TEXT NOT NULL,
    error TEXT,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX IF NOT EXISTS idx_portfolio_indexer_runs_wallet_id
    ON portfolio_indexer_runs (wallet_id, created_at DESC);
