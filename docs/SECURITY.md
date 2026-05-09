# Security Architecture — Open Tappd

## Threat Model

Open Tappd handles **drinking behavior data** — sensitive personal information that reveals lifestyle habits. The security architecture treats every user's tasting history as confidential by default, with multiple defense layers.

**Assets protected:**
- User credentials (recovery keys, passkey material)
- Personal data (email addresses, rating notes)
- Drinking behavior (which beers a user has rated, when, and how)
- Session tokens (JWTs)

**Trust boundaries:**
```
┌─────────────────────────────────────────────────────┐
│  Browser (WASM)                                     │
│  ┌────────────┐  ┌─────────────────────────────┐    │
│  │ Leptos UI  │  │ WebAuthn API                 │    │
│  │            │  │ navigator.credentials.*      │    │
│  └─────┬──────┘  └──────────┬──────────────────┘    │
│        │ HTTPS/fetch        │ Authenticator (TPM,   │
│        │                    │ Touch ID, etc.)       │
└────────┼────────────────────┼───────────────────────┘
         │ Trust boundary     │
┌────────▼────────────────────▼───────────────────────┐
│  API Server (Axum)                                  │
│  ┌──────────┐  ┌───────────┐  ┌───────────────┐    │
│  │ JWT Auth │  │ WebAuthn  │  │ AES-256-GCM   │    │
│  │ Extractor│  │ RP Logic  │  │ Encryption    │    │
│  └────┬─────┘  └─────┬─────┘  └──────┬────────┘    │
│       │               │               │             │
│  ┌────▼───────────────▼───────────────▼────────┐    │
│  │            SQLx (parameterized queries)     │    │
│  └─────────────────────┬───────────────────────┘    │
└────────────────────────┼────────────────────────────┘
                         │ Trust boundary
┌────────────────────────▼────────────────────────────┐
│  PostgreSQL                                         │
│  - Encrypted fields (email, notes) are BYTEA        │
│  - Passwords stored as Argon2 hashes                │
│  - FK cascades for clean deletion                   │
└─────────────────────────────────────────────────────┘
```

---

## 1. Authentication

### 1.1 Passkey Authentication (Primary)

Passkeys (WebAuthn/FIDO2) are the primary authentication method. See [WEBAUTHN.md](WEBAUTHN.md) for protocol details.

**Security properties:**
- **Phishing-resistant:** Credentials are bound to the RP ID (domain). A credential created for `example.com` cannot be used on `evil.com`
- **No shared secrets:** Only the public key is stored server-side. Private key never leaves the authenticator
- **User verification required:** Biometric or PIN confirmation enforced (`userVerification: "required"`)
- **Replay protection:** Single-use challenges (256-bit random, 5-minute TTL)
- **Cloning detection:** Monotonic counter validation (when supported by authenticator)

### 1.2 Recovery Key (Fallback)

System-generated recovery keys replace user-chosen passwords:

```
Format: XXXX-XXXX-XXXX-XXXX-XXXX-XXXX
Charset: ABCDEFGHJKLMNPQRSTUVWXYZ23456789
         (no I, O, 0, 1 to avoid visual confusion)
Entropy: 6 groups × 4 chars × log₂(32) = 120 bits
```

**Security properties:**
- Generated with `OsRng` (OS-level CSPRNG)
- Hashed with **Argon2id** (memory-hard, GPU-resistant) before storage
- Shown to user exactly once at registration
- Not chosen by user — eliminates weak password risk

### 1.3 JWT Tokens

Custom HS256 HMAC implementation (avoids `jsonwebtoken` crate's `ring`/OpenSSL dependency):

```
Header:  {"alg":"HS256","typ":"JWT"}  (fixed, not parsed from token)
Payload: { sub: UUID, username: String, exp: u64, iat: u64 }
Signing: HMAC-SHA256(header_b64.payload_b64, JWT_SECRET)
```

**Security properties:**
- **24-hour expiry** — tokens are short-lived
- **HMAC verification** — constant-time comparison via `hmac` crate's `verify_slice()`
- **Minimal claims** — only user ID and username (no sensitive data in token)
- **Secret from environment** — `JWT_SECRET` env var, never hardcoded

### 1.4 Auth Extractor Pattern

All authenticated endpoints use the `AuthUser` extractor:

```rust
pub struct AuthUser {
    pub user_id: Uuid,
    pub username: String,
}

#[async_trait]
impl<S> FromRequestParts<S> for AuthUser {
    // Extracts and validates JWT from Authorization: Bearer header
    // Returns 401 if missing, expired, or invalid signature
}
```

**Critical security invariant:** User identity always comes from the verified JWT, never from request parameters. All `/me` endpoints use `auth.user_id`:

```rust
// ✅ Correct: identity from JWT
let ratings = db::ratings::get_user_ratings(&state.pool, auth.user_id, ...).await?;

// ❌ Never done: identity from URL parameter
// GET /api/users/:id/ratings  ← this route does not exist
```

This eliminates Insecure Direct Object Reference (IDOR) vulnerabilities by design.

---

## 2. Data Encryption

### 2.1 Field-Level Encryption (AES-256-GCM)

Sensitive fields are encrypted at the application level before database storage:

| Field | Table | Column Type | Purpose |
|-------|-------|-------------|---------|
| Email | `users` | `email_encrypted BYTEA` | PII protection |
| Rating notes | `ratings` | `notes_encrypted BYTEA` | Behavioral data protection |

**Implementation** (`crates/domain/src/crypto.rs`):

```
Encryption:
  1. Generate 12-byte random nonce (OsRng)
  2. AES-256-GCM encrypt(key, nonce, plaintext)
  3. Store: nonce[12] || ciphertext[N] || tag[16]

Decryption:
  1. Split stored bytes: nonce = [0..12], ciphertext = [12..]
  2. AES-256-GCM decrypt(key, nonce, ciphertext)
```

**Key management:**
- 32-byte key loaded from `ENCRYPTION_KEY` environment variable (base64-encoded)
- Strict 32-byte validation on startup (rejects wrong-length keys)
- Key stored in `AppState` as `[u8; 32]` — in-memory only
- Database has `encryption_keys` table for future key rotation metadata

### 2.2 What the Database Sees

```
users table:
  username:         "alice"              ← plaintext (needed for login)
  email_encrypted:  0x8a3f... (BYTEA)   ← encrypted, opaque to DB
  recovery_key_hash: "$argon2id$..."     ← one-way hash, irreversible

ratings table:
  score:            8                    ← plaintext (needed for aggregation)
  notes_encrypted:  0x7b2e... (BYTEA)   ← encrypted, opaque to DB
```

A database breach exposes usernames and aggregate rating data, but **not** emails, notes, or passwords.

---

## 3. Privacy Architecture

### 3.1 Privacy by Default

New accounts are created with maximum privacy:

```sql
INSERT INTO user_privacy_settings (user_id, profile_visibility, show_ratings, show_badges, show_stats)
VALUES ($1, 'private', false, false, false);
```

Users must explicitly opt-in to share any information.

### 3.2 Visibility Controls

```sql
CREATE TYPE profile_visibility AS ENUM ('public', 'private', 'friends');
```

Per-user toggles: `show_ratings`, `show_badges`, `show_stats` — each independently controllable.

### 3.3 Aggregate-Only Public Data

Beer pages show only aggregate scores (average, count, distribution). Individual ratings are **never** attributed to specific users in public responses:

```rust
// Public: aggregate only
GET /api/beers/:id/ratings → { average: 7.2, count: 42, distribution: [...] }

// Private: own ratings only (requires auth)
GET /api/users/me/ratings → [{ beer_id, score, notes }]
```

There is no `GET /api/users/:id/ratings` endpoint — other users' rating histories cannot be enumerated.

### 3.4 Data Sovereignty (GDPR)

**Data export** (`GET /api/users/me/data-export`):
- Returns all user data as JSON
- Decrypts encrypted fields (email, notes) for the export
- Only accessible by the data owner (auth required)

**Account deletion** (`DELETE /api/users/me`):
- Requires recovery key confirmation
- **Hard delete** — not soft delete
- Cascading FK deletes remove: ratings, badges, privacy settings, passkeys
- Aggregate beer scores are recalculated
- Irreversible — no grace period in current implementation

---

## 4. Database Security

### 4.1 SQL Injection Prevention

All database queries use **SQLx parameterized queries**:

```rust
sqlx::query_as::<_, UserRow>(
    "SELECT id, username, ... FROM users WHERE username = $1"
)
.bind(&username)  // parameterized, never interpolated
.fetch_optional(pool)
.await
```

SQLx's compile-time query checking (when `SQLX_OFFLINE=false`) provides an additional layer of verification.

### 4.2 Schema Constraints

```sql
-- Unique constraints prevent duplicates
username VARCHAR(30) NOT NULL UNIQUE
credential_id BYTEA NOT NULL UNIQUE

-- Check constraints enforce domain rules
score INTEGER NOT NULL CHECK (score >= 0 AND score <= 10)

-- One rating per user per beer
UNIQUE (user_id, beer_id)

-- Cascading deletes for clean account removal
user_id UUID REFERENCES users(id) ON DELETE CASCADE
```

### 4.3 Ownership Enforcement

Destructive operations include ownership checks at the SQL level:

```sql
-- Delete passkey: must own it
DELETE FROM user_passkeys WHERE id = $1 AND user_id = $2

-- Prevents users from deleting other users' passkeys even if they
-- somehow guess the passkey UUID
```

---

## 5. Transport Security

### 5.1 TLS

- SQLx uses **rustls** for PostgreSQL connections (no native OpenSSL)
- Production deployments should use HTTPS (reverse proxy with TLS termination)
- WebAuthn requires secure context (`https://` or `localhost`) in browsers

### 5.2 CORS

Tower-HTTP CORS middleware is configured on the API router. In development, the trunk dev server proxies `/api/` requests to the backend, avoiding CORS issues.

---

## 6. Secret Management

| Secret | Source | Storage |
|--------|--------|---------|
| `DATABASE_URL` | Environment variable | Process memory |
| `ENCRYPTION_KEY` | Environment variable (base64) | `AppState` as `[u8; 32]` |
| `JWT_SECRET` | Environment variable | `AppState` as `String` |
| `WEBAUTHN_RP_ID` | Environment variable (default: "localhost") | `WebAuthnConfig` |
| `WEBAUTHN_ORIGIN` | Environment variable (default: "http://localhost:8080") | `WebAuthnConfig` |

**All secrets are environment-sourced.** None are hardcoded, committed to git, or stored in the database.

The `.env` file is in `.gitignore`. A `.env.example` template exists without actual secret values.

---

## 7. Attack Surface Summary

| Attack Vector | Mitigation |
|---------------|-----------|
| Password brute-force | No user passwords. Recovery keys are 120-bit random + Argon2 |
| Phishing | Passkeys are origin-bound; cannot be phished |
| SQL injection | Parameterized queries throughout (SQLx) |
| IDOR (accessing other users' data) | All `/me` routes use JWT `user_id`; no user-ID-in-URL |
| Database breach | Emails/notes encrypted with AES-256-GCM; passwords are Argon2 hashes |
| Token theft | 24-hour JWT expiry; no refresh tokens yet |
| Replay attacks | Single-use WebAuthn challenges with 5-minute TTL |
| Credential cloning | WebAuthn counter monotonicity check |
| Privacy enumeration | No public user rating endpoints; aggregate-only beer scores |
| XSS | WASM frontend (Leptos) — no innerHTML; framework handles escaping |

---

## 8. Known Limitations & Future Work

1. **No rate limiting** — login/register endpoints should be rate-limited to prevent brute-force
2. **No refresh tokens** — JWT expiry is 24 hours with no refresh mechanism
3. **Single-server challenge storage** — WebAuthn challenges are in-memory; needs shared storage for horizontal scaling
4. **No CSP headers** — Content Security Policy should be added for production
5. **No audit logging** — security events (failed logins, passkey additions) are not logged to a persistent audit trail
6. **No key rotation** — encryption key rotation infrastructure exists in schema but not implemented
7. **No account lockout** — repeated failed recovery key attempts don't lock the account
8. **Email not verified** — email is optional and never verified (by design for privacy, but limits recovery options)
