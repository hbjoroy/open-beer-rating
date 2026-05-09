-- Tasting Sessions, Locations & Tasting Entity
-- Replaces the simple ratings model with richer tasting context.

-- Enums
DO $$ BEGIN
    CREATE TYPE location_type AS ENUM (
        'bar', 'restaurant', 'brewery_taproom', 'festival',
        'home', 'online', 'other'
    );
EXCEPTION WHEN duplicate_object THEN NULL;
END $$;

DO $$ BEGIN
    CREATE TYPE session_visibility AS ENUM ('private', 'participants', 'public');
EXCEPTION WHEN duplicate_object THEN NULL;
END $$;

-- Locations
CREATE TABLE IF NOT EXISTS locations (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    name VARCHAR(200) NOT NULL,
    location_type location_type NOT NULL,
    metadata JSONB NOT NULL DEFAULT '{}',
    created_by UUID NOT NULL REFERENCES users(id),
    is_active BOOLEAN NOT NULL DEFAULT true,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

-- Tasting Sessions
CREATE TABLE IF NOT EXISTS tasting_sessions (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    name VARCHAR(200) NOT NULL,
    description TEXT,
    location_id UUID REFERENCES locations(id),
    created_by UUID NOT NULL REFERENCES users(id),
    join_code VARCHAR(6) NOT NULL UNIQUE,
    visibility session_visibility NOT NULL DEFAULT 'participants',
    started_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    ended_at TIMESTAMPTZ,
    planned_start TIMESTAMPTZ,
    planned_end TIMESTAMPTZ,
    auto_end_minutes INTEGER NOT NULL DEFAULT 180,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

-- Session Participants
CREATE TABLE IF NOT EXISTS session_participants (
    session_id UUID NOT NULL REFERENCES tasting_sessions(id) ON DELETE CASCADE,
    user_id UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    joined_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    PRIMARY KEY (session_id, user_id)
);

-- Tastings (replaces ratings)
CREATE TABLE IF NOT EXISTS tastings (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    user_id UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    beer_id UUID NOT NULL REFERENCES beers(id) ON DELETE CASCADE,
    score INTEGER NOT NULL CHECK (score >= 0 AND score <= 10),
    notes_encrypted BYTEA,
    location_id UUID REFERENCES locations(id),
    session_id UUID REFERENCES tasting_sessions(id) ON DELETE SET NULL,
    tasted_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

-- Indexes
CREATE INDEX IF NOT EXISTS idx_tastings_user_id ON tastings(user_id);
CREATE INDEX IF NOT EXISTS idx_tastings_beer_id ON tastings(beer_id);
CREATE INDEX IF NOT EXISTS idx_tastings_session_id ON tastings(session_id) WHERE session_id IS NOT NULL;
CREATE INDEX IF NOT EXISTS idx_tastings_tasted_at ON tastings(tasted_at DESC);
CREATE INDEX IF NOT EXISTS idx_sessions_ended_at ON tasting_sessions(ended_at) WHERE ended_at IS NULL;
CREATE INDEX IF NOT EXISTS idx_sessions_join_code ON tasting_sessions(join_code);
CREATE INDEX IF NOT EXISTS idx_locations_type ON locations(location_type);
CREATE INDEX IF NOT EXISTS idx_locations_active ON locations(is_active) WHERE is_active = true;

-- Migrate existing ratings into tastings
INSERT INTO tastings (id, user_id, beer_id, score, notes_encrypted, tasted_at, created_at, updated_at)
SELECT id, user_id, beer_id, score, notes_encrypted, created_at, created_at, created_at
FROM ratings
ON CONFLICT (id) DO NOTHING;

-- Backward-compat view: latest tasting per user per beer (mimics old UNIQUE(user_id, beer_id))
CREATE OR REPLACE VIEW ratings_compat AS
SELECT DISTINCT ON (user_id, beer_id)
    id, user_id, beer_id, score, notes_encrypted, tasted_at AS created_at
FROM tastings
ORDER BY user_id, beer_id, tasted_at DESC;

-- Privacy settings extension
ALTER TABLE user_privacy_settings
    ADD COLUMN IF NOT EXISTS show_tasting_sessions BOOLEAN NOT NULL DEFAULT false;
