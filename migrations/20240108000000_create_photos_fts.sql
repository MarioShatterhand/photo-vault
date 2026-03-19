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
