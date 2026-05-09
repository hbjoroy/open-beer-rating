-- Add passkey support: user_passkeys table + replace password_hash with recovery_key_hash

-- Rename password_hash to recovery_key_hash
ALTER TABLE users RENAME COLUMN password_hash TO recovery_key_hash;

-- Create user_passkeys table
CREATE TABLE IF NOT EXISTS user_passkeys (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    user_id UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    credential_id BYTEA NOT NULL UNIQUE,
    public_key_cbor BYTEA NOT NULL,
    counter INTEGER NOT NULL DEFAULT 0,
    transports TEXT,
    name VARCHAR(100) NOT NULL DEFAULT 'Passkey',
    created_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE INDEX IF NOT EXISTS idx_user_passkeys_user_id ON user_passkeys(user_id);
CREATE INDEX IF NOT EXISTS idx_user_passkeys_credential_id ON user_passkeys(credential_id);
