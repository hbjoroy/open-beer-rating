# Shared catalog and Untappd import - 2026-10-29

This section supersedes the location-ownership rules in the older rollout below.
Locations are now a shared catalog for all signed-in users. A contributor is an
optional audit reference, not an access boundary. Tastings, encrypted notes,
photos, and session membership retain their existing access rules. A session's
shared location is visible to its authorized participants. Account deletion
keeps catalog entries and removes the contributor reference.

## Migration and rollback

`20261029000002_shared_catalog_import.sql` is an explicit test-data transition:

- Remove legacy home/online locations and clear their references before sharing
  the remaining catalog. No tasting, score or note is deleted by this cleanup.
- Merge exact normalized brewery/name/geography, beer/brewery/style/ABV, and
  venue/name/city/country duplicates; repoint tasting/session foreign keys first.
- Add unique Untappd IDs to breweries, beers and venues; add account-scoped
  check-in provenance, cascading with the tasting/account.

Before deploying: stop writers, take a fully readable and hash-verified database
backup, rehearse the migration against a restored copy, then run the explicit
migrator and deploy the matching API/web build. Check preserved tasting counts,
merged references, and the import preview. Do not run the old binary against the
new nullable location contributor field. Rollback requires the matching old
binary and pre-migration database together; after new imports, prefer a forward
fix to avoid losing the new history. No production migration is implicit in the
API startup or in the UI build.

## Import contract

Use **My Profile -> Import from Untappd** (`/account/import`). Select the flat
JSON array export, preview it, then confirm importing into the signed-in account.
No user ID is accepted from the file or request. The raw file stays in browser
memory and is not retained by the server or committed to Git.

- `POST /api/imports/untappd/preview`: the raw JSON file, Bearer authorization.
- `POST /api/imports/untappd`: same bytes, Bearer authorization, and the returned
  receipt in `X-Import-Preview`. Receipts bind the file, account, matching decisions
  and report for 30 minutes. Catalog changes require a new preview.
- 16 MiB / 10,000-row bounds. Invalid file structure aborts the import. Each check-in uses a savepoint: unresolved
  catalog identities skip only that check-in; an ambiguous venue omits only the
  venue link. A preview performs the same transaction and rolls it back.
- Source IDs win over names. Unlinked catalog rows can be adopted when their
  normalized names and available geography match. A unique beer name within the
  same brewery is adopted despite differing style descriptions or ABV: existing
  catalog details are retained and original source details preserved privately.
  Multiple candidates or contradictory beer identity are reported as skipped
  check-ins, never as a requirement for the user to edit the catalog. An ambiguous
  venue is omitted while the tasting imports. No fuzzy name merging is performed.
  Distinct source beer IDs remain distinct products, even with identical names.
- A database advisory lock serializes imports across API replicas. Catalog table
  locks cover matching against ordinary catalog writes. Unique constraints back
  the source identities and `(user_id, checkin_id)` idempotency.
- Reimport skips existing check-ins and never overwrites subsequent manual edits.
  Repeated check-in IDs count once; the last copy is used, and conflicting copies
  are explained in the report. Deleting a
  tasting also deletes its provenance, so a later deliberate import can recreate it.
- Dates preserve the instant in UTC. Scores are doubled and rounded half-up to the
  integer 0-10 scale. Missing/null/empty scores are reported and skipped, not
  fabricated as zero. A real numeric zero remains a zero.
- Notes use existing field encryption. The original rating, source IDs, serving
  description, original beer/brewery/venue details, IBU, flavor profiles, purchase venue and photo URL are encrypted
  provenance included in the owner's data export. URLs are not fetched or rendered
  as third-party images. No imported friends, toasts, comments, global scores or
  Untappd badges are stored. Image download is a separate future feature.
- Shared venues carry city/country/state/coordinates. They are searchable by name
  or city; creation reuses a matching normalized name/city/country entry.
- Preview and completion include automatic-resolution notices, skipped check-ins
  and omitted venues. The UI displays a bounded list and offers the full report as
  a private, temporary browser download. The confirmation states how many tastings
  will be imported. Catalog corrections invalidate a preview when exclusions change.

## Verification

The isolated PostgreSQL tests cover three-user catalog reuse, repeated import,
concurrent import, all-or-nothing rollback, ambiguous names, external renames,
distinct source products, score/date conversion, preview account/file/catalog
binding, export/deletion and migration reference preservation. The full workspace
suite and the actual Trunk/WASM build are also checked.

Local opt-in rehearsal (the personal fixture is deliberately absent from CI):

```powershell
$env:DATABASE_URL = 'postgres://postgres:local-review-only@127.0.0.1:55432/opentappd_test'
$env:UNTAPPD_FIXTURE_PATH = 'S:\Source\open-beer-rating\untappd-26-10-28.json'
# Optional, local-only snapshot of the shared catalog to reproduce existing matches:
$env:UNTAPPD_CATALOG_FIXTURE_PATH = 'S:\Source\open-beer-rating\target\production-catalog.json'
cargo test -p open-tappd-api --test untappd_import --features external-untappd-fixture-tests personal_export_three_users -- --ignored --nocapture
```

The supplied 4,240-row export imported 4,234 rated check-ins per account: 2,808
beers, 798 breweries and 709 venues shared by three accounts (12,702 tastings).
Six unrated check-ins were reported; a repeated import added zero. The full file
also completed through the local browser's upload, preview and confirmation UI.
The 2026-10-29 robustness regression additionally seeded the actual production
catalog before importing the full export. All 4,234 rated entries were importable;
Dogs Allowed reused its existing UUID despite IPAImperial versus Untappd's longer
style description. Three accounts shared the resulting catalog and reimport added
zero check-ins. No schema change is required for this follow-up.

---

# Privacy and canonical tastings — 2026-10-28

This increment changes the backend only. The public recent-activity endpoint
returns an empty JSON array for compatibility; it does not query personal data.
Existing profile flags do not constitute consent to publish location or session
context. The current home screen will consequently show its empty-feed state
until the separately planned UI work.

## Access rules

- Beer catalog and aggregates remain public. Aggregates use one latest tasting
  per user, including private users, without individual attribution.
- Locations require authentication and ownership for reading and modification.
  A foreign or inactive location cannot be attached to a new/updated tasting or
  new session. Legacy foreign location context is hidden from history/export.
- Session lists and details require ownership or membership, including sessions
  previously marked public. Public sessions can still be joined by ID; other
  sessions require an invitation code. Membership grants access to group scores,
  not other people's private notes or personal locations.
  Own tasting history retains its session reference/name after leaving; leaving
  revokes access to the group's current details and other members' tasting rows.
- Public individual activity stays disabled pending explicit sharing design with
  Sprøyt. No data is sent to another service in this increment.
- Deleted users' outstanding JWTs are rejected. Account deletion removes owned
  sessions and locations, detaches their context from other users' tastings, and
  preserves those tastings and the shared beer/brewery catalog.

## Storage and API compatibility

`tastings` is the only writable rating history. The `ratings` and `ratings_compat`
SQL views are read-only projections. Legacy POST ratings updates the latest
tasting's score/notes while preserving its time and context; if absent, it creates
a tasting. Concurrent legacy updates serialize per user and beer. Native tasting
POST still appends events; it does not deduplicate network retries.

Latest always means `tasted_at DESC, created_at DESC, id DESC`. Editing a historical
event does not change its place in the tasting chronology. Removing the latest
event exposes the preceding event in the aggregate. Legacy rating lists expose
one row per beer; full history/export contains every tasting.

Badge evaluation uses tasting data. If badge evaluation fails after a successful
save, the save remains successful and the error is logged; badge evaluation is
retried on the next submission. Existing awarded badges are not revoked on edit
or deletion. Neither feature is a background job system.

Export uses a repeatable-read snapshot and has no former 10,000-row cap. It
includes all own tastings, decrypted notes, owned locations (including inactive
ones), created sessions, own memberships, safe passkey metadata, badges and
privacy settings. Legacy `ratings` summaries are retained for compatibility.
Authentication secrets and credential material are excluded.

## Deployment procedure

1. Stop all old application writers and take a restorable PostgreSQL backup.
2. Rehearse on a restored copy and compare history/counts before production.
3. Use the new SQLx migration runner, not individual SQL files or `-Force` replay:
   `cargo run --locked -p open-tappd-api --bin migrate` with an explicit
   `DATABASE_URL`, or `./scripts/migrate-db.ps1 -EnvFile .env` locally.
4. Deploy the new API only after migration succeeds; then verify the access rules,
   aggregate consistency and a sample export before reopening traffic.

The migration takes exclusive locks and runs transactionally. Missing legacy
ratings retain IDs and timestamps. Divergent copies are retained as separate
tastings without overwriting history; an already identical copy is not duplicated.
Collisions involving a different owner or beer abort rather than guessing.

The old script maintained `_script_migrations` instead of SQLx's checksum ledger.
The runner refuses unmatched legacy entries. Such databases need a separately
reviewed baseline reconciliation on a restored copy: verify actual schema and
historical migration contents before establishing SQLx's ledger. Do not manually
mark migrations complete merely to bypass the guard.

Do not roll back to the old binary after conversion: it tries to write the removed
ratings table. Rollback requires restoring the backup while writers remain stopped,
with explicit handling of any writes accepted after deployment. No production
migration or backup restore is performed by the development tests.

## Repeatable verification

Use a dedicated PostgreSQL server: SQLx creates disposable databases per test and
requires database-creation privileges. Never use production credentials for tests.
On Windows inspect `wslc --help` and `wslc run --help`, then start PostgreSQL with a
loopback-only port, for example:

```powershell
wslc run --detach --name open-beer-review-db --publish 127.0.0.1:55432:5432 --env POSTGRES_PASSWORD=local-review-only --env POSTGRES_DB=opentappd_test postgres:17-alpine
$env:DATABASE_URL = 'postgres://postgres:local-review-only@127.0.0.1:55432/opentappd_test'
cargo test --workspace --locked -- --include-ignored
cargo clippy --workspace --all-targets --locked
```

The GitHub build workflow runs the PostgreSQL tests before producing an image.
Existing WebAuthn implementation warnings are not evidence of a full authentication
security audit. This increment does not claim one.

Local verification on 2026-10-28: all 50 tests passed with `--include-ignored`
(25 PostgreSQL integration tests, one migration-history guard test, 23 domain
tests and one doctest). Coverage includes 10,001-entry export, concurrent legacy
updates, deterministic timestamp ties, foreign legacy context redaction,
owner/participant boundaries, account deletion and migration rollback. The
PowerShell migration wrapper succeeded on a fresh dedicated database and on a
second run. Clippy completed with existing warnings; changed Rust files passed
rustfmt checks and the diff passed whitespace checks. Production data, browser
UX and remote CI were not exercised by this verification.

## Tasting photo migration

`20261028000002_tasting_photos.sql` adds one optional photo per tasting as two
bounded JPEG bytea columns. Apply with the same explicit SQLx migration command
before running the updated API. Back up the database as usual; photos are part of
that database backup and owner data export. No media directory or new service is
required. The API never runs migrations implicitly. This migration was exercised
in isolated PostgreSQL tests and the local preview, not production.
