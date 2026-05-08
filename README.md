# Open Tappd 🍺

A community-owned, privacy-first, open-source beer tasting platform with responsible gamification.

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
- **ORM:** SQLx (runtime-checked queries)

## Features

- 🍺 **Beer & Brewery catalog** — Community-contributed database
- 📊 **0–10 rating scale** — More granularity than 0–5
- 🏆 **Badge system** — First Sip, Explorer, Connoisseur, Style Hunter, Loyal Patron
- 🔒 **Privacy-first** — Granular visibility controls, private by default
- 📦 **Data export** — Full GDPR-compliant data portability
- 🗑️ **Account deletion** — Hard delete with data purge
- 🔐 **Field-level encryption** — AES-256-GCM for email and rating notes

## Getting Started

### Prerequisites

- Rust (stable, 1.75+)
- Docker & Docker Compose (for PostgreSQL)
- [trunk](https://trunkrs.dev/) (for frontend dev server, optional)

### Setup

```bash
# Clone the repository
git clone https://github.com/open-tappd/open-tappd.git
cd open-tappd

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
| POST | `/api/users/register` | No | Register (username + password, email optional) |
| POST | `/api/users/login` | No | Login, returns JWT |
| POST | `/api/breweries` | Yes | Create brewery |
| GET | `/api/breweries` | No | List breweries |
| GET | `/api/breweries/{id}` | No | Get brewery with beers |
| POST | `/api/beers` | Yes | Create beer |
| GET | `/api/beers` | No | List/search beers |
| GET | `/api/beers/{id}` | No | Get beer with aggregate rating |
| POST | `/api/beers/{id}/ratings` | Yes | Rate a beer (0–10) |
| GET | `/api/beers/{id}/ratings` | No | Get aggregate ratings only |
| GET | `/api/users/me/ratings` | Yes | My ratings (private) |
| GET | `/api/users/me/badges` | Yes | My earned badges |
| GET | `/api/users/me/privacy` | Yes | Get privacy settings |
| PUT | `/api/users/me/privacy` | Yes | Update privacy settings |
| GET | `/api/users/me/data-export` | Yes | Export all my data |
| DELETE | `/api/users/me` | Yes | Delete account (irreversible) |

## Project Structure

```
open-tappd/
├── crates/
│   ├── domain/    # Shared types, validation, privacy, crypto
│   ├── api/       # Axum REST API (lib + binary)
│   └── web/       # Leptos WASM frontend
├── migrations/    # SQLx database migrations
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
