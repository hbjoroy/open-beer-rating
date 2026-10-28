-- One explicitly verified external identity per local account. Never match by email.
CREATE TABLE external_identities (
    user_id UUID PRIMARY KEY REFERENCES users(id) ON DELETE CASCADE,
    issuer TEXT NOT NULL,
    subject TEXT NOT NULL,
    display_name TEXT NOT NULL,
    sproyt_user_id UUID NOT NULL UNIQUE,
    linked_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    UNIQUE (issuer, subject)
);

-- Short-lived, browser-bound capabilities; payloads are encrypted with the app key.
CREATE TABLE identity_flows (
    state_hash BYTEA PRIMARY KEY,
    browser_hash BYTEA NOT NULL,
    user_id UUID REFERENCES users(id) ON DELETE CASCADE,
    stage TEXT NOT NULL CHECK (stage IN ('authorize', 'confirm')),
    payload BYTEA NOT NULL,
    expires_at TIMESTAMPTZ NOT NULL
);
CREATE INDEX identity_flows_expiry ON identity_flows(expires_at);
