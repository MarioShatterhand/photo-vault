-- Multi-user migration: scope photos by user_id.
-- SQLite has no ALTER COLUMN, so we use the recreate-table pattern.
-- FTS5 + triggers must be dropped before the source table and recreated after.

DROP TRIGGER IF EXISTS photos_ai;
DROP TRIGGER IF EXISTS photos_ad;
DROP TRIGGER IF EXISTS photos_au;
DROP TABLE IF EXISTS photos_fts;

CREATE TABLE photos_new (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    user_id INTEGER NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    public_id TEXT NOT NULL,
    filename TEXT NOT NULL,
    original_name TEXT NOT NULL,
    hash TEXT NOT NULL,
    size INTEGER NOT NULL,
    width INTEGER NOT NULL DEFAULT 0,
    height INTEGER NOT NULL DEFAULT 0,
    created_at TEXT NOT NULL DEFAULT (datetime('now'))
);

-- Backfill: assign every pre-existing photo to the first user.
-- Guard: skip the insert entirely if there are no users yet (fresh install).
INSERT INTO photos_new (id, user_id, public_id, filename, original_name, hash, size, width, height, created_at)
SELECT id, (SELECT MIN(id) FROM users), public_id, filename, original_name, hash, size, width, height, created_at
FROM photos
WHERE EXISTS (SELECT 1 FROM users);

DROP TABLE photos;
ALTER TABLE photos_new RENAME TO photos;

CREATE UNIQUE INDEX idx_photos_user_hash ON photos(user_id, hash);
CREATE UNIQUE INDEX idx_photos_public_id ON photos(public_id);
CREATE INDEX idx_photos_user_created ON photos(user_id, created_at DESC);

CREATE VIRTUAL TABLE photos_fts USING fts5(
    original_name,
    content='photos',
    content_rowid='id'
);

INSERT INTO photos_fts(rowid, original_name)
    SELECT id, original_name FROM photos;

CREATE TRIGGER photos_ai AFTER INSERT ON photos BEGIN
    INSERT INTO photos_fts(rowid, original_name) VALUES (new.id, new.original_name);
END;

CREATE TRIGGER photos_ad AFTER DELETE ON photos BEGIN
    INSERT INTO photos_fts(photos_fts, rowid, original_name) VALUES('delete', old.id, old.original_name);
END;

CREATE TRIGGER photos_au AFTER UPDATE ON photos BEGIN
    INSERT INTO photos_fts(photos_fts, rowid, original_name) VALUES('delete', old.id, old.original_name);
    INSERT INTO photos_fts(rowid, original_name) VALUES (new.id, new.original_name);
END;
