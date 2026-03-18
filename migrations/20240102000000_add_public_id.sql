ALTER TABLE photos ADD COLUMN public_id TEXT;
UPDATE photos SET public_id = lower(hex(randomblob(16)));
CREATE UNIQUE INDEX idx_photos_public_id ON photos(public_id);
