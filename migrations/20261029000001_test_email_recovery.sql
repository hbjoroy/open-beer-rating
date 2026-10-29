ALTER TABLE users
    ADD COLUMN auth_version BIGINT NOT NULL DEFAULT 0,
    ADD COLUMN email_verified_at TIMESTAMPTZ,
    ADD COLUMN email_verification_source TEXT,
    ADD COLUMN email_recovery_allowed_until TIMESTAMPTZ;

-- Explicit, one-time trust decision for the existing test cohort only.
-- No default on these columns: later registrations stay unverified/ineligible.
UPDATE users SET email_verified_at=now(),
    email_verification_source='test_bootstrap_2026_10_29',
    email_recovery_allowed_until=now()+interval '14 days'
WHERE email_encrypted IS NOT NULL;

CREATE TABLE email_recovery_codes (
    id UUID PRIMARY KEY,
    user_id UUID NOT NULL UNIQUE REFERENCES users(id) ON DELETE CASCADE,
    code_hash BYTEA NOT NULL,
    attempts INTEGER NOT NULL DEFAULT 0,
    expires_at TIMESTAMPTZ NOT NULL
);
CREATE TABLE email_recovery_requests (
    id BIGSERIAL PRIMARY KEY,
    user_id UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    requested_at TIMESTAMPTZ NOT NULL DEFAULT now()
);
CREATE INDEX email_recovery_requests_user_time ON email_recovery_requests(user_id,requested_at);
CREATE TABLE email_recovery_outbox (
    id UUID PRIMARY KEY REFERENCES email_recovery_codes(id) ON DELETE CASCADE,
    payload BYTEA NOT NULL,
    attempts INTEGER NOT NULL DEFAULT 0,
    next_attempt_at TIMESTAMPTZ NOT NULL DEFAULT now()
);
