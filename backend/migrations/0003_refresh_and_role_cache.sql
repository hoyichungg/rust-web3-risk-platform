-- Extend session + wallet schema for refresh tokens, rolling sessions, and role cache
DELETE FROM user_sessions;

ALTER TABLE user_sessions
    ADD COLUMN refresh_token_hash TEXT NOT NULL DEFAULT '',
    ADD COLUMN refreshed_at TIMESTAMPTZ NOT NULL DEFAULT NOW();

ALTER TABLE user_sessions
    ALTER COLUMN refresh_token_hash DROP DEFAULT;

CREATE UNIQUE INDEX IF NOT EXISTS idx_user_sessions_refresh_hash
    ON user_sessions (refresh_token_hash);

ALTER TABLE wallets
    ADD COLUMN role_cache SMALLINT,
    ADD COLUMN role_cache_updated_at TIMESTAMPTZ;
