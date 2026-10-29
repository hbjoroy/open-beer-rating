# Temporary test-account email recovery

The owner explicitly trusts the already-registered email addresses of the current
test cohort. Migration `20261029000001` marks only existing non-null email records
with `email_verified_at`, provenance `test_bootstrap_2026_10_29`, and a 14-day
eligibility window. New accounts receive no verification or recovery eligibility.
This is an administrative trust decision, not evidence that those owners previously
completed an email challenge. Successful code redemption changes provenance to
`email_code`. Verification metadata appears in private data export.

Deployment also configures a global `EMAIL_RECOVERY_UNTIL` deadline. Both the
per-account deadline and the global deadline must still be valid. The initial
public deadline is 12 November 2026. Disable Helm `emailRecovery.enabled` to close
earlier. Never rerun the bootstrap UPDATE or enable automatic verification for new
registrations. Before broadening registration, remove this transitional eligibility
and require a normal email verification flow or bjoroy.me identity as appropriate.

## User flow

1. Open `/email-recovery` from sign-in and enter username plus registered email.
2. Receive a random, single-use code by email, valid for 20 minutes. It is usable
   only at the recovery endpoint, never as a normal password/recovery key.
3. Redeem the code to rotate the recovery key and invalidate previous app sessions.
   The new key appears once; save it and explicitly continue through normal login.
4. My Profile strongly recommends linking the bjoroy.me identity before the
   transitional facility closes. Linking still requires its normal OIDC proof and
   explicit confirmation; email recovery does not automatically link identities.

Requests return the same message for unknown, mismatched, unverified, ineligible
and throttled accounts. Responses take at least 350ms; SMTP runs separately.
Limits are one send/minute, three/hour and five/day per account. A new code replaces
the previous one. Five failed attempts lock that code. Codes use the same 120-bit
random generator as recovery keys and are stored as domain-separated HMAC-SHA256
digests. Codes and replacement keys are excluded from logs and redirect URLs.
Browser POSTs require the exact configured HTTPS Origin; sensitive responses use
`Cache-Control: no-store`. Existing passkeys and linked identities remain usable.

## Delivery and session invalidation

The existing Brevo SMTP service is used through lettre with required STARTTLS and
certificate validation. A namespaced Secret contains only the SMTP username and
password. No new mail service is introduced. Sender is Open Drinks at the already
configured bjoroy.me sender address.

Encrypted, short-lived outbox payloads hold recipient/code only until accepted by
SMTP or expired. Workers use PostgreSQL `FOR UPDATE SKIP LOCKED`, a 15-second SMTP
timeout, and at most three attempts. SMTP acceptance is not proof of inbox delivery;
check spam and provider delivery information when diagnosing an actual request.
Request timestamps have a 30-day retention limit. Account deletion cascades all
codes, outbox entries and request history. Private exports include the timestamp
history but exclude transient credentials/outbox payloads.

JWTs include an account authentication version. Old JWTs default to version zero.
Successful recovery increments the version atomically with key replacement, removes
remaining email codes and pending identity flows, and causes protected API requests
from old sessions to fail. Token issuers use the version read with the authenticated
credential, avoiding a stale credential acquiring a newer version during recovery.
Pending OIDC links also retain their starting authentication version and cannot
attach after recovery invalidates it.

Deployment requires a verified backup and restored-copy migration rehearsal. Drain
the old application before reopening requests on the new version, since old code
does not check authentication versions. After any recovery, rollback must retain
version-aware code and changed recovery keys; do not blindly restore the old DB or
old image over new writes. Closing email recovery needs no DB rollback.

Tests cover actual MIME delivery to a local SMTP sink, safe failure/retry, bootstrap
scope, new-account exclusion, mismatched addresses, cooldown/hour limits, expiration,
attempt limits, replay/concurrent redemption, key replacement, session invalidation,
export, account deletion and stale identity-link rejection.

Guidance: [OWASP Forgot Password Cheat Sheet](https://cheatsheetseries.owasp.org/cheatsheets/Forgot_Password_Cheat_Sheet.html),
[lettre SMTP transport](https://docs.rs/lettre/0.11.23/lettre/transport/smtp/).
