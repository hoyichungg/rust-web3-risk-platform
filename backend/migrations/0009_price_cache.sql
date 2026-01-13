-- Cache latest token prices with TTL to reduce external API calls
CREATE TABLE IF NOT EXISTS price_cache (
    symbol TEXT PRIMARY KEY,
    price NUMERIC NOT NULL,
    fetched_at TIMESTAMPTZ NOT NULL,
    expires_at TIMESTAMPTZ NOT NULL,
    source TEXT NOT NULL DEFAULT 'coingecko',
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX IF NOT EXISTS idx_price_cache_expires_at ON price_cache (expires_at);
