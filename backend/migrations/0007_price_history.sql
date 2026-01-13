-- Store historical price points for backtests/analytics
CREATE TABLE IF NOT EXISTS price_history (
    id UUID PRIMARY KEY,
    symbol TEXT NOT NULL,
    price NUMERIC NOT NULL,
    price_ts TIMESTAMPTZ NOT NULL,
    source TEXT NOT NULL DEFAULT 'coingecko',
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    UNIQUE (symbol, price_ts)
);

CREATE INDEX IF NOT EXISTS idx_price_history_symbol_ts ON price_history (symbol, price_ts DESC);
