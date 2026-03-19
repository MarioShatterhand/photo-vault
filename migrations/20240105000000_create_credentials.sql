CREATE TABLE credentials (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    user_id INTEGER NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    credential_id TEXT NOT NULL UNIQUE,
    passkey_json TEXT NOT NULL,
    name TEXT NOT NULL DEFAULT 'My Passkey',
    created_at TEXT NOT NULL DEFAULT (datetime('now')),
    last_used TEXT
);
CREATE INDEX idx_credentials_user_id ON credentials(user_id);
