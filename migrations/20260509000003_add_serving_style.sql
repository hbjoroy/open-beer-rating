-- Add serving_style to tastings

DO $$ BEGIN
    CREATE TYPE serving_style AS ENUM (
        'draft', 'bottle', 'can', 'cask', 'crowler', 'growler', 'nitro', 'taster', 'other'
    );
EXCEPTION WHEN duplicate_object THEN NULL;
END $$;

ALTER TABLE tastings
    ADD COLUMN IF NOT EXISTS serving_style serving_style;
