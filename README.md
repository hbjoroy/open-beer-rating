# Open Tappd 🍺

A community-owned, privacy-first, open-source beer tasting platform with responsible gamification.

## Why?

Existing beer rating apps are closed-source and centralized. Your drinking data — what you drink, when, how often — is sensitive personal information. Open Tappd puts you in control.

### Principles

- **Privacy by default** — Your activity is private unless you choose to share it
- **Pseudonymous accounts** — No email required to participate
- **Aggregate, not individual** — Public beer ratings show averages, never individual users
- **Your data, your choice** — Export or delete everything at any time
- **Open source** — MIT licensed, community-owned

## Tech Stack

- **Language:** Rust (full stack)
- **Frontend:** Leptos (WebAssembly)
- **Backend:** Axum
- **Database:** PostgreSQL
- **ORM:** SQLx (compile-time checked queries)

## Getting Started

### Prerequisites

- Rust (stable, 1.75+)
- Docker & Docker Compose (for PostgreSQL)

### Setup

```bash
# Clone the repository
git clone https://github.com/open-tappd/open-tappd.git
cd open-tappd

# Start PostgreSQL
docker compose up -d

# Copy environment config
cp .env.example .env

# Build the workspace
cargo build

# Run the API server
cargo run -p open-tappd-api

# Run tests
cargo test
```

## Project Structure

```
open-tappd/
├── crates/
│   ├── domain/    # Shared types, validation, privacy, crypto
│   ├── api/       # Axum REST API
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
