-- Test-data transition: back up first and stop application writers.
-- Shared locations survive the account that contributed them. Do not publish
-- legacy home/online locations or their private metadata into the new catalog.
UPDATE tastings SET location_id = NULL WHERE location_id IN
    (SELECT id FROM locations WHERE location_type IN ('home', 'online'));
UPDATE tasting_sessions SET location_id = NULL WHERE location_id IN
    (SELECT id FROM locations WHERE location_type IN ('home', 'online'));
DELETE FROM locations WHERE location_type IN ('home', 'online');

CREATE FUNCTION catalog_key(text) RETURNS text LANGUAGE sql IMMUTABLE STRICT
AS $$ SELECT lower(regexp_replace(btrim($1), '\s+', ' ', 'g')) $$;

-- Collapse exact catalog duplicates and repoint all references before deletion.
CREATE TEMP TABLE brewery_merges ON COMMIT DROP AS
SELECT id, first_value(id) OVER (
    PARTITION BY catalog_key(name), catalog_key(coalesce(country,'')), catalog_key(coalesce(city,''))
    ORDER BY created_at, id) AS canonical FROM breweries;
UPDATE beers b SET brewery_id = m.canonical FROM brewery_merges m
WHERE b.brewery_id = m.id AND m.id <> m.canonical;
DELETE FROM breweries b USING brewery_merges m WHERE b.id=m.id AND m.id<>m.canonical;

CREATE TEMP TABLE beer_merges ON COMMIT DROP AS
SELECT id, first_value(id) OVER (
    PARTITION BY brewery_id, catalog_key(name), catalog_key(coalesce(style,'')), abv
    ORDER BY created_at, id) AS canonical FROM beers;
UPDATE tastings t SET beer_id=m.canonical FROM beer_merges m WHERE t.beer_id=m.id AND m.id<>m.canonical;
DELETE FROM beers b USING beer_merges m WHERE b.id=m.id AND m.id<>m.canonical;

CREATE TEMP TABLE location_merges ON COMMIT DROP AS
SELECT id, first_value(id) OVER (
    PARTITION BY catalog_key(name), catalog_key(coalesce(metadata->>'city','')),
                 catalog_key(coalesce(metadata->>'country',''))
    ORDER BY is_active DESC, created_at, id) AS canonical FROM locations;
UPDATE tastings t SET location_id=m.canonical FROM location_merges m WHERE t.location_id=m.id AND m.id<>m.canonical;
UPDATE tasting_sessions t SET location_id=m.canonical FROM location_merges m WHERE t.location_id=m.id AND m.id<>m.canonical;
DELETE FROM locations l USING location_merges m WHERE l.id=m.id AND m.id<>m.canonical;

ALTER TABLE locations DROP CONSTRAINT locations_created_by_fkey;
ALTER TABLE locations ALTER COLUMN created_by DROP NOT NULL;
ALTER TABLE locations ADD FOREIGN KEY (created_by) REFERENCES users(id) ON DELETE SET NULL;
ALTER TABLE breweries ADD COLUMN untappd_id bigint UNIQUE CHECK (untappd_id > 0);
ALTER TABLE beers ADD COLUMN untappd_id bigint UNIQUE CHECK (untappd_id > 0);
ALTER TABLE locations ADD COLUMN untappd_id bigint UNIQUE CHECK (untappd_id > 0);
CREATE INDEX breweries_catalog_name ON breweries(catalog_key(name));
CREATE INDEX beers_catalog_name ON beers(brewery_id, catalog_key(name));
CREATE INDEX locations_catalog_name ON locations(catalog_key(name));

-- Private provenance follows the tasting on export/deletion; no raw export or
-- third-party friends, comments or toasts are retained.
CREATE TABLE untappd_checkins (
    user_id uuid NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    checkin_id bigint NOT NULL CHECK (checkin_id > 0),
    tasting_id uuid NOT NULL UNIQUE REFERENCES tastings(id) ON DELETE CASCADE,
    source_encrypted bytea NOT NULL,
    PRIMARY KEY(user_id, checkin_id)
);
