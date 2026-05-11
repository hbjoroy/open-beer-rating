# ==============================================================================
# Multi-stage Dockerfile for Open Tappd
# Uses zig linker for fast, portable builds with smart layer caching.
#
# Stage 1: Chef      — cargo-chef for dependency planning
# Stage 2: Deps      — build only dependencies (cached until Cargo.lock changes)
# Stage 3: API build — compile API binary with zig linker
# Stage 4: WASM build — compile frontend WASM with trunk
# Stage 5: Runtime   — minimal Debian image with just the binary + static assets
# ==============================================================================

# ------ Stage 1: Chef (plan dependencies) ------
FROM rust:1.87-bookworm AS chef

RUN apt-get update && apt-get install -y --no-install-recommends \
    curl xz-utils && \
    rm -rf /var/lib/apt/lists/*

# Install zig (used as linker)
ARG ZIG_VERSION=0.14.1
RUN case "$(uname -m)" in \
      x86_64) ZIG_ARCH=x86_64 ;; \
      aarch64) ZIG_ARCH=aarch64 ;; \
      *) echo "Unsupported arch: $(uname -m)" && exit 1 ;; \
    esac && \
    curl -fsSL "https://ziglang.org/download/${ZIG_VERSION}/zig-linux-${ZIG_ARCH}-${ZIG_VERSION}.tar.xz" \
    | tar xJ -C /opt && \
    ln -s /opt/zig-linux-${ZIG_ARCH}-${ZIG_VERSION}/zig /usr/local/bin/zig

# Install cargo-chef and cargo-zigbuild
RUN cargo install cargo-chef cargo-zigbuild --locked

WORKDIR /app

# ------ Stage 2: Plan (captures dependency graph) ------
FROM chef AS planner

COPY Cargo.toml Cargo.lock ./
COPY crates/domain/Cargo.toml crates/domain/Cargo.toml
COPY crates/api/Cargo.toml crates/api/Cargo.toml
COPY crates/web/Cargo.toml crates/web/Cargo.toml
COPY crates/webauthn/Cargo.toml crates/webauthn/Cargo.toml

# Minimal lib.rs stubs so cargo-chef can parse the workspace
RUN mkdir -p crates/domain/src crates/api/src crates/web/src crates/webauthn/src && \
    echo "fn main() {}" > crates/api/src/main.rs && \
    touch crates/domain/src/lib.rs crates/api/src/lib.rs crates/web/src/main.rs crates/webauthn/src/lib.rs

RUN cargo chef prepare --recipe-path recipe.json

# ------ Stage 3: Build dependencies (cached layer) ------
FROM chef AS deps

COPY --from=planner /app/recipe.json recipe.json

ENV SQLX_OFFLINE=true
ENV CC="zig cc"

# Cook dependencies only — this is the expensive step that gets cached
RUN cargo chef cook --release --recipe-path recipe.json --package open-tappd-api \
    --zigbuild

# ------ Stage 4: Build API binary ------
FROM deps AS api-builder

# Copy full source
COPY Cargo.toml Cargo.lock ./
COPY crates/ crates/
COPY migrations/ migrations/

ENV SQLX_OFFLINE=true

# Build the API with zig linker
RUN cargo zigbuild --release --package open-tappd-api

# ------ Stage 5: Build WASM frontend ------
FROM rust:1.87-bookworm AS wasm-builder

RUN rustup target add wasm32-unknown-unknown

# Install trunk and wasm-opt
RUN cargo install trunk --locked && \
    apt-get update && apt-get install -y --no-install-recommends binaryen && \
    rm -rf /var/lib/apt/lists/*

WORKDIR /app

# Copy workspace files
COPY Cargo.toml Cargo.lock ./
COPY crates/ crates/

ENV SQLX_OFFLINE=true

# Build the WASM frontend
WORKDIR /app/crates/web
RUN trunk build --release

# ------ Stage 6: Runtime ------
FROM debian:bookworm-slim AS runtime

RUN apt-get update && apt-get install -y --no-install-recommends \
    ca-certificates curl && \
    rm -rf /var/lib/apt/lists/*

RUN groupadd --gid 1000 app && \
    useradd --uid 1000 --gid app --shell /bin/bash --create-home app

WORKDIR /app

# Copy the API binary
COPY --from=api-builder /app/target/release/open-tappd-api ./

# Copy the WASM frontend
COPY --from=wasm-builder /app/crates/web/dist ./static/

# Copy migrations (applied at startup)
COPY migrations/ ./migrations/

# Set ownership
RUN chown -R app:app /app

USER app

ENV STATIC_DIR=/app/static
ENV API_HOST=0.0.0.0
ENV API_PORT=3000
ENV RUST_LOG=info,open_tappd_api=debug

EXPOSE 3000

HEALTHCHECK --interval=30s --timeout=5s --start-period=10s --retries=3 \
    CMD curl -f http://localhost:3000/health || exit 1

ENTRYPOINT ["./open-tappd-api"]
