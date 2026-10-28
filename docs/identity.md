# bjoroy.me identity and Sprøyt

Open Tappd keeps its local account UUID, recovery key and passkeys. An existing
signed-in user can explicitly attach one authentik identity from My Profile.
After authentik authenticates the user, a confirmation screen names both accounts.
The link does not publish tastings, join circles, or give access to Sprøyt APIs.
Later, Sign in with bjoroy.me authenticates the linked local account. An unlinked
identity must first sign in locally or register, then link from My Profile;
there is no automatic account creation or matching by email/display name.

## Identity contract

- Same authentik instance as Sprøyt: `https://sproyt-security.bjoroy.me`.
- Separate confidential client and secret, provider/application slug `open-tappd`.
- Exact issuer: `https://sproyt-security.bjoroy.me/application/o/open-tappd/`.
- Strict callback: `https://open-drink.bjoroy.me/api/identity/callback`.
- Authorization Code, PKCE S256, `openid profile`, asymmetric RS256 signing.
- Subject mode **hashed_user_id**, matching Sprøyt; per-provider issuer mode.
- In deployed authentik 2026.8.2, both subjects are `user.uid`, independent of
  provider. Do not change either provider's subject mode without a reviewed migration.
- Sprøyt derives its internal UUIDv5 with NAMESPACE_URL from the exact string
  `https://sproyt-security.bjoroy.me/application/o/sproyt/:{subject}`. Open Tappd
  records this same ID after verifying its own provider's token. This is a
  deliberate identity mapping, NOT permission to accept Sprøyt-audience tokens.
  The ID does not prove that a Sprøyt profile or circle membership already exists.

The issuer is pinned in server code. ID token signature, issuer, audience,
expiration, nonce and access-token hash are verified through `openidconnect`.
Discovery and token HTTP redirects are disabled. Linking starts with local bearer
authentication; both starts and confirmation require the exact configured Origin.
Random state and a host-only Secure HttpOnly SameSite=Lax cookie bind authorization
to the browser. Database-backed transactions expire, are single use, work across
replicas, and encrypt transient PKCE/nonce/identity payloads. Tokens never appear in
redirect URLs or localStorage. Expired flows are removed when a new flow starts.
Flow storage is capped. Identity logout retains existing app behavior: the local
in-memory session ends; authentik SSO and Sprøyt sessions are independent.

Uniqueness constraints prevent both duplicate links and overwriting a local
account's identity. A link is included in private account export and cascades on
account deletion. Social eligibility must be read from the server-side link, not
from browser-provided flags. Future social operations need their own consent and
authorization checks; recovery access alone must not imply a recent authentik
authentication for sensitive cross-service operations.

## Deployment

1. Add a separate provider/application without modifying Sprøyt's issuer, client,
   mappings or sessions. Use the same subject mode, asymmetric signing key and
   current access bindings. Keep profile claims small; no avatars/groups required.
2. Place its generated secret only in a namespaced Kubernetes Secret (`client-secret`).
   Configure Helm `identity.enabled`, `clientId`, `existingSecret`, and HTTPS `origin`.
3. Back up the database, apply migration `20261028000003_external_identities.sql`
   with the image's explicit `/app/migrate`, then roll out the app.
4. Check discovery, login redirect, PKCE/state/nonce, callback rejection, recovery
   regressions, and a real user's link and later sign-in. Automated tests cannot
   substitute for user authentication at authentik; never fabricate a production link.

The schema is additive. Disable `identity.enabled` to suspend new identity flows
without losing local sign-in or deleting links. Retain the provider/secret and
backup while investigating a rollback. Do not restore over new user writes.

References: [authentik OAuth2 provider](https://docs.goauthentik.io/add-secure-apps/providers/oauth2/),
[Rust OpenID Connect client](https://docs.rs/openidconnect/4.0.1/openidconnect/).
