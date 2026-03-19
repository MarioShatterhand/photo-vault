CREATE TABLE webauthn_challenges (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    challenge_id TEXT NOT NULL UNIQUE,
    challenge_type TEXT NOT NULL CHECK(challenge_type IN ('registration', 'authentication')),
    state_json TEXT NOT NULL,
    created_at TEXT NOT NULL DEFAULT (datetime('now')),
    expires_at TEXT NOT NULL
);
CREATE INDEX idx_challenges_expires_at ON webauthn_challenges(expires_at);
