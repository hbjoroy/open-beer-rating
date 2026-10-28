-- Originals and metadata are never retained. A tasting has at most one photo,
-- stored as two bounded JPEG variants, and deletion follows its owner tasting.
CREATE TABLE tasting_photos (
    tasting_id UUID PRIMARY KEY REFERENCES tastings(id) ON DELETE CASCADE,
    thumb_jpeg BYTEA NOT NULL CHECK (octet_length(thumb_jpeg) BETWEEN 1 AND 2097152),
    large_jpeg BYTEA NOT NULL CHECK (octet_length(large_jpeg) BETWEEN 1 AND 8388608),
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now()
);
