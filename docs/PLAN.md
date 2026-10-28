# Open Tappd — Community Beer Tasting Platform

## Current implementation direction (2026-10-28)

The historical phases below are background, not a statement of completion.
First consolidate privacy and tasting storage, and pass real PostgreSQL regression
tests before changing the user interface. `tastings` is the canonical history;
legacy rating endpoints remain compatibility adapters. Public aggregates use the
latest tasting per user, ordered by tasted_at, created_at and ID descending.

Public individual activity is disabled until explicit audience and consent rules
exist. Locations are owner-only; session details and tasting results require
ownership or membership. Joining a session shares its tasting results with that
group, never private notes or another user's personal location.

Future social functionality must be planned in collaboration with **Sprøyt**, the
social network/chat system being developed alongside this project. Discover its
actual APIs and identity model before selecting an integration contract. Plan
explicit account linking, audience selection, consent, revocation/deletion and
failure handling. Do not build a parallel social graph or automatically send
tasting history, notes or location data to Sprøyt.

See [backend migration and verification](BACKEND_MIGRATION.md) for this increment.

### Delivered usability stages (2026-10-28)

- URL-based navigation with browser history, deep links, safe login return paths,
  mobile navigation and an accessible account menu.
- Searchable, paginated beer catalogue (beer or brewery), beer details and one
  shared tasting form. Account-scoped drafts survive navigation; logout clears them.
- Private tasting history with beer filtering, editing and explicit deletion
  confirmation. Editing preserves serving style and location context.
- Session creation, invitation links/codes, selection for new tastings, membership
  departure and owner-controlled ending. Clearing the selected session does not
  leave its membership. Session results use the restricted backend API.
- PostgreSQL regression tests and browser-target compilation gate image builds.

Next: account/privacy controls, real-device passkey testing and a small usability
pilot. Agree the Sprøyt identity and consent contract before adding social sharing.
Network retries still need an idempotency design before offline submission is added.
The historical roadmap below does not override these priorities or the canonical
tasting model.

### Tasting photos (2026-10-28)

One optional private photo per tasting, captured with the camera or selected from
files. Store only two JPEG derivatives in PostgreSQL, linked by a cascading foreign
key; no original file or external media service. Thumbnail: short edge 320px,
long edge at most 640px, JPEG quality 80. Large: short edge 960px, long edge at most
1920px, quality 85. Preserve aspect ratio, do not upscale, and do not crop labels.
A 3:2 photo becomes 480x320 and 1440x960. Extreme aspect ratios use the long-edge cap.

The API accepts JPEG, PNG, WebP and supported HEIC/HEIF still images up to 12MiB
and 40 megapixels. It applies orientation and removes metadata by reencoding
pixels. HEIC decoding uses the pinned pure-Rust heic-rs decoder with tile/SPS
preflight and a 192MiB memory budget; some HEICs above 12MP and uncommon HEIF
features are rejected with a JPEG export suggestion. See the
[HEIC test evidence](../crates/api/tests/fixtures/heic/README.md) for supported
features and real iPhone verification. Private authenticated image routes
use no-store responses; session membership does not grant access to photos. Owner
data exports include both JPEG variants as base64, and deletion removes them.

The tasting is saved before its photo upload. A failed photo upload can be retried
against the saved tasting without creating a second history entry. Pending photos
are kept only in page memory; they do not survive reloading like text drafts do.
The history card reserves a thumbnail slot at its upper right, including a quiet
placeholder for tastings without photos. Long names wrap in the remaining column;
the enlarged image opens across the full card width.

The mobile camera control uses native file capture with a rear-camera hint and a
separate library control. Physical Android camera acceptance is tracked separately
from desktop upload and automated decoder tests; the local smoke-test procedure
is documented in LOCAL_DEV.md.

## Problem Statement

Existing beer rating/tasting apps (Untappd, etc.) are closed-source and centralized. We want to build an open-source, community-owned, **privacy-first** alternative with responsible gamification. Users should be able to register beer products, rate them on a 0–10 scale, and earn badges/achievements for their tasting journey — all while maintaining strong control over their personal data. Drinking behavior is sensitive personal data, and the platform should treat it as such from day one.

## Privacy Principles

1. **Privacy by default:** Ratings are aggregated publicly; individual attribution is never exposed unless the user explicitly opts in.
2. **Pseudonymous accounts:** Email is optional. Users can register with just a username and password.
3. **Granular visibility controls:** Users choose what's visible per category (ratings, badges, profile info).
4. **Data sovereignty (GDPR-ready):** Full account deletion with data purge, data export, right to be forgotten.
5. **Encryption at rest:** Sensitive fields (email, rating notes) are encrypted in the database using application-level encryption.
6. **Minimal data collection:** Only collect what's strictly necessary. No tracking, no analytics telemetry.
7. **Future direction: Signal-level privacy.** Architecture decisions should not preclude end-to-end encryption of user activity in future phases.

## Tech Stack

| Layer       | Choice                     |
|-------------|----------------------------|
| Language    | Rust (full stack)          |
| Frontend    | Leptos (WASM, SSR + CSR)  |
| Backend API | Axum (Tokio ecosystem)    |
| Database    | PostgreSQL                 |
| DB Library  | SQLx (compile-time checks)|
| License     | MIT                        |

## Project Structure — Cargo Workspace

```
open-tappd/
├── Cargo.toml              # Workspace root
├── README.md
├── LICENSE
├── .gitignore
├── migrations/             # SQLx migrations
│   └── ...
├── crates/
│   ├── domain/             # Shared domain types, validation, business logic
│   ├── api/                # Axum backend API
│   ├── web/                # Leptos WASM frontend
│   └── webauthn/           # Pure-Rust WebAuthn server library (separable)
└── docker-compose.yml      # PostgreSQL for local dev
```

## Approach — Phased Delivery

### Phase 1 — Foundation ✅
- Workspace scaffolding, Docker Compose, SQLx migrations, Axum skeleton, Leptos skeleton

### Phase 2 — Core Features & Gamification ✅
- Domain models, user registration/auth, brewery/beer/rating CRUD, badge system
- Leptos frontend: Home, Login, Register, BeerList, BeerDetail, AddBeer, Profile pages
- Integration tests (16 tests)

### Phase 2.5 — Privacy & Data Sovereignty ✅
- AES-256-GCM field-level encryption (email, rating notes)
- Privacy settings API, data export, account deletion (GDPR-ready)

### Phase 3 — Passkey Authentication (In Progress)
Goal: Passkeys as the **only** auth method. System-generated recovery key as fallback. No user-chosen passwords.

#### UX Flow

**Browsing (unauthenticated):**
- Beer list, beer details, aggregate ratings — all fully accessible without login
- Nav shows: Home | Beers | Login | Register

**Registration:**
1. User enters username + optional email
2. Backend creates user with system-generated recovery key
3. Backend initiates passkey registration ceremony
4. Frontend calls `navigator.credentials.create()` → user creates passkey
5. On success: show recovery key **once** with "I've saved my recovery key" confirmation
6. User is logged in (JWT issued)

**Returning visit (happy path):**
- On page load, frontend silently attempts `navigator.credentials.get()`
- If passkey succeeds → user is automatically logged in
- No login page interaction needed

**Login page (recovery + passkey retry):**
- Primary: "Sign in with Passkey" button
- Secondary: Recovery section — username + recovery key fields
- After recovery login → prompt to register new passkey on this device

#### Architecture Decision: Pure-Rust WebAuthn

We implement WebAuthn server-side as a **separate crate** (`open-tappd-webauthn`) using pure-Rust crypto:
- **Motivation:** `webauthn-rs` hard-depends on `openssl-sys` via its X.509 attestation verification. On Windows ARM64 without Perl/OpenSSL, this doesn't build. For passkey auth, we don't need attestation CA verification.
- **Approach:** Implement WebAuthn RP (Relying Party) logic using `p256` (ECDSA), `sha2`, `ciborium` (CBOR), `rand` — all pure Rust.
- **Scope:** Registration ceremony, authentication ceremony, discoverable credentials. Attestation verification limited to `none` and `self` (sufficient for passkeys).
- **Future:** This crate can be extracted to its own repo as an independent library.

### Future Phases
- **Phase 4 — Social Features:** Follow users (with consent), activity feed (opt-in), comments
- **Phase 5 — Advanced Gamification:** Leaderboards (opt-in), seasonal challenges, streaks
- **Phase 6 — Discovery:** Recommendations, style exploration guides
- **Phase 7 — Native Apps:** Dioxus or Tauri
- **Phase 8 — Federation:** ActivityPub for community ownership at protocol level
- **Phase 9 — Signal-Level Privacy:** E2E encryption, zero-knowledge proofs

## Key Decisions & Notes

1. **Rating scale 0–10:** Integer, enforced at DB + domain level
2. **One rating per user per beer:** Upsert semantics
3. **Badge evaluation is synchronous:** Run after each rating
4. **No admin system yet:** Any authenticated user can add breweries/beers
5. **Recovery keys (not passwords):** System-generated, Argon2-hashed, format `XXXX-XXXX-XXXX-XXXX-XXXX-XXXX`
6. **JWT auth:** HS256 HMAC (custom, no `jsonwebtoken` crate to avoid `ring`/OpenSSL)
7. **Privacy by default:** New accounts start with everything private
8. **Pseudonymous accounts:** Email optional, login is username-based
9. **No individual rating attribution:** Public pages show only aggregates
10. **AES-256-GCM field encryption:** Applied to email and rating notes
11. **Hard delete on account removal:** No soft-delete
12. **Pure-Rust crypto stack:** No OpenSSL dependency — rustls for TLS, ring/p256 for WebAuthn
13. **WebAuthn as separable crate:** `open-tappd-webauthn` designed for potential extraction
