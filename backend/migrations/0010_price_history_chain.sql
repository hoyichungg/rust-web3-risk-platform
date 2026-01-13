-- Add chain_id to price history for multi-chain disambiguation and better indexing
ALTER TABLE price_history
    ADD COLUMN IF NOT EXISTS chain_id BIGINT NOT NULL DEFAULT 0;

CREATE INDEX IF NOT EXISTS idx_price_history_symbol_chain_ts
    ON price_history (symbol, chain_id, price_ts DESC);
