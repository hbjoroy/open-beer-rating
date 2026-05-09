# WebAuthn Implementation — Architecture & Technical Description

## Overview

`open-tappd-webauthn` is a **pure-Rust** WebAuthn Relying Party (RP) library implementing the [W3C Web Authentication](https://www.w3.org/TR/webauthn-3/) standard. It was purpose-built because the canonical Rust WebAuthn library (`webauthn-rs`) hard-depends on `openssl-sys`, which doesn't build on Windows ARM64 without Perl + OpenSSL toolchain.

**Design goals:**
- Zero C dependencies — uses only pure-Rust crates
- Correct-by-construction — follows W3C §7.1 (registration) and §7.2 (authentication) step-by-step
- Separable — can be extracted to its own repository/crate
- Minimal scope — supports passkey auth (the 90% use case), not the full attestation CA ecosystem

## Dependency Stack

| Purpose           | Crate       | Why                                        |
|-------------------|-------------|--------------------------------------------|
| ECDSA (P-256)     | `p256`      | ES256 signature verification               |
| SHA-256           | `sha2`      | rpIdHash, clientDataJSON hash              |
| CBOR              | `ciborium`  | Parse attestationObject and COSE keys      |
| Base64url         | `base64`    | Encode/decode challenges, credential IDs   |
| Randomness        | `rand`      | Cryptographic challenge generation         |
| Errors            | `thiserror` | Typed error hierarchy                      |
| Serialization     | `serde`     | JSON protocol types, credential storage    |
| User identifiers  | `uuid`      | User IDs as UUID v4                        |

No `ring`, no `openssl`, no native TLS — all pure Rust.

## Module Architecture

```
open-tappd-webauthn/src/
├── lib.rs              ← Public API: WebAuthn struct, challenge caching
├── config.rs           ← WebAuthnConfig (RP ID, origin, name)
├── proto.rs            ← JSON protocol types (matches browser WebAuthn API)
├── credential.rs       ← StoredCredential, CoseKey, authData binary parser
├── registration.rs     ← Registration ceremony (start + finish)
├── authentication.rs   ← Authentication ceremony (start + finish)
├── crypto.rs           ← ES256 verification, SHA-256, challenge generation
├── cbor.rs             ← CBOR parsing: attestationObject, COSE keys
├── base64url.rs        ← URL-safe base64 encoding (no padding)
└── error.rs            ← WebAuthnError enum with thiserror
```

## Protocol Flow

### Registration Ceremony

```
Browser                          Server (open-tappd-webauthn)
  │                                    │
  │  POST /passkeys/register/start     │
  │  (with JWT auth)                   │
  │──────────────────────────────────→│
  │                                    │ 1. Generate 32-byte random challenge
  │                                    │ 2. Build CreationChallengeResponse:
  │                                    │    - rp: { id: "localhost", name: "Open Tappd" }
  │                                    │    - user: { id: base64url(uuid), name, displayName }
  │                                    │    - pubKeyCredParams: [{ alg: -7 (ES256) }]
  │                                    │    - authenticatorSelection: {
  │                                    │        residentKey: "required",
  │                                    │        userVerification: "required"
  │                                    │      }
  │                                    │    - attestation: "none"
  │                                    │ 3. Store RegistrationState keyed by challenge
  │←──────────────────────────────────│
  │  { challenge: CreationChallengeResponse }
  │                                    │
  │  navigator.credentials.create()    │
  │  User creates passkey (biometric)  │
  │                                    │
  │  POST /passkeys/register/finish    │
  │  { id, rawId, type, response: {    │
  │    clientDataJSON, attestationObject, transports } }
  │──────────────────────────────────→│
  │                                    │ Validation (W3C §7.1):
  │                                    │  1. Decode clientDataJSON (base64url → UTF-8 JSON)
  │                                    │  2. Verify type == "webauthn.create"
  │                                    │  3. Verify challenge matches stored state
  │                                    │  4. Verify origin matches config
  │                                    │  5. Reject cross-origin requests
  │                                    │  7. SHA-256 hash clientDataJSON bytes
  │                                    │  8. CBOR-decode attestationObject → { fmt, authData }
  │                                    │  9. Parse authData binary:
  │                                    │     [0..32]  rpIdHash
  │                                    │     [32]     flags byte
  │                                    │     [33..37] counter (big-endian u32)
  │                                    │     [37+]    attestedCredentialData
  │                                    │ 10. Verify rpIdHash == SHA-256("localhost")
  │                                    │ 11. Verify UP flag (user present)
  │                                    │ 12. Verify UV flag (user verified)
  │                                    │ 13. Validate backup flags (BE=0,BS=1 → reject)
  │                                    │ 15. Verify fmt is "none" or "packed"
  │                                    │ 17. Parse ACD: aaguid[16], credIdLen[2],
  │                                    │     credId[L], COSE pubkey (CBOR)
  │                                    │ 18. Parse COSE key: kty=2, alg=-7, crv=1,
  │                                    │     x[32], y[32]
  │                                    │ 19. Return StoredCredential
  │←──────────────────────────────────│
  │  { id, name }  (stored in DB)      │
```

### Authentication Ceremony

```
Browser                          Server (open-tappd-webauthn)
  │                                    │
  │  POST /passkeys/auth/start         │
  │  (no auth required)                │
  │──────────────────────────────────→│
  │                                    │ 1. Generate 32-byte challenge
  │                                    │ 2. Build RequestChallengeResponse:
  │                                    │    - challenge: base64url(bytes)
  │                                    │    - rpId: "localhost"
  │                                    │    - allowCredentials: [] (discoverable)
  │                                    │    - userVerification: "required"
  │                                    │ 3. Store AuthenticationState
  │←──────────────────────────────────│
  │  { challenge, challenge_id }       │
  │                                    │
  │  navigator.credentials.get()       │
  │  OS shows passkey picker           │
  │  User selects account + biometric  │
  │                                    │
  │  POST /passkeys/auth/finish        │
  │  { id, rawId, type, response: {    │
  │    authenticatorData, clientDataJSON,│
  │    signature, userHandle } }       │
  │──────────────────────────────────→│
  │                                    │ 1. Extract userHandle → UUID → load credentials
  │                                    │ 2. Match rawId to stored credential
  │                                    │ 3. Decode clientDataJSON
  │                                    │ 4. Verify type == "webauthn.get"
  │                                    │ 5. Verify challenge
  │                                    │ 6. Verify origin
  │                                    │ 7. Parse authData (no ACD this time)
  │                                    │ 8. Verify rpIdHash
  │                                    │ 9. Verify UP + UV flags
  │                                    │ 13. Validate backup flags
  │                                    │ 14. verification_data = authData || SHA-256(clientDataJSON)
  │                                    │ 15. Verify ECDSA-P256 signature over verification_data
  │                                    │ 16. Counter validation:
  │                                    │     - if counter > 0 || stored > 0:
  │                                    │       new must be > stored (else: cloning detected)
  │                                    │     - if both 0: skip (synced passkeys)
  │                                    │ 17. Return AuthenticationResult
  │←──────────────────────────────────│
  │  { token, user }  (JWT issued)     │
```

## Binary Formats

### authData Layout (W3C §6.1)

```
Offset  Length  Field
───────────────────────────────────────
0       32      rpIdHash        SHA-256 of RP ID string (UTF-8 bytes)
32      1       flags           Bitmask:
                                  bit 0 (0x01) = UP  (User Present)
                                  bit 2 (0x04) = UV  (User Verified)
                                  bit 3 (0x08) = BE  (Backup Eligible)
                                  bit 4 (0x10) = BS  (Backup State)
                                  bit 6 (0x40) = AT  (Attested Credential Data present)
                                  bit 7 (0x80) = ED  (Extension Data present)
33      4       counter         Big-endian u32 sign counter
37+     var     ACD             Only if AT=1 (registration only):
  37    16        aaguid          Authenticator model UUID
  53    2         credIdLen       Big-endian u16
  55    L         credentialId    Raw bytes
  55+L  var       pubKey          CBOR-encoded COSE key
```

### COSE Key Format (EC2 / ES256)

```cbor
{
  1: 2,      // kty = EC2
  3: -7,     // alg = ES256
  -1: 1,     // crv = P-256
  -2: h'...', // x coordinate (32 bytes)
  -3: h'...'  // y coordinate (32 bytes)
}
```

Parsed into `CoseKey::EC2 { x: Vec<u8>, y: Vec<u8> }`.

### Signature Verification (ES256)

The authenticator signs: `ECDSA-SHA256(authData_bytes || SHA-256(clientDataJSON_raw_bytes))`

Our verification:
```rust
// 1. Reconstruct verification message
let client_data_hash = sha256(&client_data_json_raw_bytes);
let verification_data = [auth_data_bytes, &client_data_hash].concat();

// 2. Build P-256 verifying key from stored (x, y)
let point = EncodedPoint::from_affine_coordinates(x, y, false);
let key = VerifyingKey::from_encoded_point(&point)?;

// 3. Parse DER-encoded ECDSA signature
let sig = Signature::from_der(signature_bytes)?;

// 4. Verify (p256 crate hashes internally with SHA-256)
key.verify(&verification_data, &sig)?;
```

## Challenge Management

Challenges are stored in-memory with TTL:

```
WebAuthn struct
├── reg_challenges:  Mutex<HashMap<Vec<u8>, (RegistrationState, Instant)>>
└── auth_challenges: Mutex<HashMap<Vec<u8>, (AuthenticationState, Instant)>>
```

- **Key:** Raw challenge bytes (32 bytes)
- **TTL:** 300 seconds (5 minutes)
- **Cleanup:** Expired entries pruned on each `start_*` call
- **Single-use:** Challenge removed from cache on `finish_*` (replay protection)

**Security properties:**
- Challenges are 256-bit cryptographically random (`rand::Rng`)
- One-time use: each challenge is consumed exactly once
- Time-limited: expired challenges are rejected
- Memory-only: no challenge state touches the database

## Supported Algorithms and Attestation

| Feature | Support |
|---------|---------|
| ES256 (P-256 ECDSA) | ✅ Full |
| RS256 (RSA PKCS#1) | ❌ Not yet |
| EdDSA (Ed25519) | ❌ Not yet |
| Attestation: none | ✅ Full |
| Attestation: packed (self) | ✅ Accepted without cert chain |
| Attestation: packed (full) | ❌ No CA verification |
| Attestation: fido-u2f | ❌ Not supported |
| Discoverable credentials | ✅ Full |
| Non-discoverable credentials | ✅ Via allowCredentials |
| User verification | ✅ Required by default |
| Counter validation | ✅ Monotonic increase check |
| Backup flag validation | ✅ BE/BS consistency check |

ES256 covers the vast majority of passkeys (Apple, Google, Microsoft authenticators all support it). RS256 support can be added later for hardware security keys.

## Stored Credential Format

Credentials are serialized as JSON via `serde` and stored in the database `public_key_cbor` column (BYTEA):

```json
{
  "credential_id": [/* raw bytes */],
  "public_key": {
    "EC2": {
      "x": [/* 32 bytes */],
      "y": [/* 32 bytes */]
    }
  },
  "counter": 0,
  "transports": ["internal"],
  "user_verified": true,
  "backup_eligible": true,
  "backup_state": true
}
```

## Extractability

The crate is designed for standalone use:
- **No dependency on `open-tappd-domain` or any other workspace crate**
- All types are self-contained with `serde` support
- The `Cargo.toml` includes package metadata (description, keywords, categories)
- To extract: copy `crates/webauthn/` to a new repo, update version, publish to crates.io

## Limitations & Future Work

1. **Single algorithm (ES256):** Most passkeys use ES256, but RS256 (Windows Hello with TPM) and EdDSA should be added for completeness
2. **No attestation CA verification:** We accept any credential without verifying the attestation certificate chain — acceptable for consumer passkey auth, not for high-assurance enterprise deployments
3. **In-memory challenge storage:** Works for single-server deployments; would need Redis/DB-backed storage for horizontal scaling
4. **No extension support:** WebAuthn extensions (credProtect, largeBlob, etc.) are parsed in the flags but not processed
