-- Run in a maintenance window with application writers stopped, after taking a
-- restorable database backup. This migration takes exclusive locks and replaces
-- the legacy ratings table. SQLx runs the entire migration in one transaction;
-- do not disable migration transactions or split these statements across runs.
LOCK TABLE ratings, tastings IN ACCESS EXCLUSIVE MODE;

-- An ID collision involving another owner/beer is not reconcilable safely.
DO $$
BEGIN
    IF EXISTS (
        SELECT 1
        FROM ratings r
        JOIN tastings t ON t.id = r.id
        WHERE t.user_id <> r.user_id OR t.beer_id <> r.beer_id
    ) THEN
        RAISE EXCEPTION 'Cannot reconcile ratings: tasting ID belongs to another user or beer';
    END IF;
END $$;

-- Preserve IDs and timestamps for ratings created since the initial copy.
INSERT INTO tastings (id, user_id, beer_id, score, notes_encrypted, tasted_at, created_at, updated_at)
SELECT r.id, r.user_id, r.beer_id, r.score, r.notes_encrypted,
       r.created_at, r.created_at, r.created_at
FROM ratings r
WHERE NOT EXISTS (SELECT 1 FROM tastings t WHERE t.id = r.id);

-- A legacy update may have diverged from the original tasting copy. Preserve
-- both histories: never overwrite the tasting's score, notes, time or context.
-- If an identical historical copy already exists, avoid creating it again.
INSERT INTO tastings (id, user_id, beer_id, score, notes_encrypted, tasted_at, created_at, updated_at)
SELECT gen_random_uuid(), r.user_id, r.beer_id, r.score, r.notes_encrypted,
       r.created_at, r.created_at, r.created_at
FROM ratings r
JOIN tastings original ON original.id = r.id
WHERE (
    original.score IS DISTINCT FROM r.score
    OR original.notes_encrypted IS DISTINCT FROM r.notes_encrypted
    OR original.tasted_at IS DISTINCT FROM r.created_at
    OR original.created_at IS DISTINCT FROM r.created_at
)
AND NOT EXISTS (
    SELECT 1 FROM tastings copy
    WHERE copy.user_id = r.user_id
      AND copy.beer_id = r.beer_id
      AND copy.score = r.score
      AND copy.notes_encrypted IS NOT DISTINCT FROM r.notes_encrypted
      AND copy.tasted_at = r.created_at
      AND copy.created_at = r.created_at
);

CREATE INDEX idx_tastings_user_beer_latest
    ON tastings (user_id, beer_id, tasted_at DESC, created_at DESC, id DESC);

CREATE OR REPLACE VIEW ratings_compat AS
SELECT DISTINCT ON (t.user_id, t.beer_id)
    t.id, t.user_id, t.beer_id, t.score, t.notes_encrypted, t.tasted_at AS created_at
FROM tastings t
ORDER BY t.user_id, t.beer_id, t.tasted_at DESC, t.created_at DESC, t.id DESC;

DROP TABLE ratings;

-- DISTINCT makes ratings_compat non-updatable. This alias retains read access
-- for legacy SQL consumers without leaving a second writable source of truth.
CREATE VIEW ratings AS SELECT * FROM ratings_compat;
