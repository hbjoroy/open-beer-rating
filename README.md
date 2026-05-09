# Open Tappd 🍺

A community-owned, privacy-first, open-source beer tasting platform with responsible gamification.

> **🤖 Built with AI** — This project is designed and coded collaboratively with an AI agent
> ([GitHub Copilot](https://github.com/features/copilot)). Every commit is co-authored by a human
> and an AI pair-programmer. We believe transparency about AI-assisted development is important —
> if you're curious about how this works in practice, check the commit history.

## Why?

Existing beer rating apps are closed-source and centralized. Your drinking data — what you drink, when, how often — is sensitive personal information. Open Tappd puts you in control.

### Principles

- **Privacy by default** — Your activity is private unless you choose to share it
- **Pseudonymous accounts** — No email required to participate
- **Aggregate, not individual** — Public beer ratings show averages, never individual users
- **Your data, your choice** — Export or delete everything at any time
- **Encryption at rest** — Sensitive fields (email, notes) encrypted with AES-256-GCM
- **Open source** — MIT licensed, community-owned

## Tech Stack

- **Language:** Rust (full stack)
- **Frontend:** Leptos (WebAssembly, CSR)
- **Backend:** Axum
- **Database:** PostgreSQL
- **ORM:** SQLx (compile-time checked queries)
- **Auth:** Custom pure-Rust WebAuthn / passkey authentication

## Features

- 🍺 **Beer & Brewery catalog** — Community-contributed database
- 📊 **0–10 rating scale** — More granularity than 0–5
- 🍻 **Tasting sessions** — Create or join group tasting events with shareable codes
- 📍 **Tasting locations** — Track where you taste — bars, festivals, home, online
- 📝 **Rich tastings** — Notes, venue, session context, and score per tasting
- 🏆 **Badge system** — First Sip, Explorer, Connoisseur, Style Hunter, Loyal Patron
- 🔐 **Passkey authentication** — Passwordless login via WebAuthn / FIDO2
- 🔒 **Privacy-first** — Granular visibility controls, private by default
- 📦 **Data export** — Full GDPR-compliant data portability
- 🗑️ **Account deletion** — Hard delete with data purge
- 🔑 **Field-level encryption** — AES-256-GCM for email and tasting notes

## Getting Started

### Prerequisites

- Rust (stable, 1.75+)
- Docker & Docker Compose (for PostgreSQL)
- [trunk](https://trunkrs.dev/) (for frontend dev server, optional)

### Setup

```bash
# Clone the repository
git clone https://github.com/hbjoroy/open-beer-rating.git
cd open-beer-rating

# Start PostgreSQL
docker compose up -d

# Copy environment config
cp .env.example .env
# Edit .env to set ENCRYPTION_KEY and JWT_SECRET

# Build the workspace
cargo build

# Run the API server
cargo run -p open-tappd-api

# Run unit tests
cargo test

# Run integration tests (requires running PostgreSQL)
cargo test -- --ignored
```

### Frontend Development

```bash
# Build the WASM frontend
cargo build -p open-tappd-web --target wasm32-unknown-unknown

# Or use trunk for dev server with live reload
cd crates/web
trunk serve
```

## API Endpoints

| Method | Path | Auth | Description |
|--------|------|------|-------------|
| GET | `/health` | No | Health check |
| **Auth** | | | |
| POST | `/api/users/register` | No | Register (username + recovery key) |
| POST | `/api/passkeys/register/start` | Yes | Start passkey registration |
| POST | `/api/passkeys/register/finish` | Yes | Complete passkey registration |
| POST | `/api/passkeys/authenticate/start` | No | Start passkey sign-in |
| POST | `/api/passkeys/authenticate/finish` | No | Complete passkey sign-in |
| POST | `/api/users/recover` | No | Recover account with recovery key |
| **Beers & Breweries** | | | |
| POST | `/api/breweries` | Yes | Create brewery |
| GET | `/api/breweries` | No | List breweries |
| GET | `/api/breweries/{id}` | No | Get brewery with beers |
| POST | `/api/beers` | Yes | Create beer |
| GET | `/api/beers?search=` | No | List/search beers by name |
| GET | `/api/beers/{id}` | No | Get beer with aggregate rating |
| **Tastings** | | | |
| POST | `/api/tastings` | Yes | Record a tasting |
| GET | `/api/tastings` | Yes | My tastings (paginated) |
| GET | `/api/tastings/{id}` | Yes | Get a specific tasting |
| DELETE | `/api/tastings/{id}` | Yes | Delete a tasting |
| GET | `/api/tastings/beer/{id}` | No | Beer aggregate (avg, count) |
| **Tasting Sessions** | | | |
| POST | `/api/tasting-sessions` | Yes | Create a session |
| GET | `/api/tasting-sessions` | Yes | List my sessions |
| GET | `/api/tasting-sessions/{id}` | Yes | Get session details |
| POST | `/api/tasting-sessions/{id}/join` | Yes | Join a session |
| POST | `/api/tasting-sessions/join/{code}` | Yes | Join by share code |
| POST | `/api/tasting-sessions/{id}/leave` | Yes | Leave a session |
| POST | `/api/tasting-sessions/{id}/end` | Yes | End a session (creator) |
| GET | `/api/tasting-sessions/{id}/tastings` | Yes | Session's tastings |
| **Locations** | | | |
| POST | `/api/locations` | Yes | Create a location |
| GET | `/api/locations` | Yes | List my locations |
| GET | `/api/locations/{id}` | Yes | Get location details |
| PUT | `/api/locations/{id}` | Yes | Update a location |
| DELETE | `/api/locations/{id}` | Yes | Soft-delete a location |
| **Profile & Privacy** | | | |
| GET | `/api/users/me/badges` | Yes | My earned badges |
| GET | `/api/users/me/privacy` | Yes | Get privacy settings |
| PUT | `/api/users/me/privacy` | Yes | Update privacy settings |
| GET | `/api/users/me/data-export` | Yes | Export all my data |
| DELETE | `/api/users/me` | Yes | Delete account (irreversible) |

## Project Structure

```
open-tappd/
├── crates/
│   ├── domain/     # Shared types, validation, privacy, crypto
│   ├── api/        # Axum REST API (lib + binary)
│   ├── web/        # Leptos WASM frontend
│   └── webauthn/   # Pure-Rust WebAuthn / FIDO2 implementation
├── migrations/     # SQLx database migrations
└── docker-compose.yml
```

## Rating Scale

We use a **0–10 integer scale** (not 0–5 like some other apps). More granularity, clearer differentiation.

| Score | Meaning          |
|-------|------------------|
| 0     | Undrinkable      |
| 1-2   | Poor             |
| 3-4   | Below average    |
| 5     | Average          |
| 6-7   | Good             |
| 8-9   | Excellent        |
| 10    | World class      |

## License

MIT — see [LICENSE](LICENSE) for details.
