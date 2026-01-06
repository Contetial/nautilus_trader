//! Database schema initialization

/// SQL schema for the database
pub const SCHEMA: &str = r#"
CREATE TABLE IF NOT EXISTS brokers (
    id              TEXT PRIMARY KEY,
    name            TEXT NOT NULL,
    broker_type     TEXT NOT NULL,
    api_key         TEXT NOT NULL,
    api_secret      TEXT NOT NULL,
    user_id         TEXT NOT NULL,
    password        TEXT NOT NULL,
    totp_secret     TEXT,
    access_token    TEXT,
    token_expiry    INTEGER,
    created_at      INTEGER NOT NULL,
    updated_at      INTEGER NOT NULL
);

CREATE TABLE IF NOT EXISTS ai_config (
    key             TEXT PRIMARY KEY,
    value           TEXT NOT NULL,
    updated_at      INTEGER NOT NULL
);
"#;
