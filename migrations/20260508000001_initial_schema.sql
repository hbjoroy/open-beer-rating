CREATE EXTENSION IF NOT EXISTS "pgcrypto";

CREATE TABLE IF NOT EXISTS users (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    username VARCHAR(30) NOT NULL UNIQUE,
    email_encrypted BYTEA,
    password_hash TEXT NOT NULL,
    display_name VARCHAR(100),
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

DO $$ BEGIN
    CREATE TYPE profile_visibility AS ENUM ('public', 'private', 'friends');
EXCEPTION WHEN duplicate_object THEN NULL;
END $$;

CREATE TABLE IF NOT EXISTS user_privacy_settings (
    user_id UUID PRIMARY KEY REFERENCES users(id) ON DELETE CASCADE,
    profile_visibility profile_visibility NOT NULL DEFAULT 'private',
    show_ratings BOOLEAN NOT NULL DEFAULT false,
    show_badges BOOLEAN NOT NULL DEFAULT false,
    show_stats BOOLEAN NOT NULL DEFAULT false,
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE TABLE IF NOT EXISTS breweries (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    name VARCHAR(200) NOT NULL,
    country VARCHAR(100),
    city VARCHAR(100),
    website VARCHAR(500),
    created_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE TABLE IF NOT EXISTS beers (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    brewery_id UUID NOT NULL REFERENCES breweries(id) ON DELETE CASCADE,
    name VARCHAR(200) NOT NULL,
    style VARCHAR(100),
    abv DOUBLE PRECISION CHECK (abv >= 0.0 AND abv <= 100.0),
    description TEXT,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE TABLE IF NOT EXISTS ratings (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    user_id UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    beer_id UUID NOT NULL REFERENCES beers(id) ON DELETE CASCADE,
    score INTEGER NOT NULL CHECK (score >= 0 AND score <= 10),
    notes_encrypted BYTEA,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    UNIQUE (user_id, beer_id)
);

CREATE TABLE IF NOT EXISTS badges (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    name VARCHAR(100) NOT NULL UNIQUE,
    description TEXT NOT NULL,
    icon_url TEXT,
    criteria_type VARCHAR(50) NOT NULL,
    criteria_value INTEGER NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE TABLE IF NOT EXISTS user_badges (
    user_id UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    badge_id UUID NOT NULL REFERENCES badges(id) ON DELETE CASCADE,
    earned_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    PRIMARY KEY (user_id, badge_id)
);

CREATE TABLE IF NOT EXISTS encryption_keys (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    key_version INTEGER NOT NULL UNIQUE,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    active BOOLEAN NOT NULL DEFAULT false
);

-- Indexes for common queries
CREATE INDEX IF NOT EXISTS idx_beers_brewery_id ON beers(brewery_id);
CREATE INDEX IF NOT EXISTS idx_beers_style ON beers(style);
CREATE INDEX IF NOT EXISTS idx_ratings_user_id ON ratings(user_id);
CREATE INDEX IF NOT EXISTS idx_ratings_beer_id ON ratings(beer_id);
CREATE INDEX IF NOT EXISTS idx_user_badges_user_id ON user_badges(user_id);
