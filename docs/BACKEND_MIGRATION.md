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
