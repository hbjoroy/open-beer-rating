# ==============================================================================
# Multi-stage Dockerfile for Open Tappd
# Cross-compiles for the target platform using cargo-zigbuild.
# All compilation stages run natively on the build host (no QEMU).
#
# Single arch:  docker build -t open-tappd .
# Multi-arch:   see scripts/build-multiarch.sh
# ==============================================================================

# ------ Stage 1: Chef (build tools + dependency planning) ------
FROM --platform=$BUILDPLATFORM rust:1.95-trixie AS chef

# Trust corporate CA (Cisco Umbrella) — remove this for CI/cloud builds
COPY cisco.cer /usr/local/share/ca-certificates/cisco.crt
RUN apt-get update && apt-get install -y --no-install-recommends \
    curl xz-utils ca-certificates && \
    update-ca-certificates && \
    rm -rf /var/lib/apt/lists/*

# Install zig (native to build host — zig cross-compiles to any target)
ARG ZIG_VERSION=0.16.0
RUN case "$(uname -m)" in \
      x86_64) ZIG_ARCH=x86_64 ;; \
      aarch64) ZIG_ARCH=aarch64 ;; \
      *) echo "Unsupported arch: $(uname -m)" && exit 1 ;; \
    esac && \
    curl -fsSL "https://ziglang.org/download/${ZIG_VERSION}/zig-${ZIG_ARCH}-linux-${ZIG_VERSION}.tar.xz" \
    | tar xJ -C /opt && \
    ln -s /opt/zig-${ZIG_ARCH}-linux-${ZIG_VERSION}/zig /usr/local/bin/zig

# Install cargo-chef and cargo-zigbuild (pre-built binaries for build host)
ARG CARGO_CHEF_VERSION=0.1.77
ARG CARGO_ZIGBUILD_VERSION=0.22.3
RUN case "$(uname -m)" in \
      x86_64) HOST_TARGET=x86_64-unknown-linux-gnu ;; \
      aarch64) HOST_TARGET=aarch64-unknown-linux-gnu ;; \
      *) echo "Unsupported arch" && exit 1 ;; \
    esac && \
    curl -fsSL "https://github.com/LukeMathWalker/cargo-chef/releases/download/v${CARGO_CHEF_VERSION}/cargo-chef-${HOST_TARGET}.tar.xz" \
    | tar xJ --strip-components=1 -C /usr/local/cargo/bin && \
    curl -fsSL "https://github.com/rust-cross/cargo-zigbuild/releases/download/v${CARGO_ZIGBUILD_VERSION}/cargo-zigbuild-${HOST_TARGET}.tar.xz" \
    | tar xJ --strip-components=1 -C /usr/local/cargo/bin

# Add cross-compilation target for the target platform
ARG TARGETARCH
RUN case "${TARGETARCH:-$(uname -m | sed 's/x86_64/amd64/;s/aarch64/arm64/')}" in \
      amd64) RUST_TARGET=x86_64-unknown-linux-gnu ;; \
      arm64) RUST_TARGET=aarch64-unknown-linux-gnu ;; \
      *) echo "Unsupported target: $TARGETARCH" && exit 1 ;; \
    esac && \
    rustup target add $RUST_TARGET

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

ARG TARGETARCH

COPY --from=planner /app/recipe.json recipe.json

ENV SQLX_OFFLINE=true

# Cook dependencies for the target platform
RUN case "${TARGETARCH:-$(uname -m | sed 's/x86_64/amd64/;s/aarch64/arm64/')}" in \
      amd64) RUST_TARGET=x86_64-unknown-linux-gnu ;; \
      arm64) RUST_TARGET=aarch64-unknown-linux-gnu ;; \
    esac && \
    cargo chef cook --release --recipe-path recipe.json --package open-tappd-api \
    --zigbuild --target $RUST_TARGET

# ------ Stage 4: Build API binary ------
FROM deps AS api-builder

ARG TARGETARCH

# Copy full source
COPY Cargo.toml Cargo.lock ./
COPY crates/ crates/
COPY migrations/ migrations/

ENV SQLX_OFFLINE=true

# Cross-compile the API binary for the target platform
RUN case "${TARGETARCH:-$(uname -m | sed 's/x86_64/amd64/;s/aarch64/arm64/')}" in \
      amd64) RUST_TARGET=x86_64-unknown-linux-gnu ;; \
      arm64) RUST_TARGET=aarch64-unknown-linux-gnu ;; \
    esac && \
    cargo zigbuild --release --package open-tappd-api --target $RUST_TARGET && \
    cp target/$RUST_TARGET/release/open-tappd-api /app/open-tappd-api

# ------ Stage 5: Build WASM frontend (arch-independent) ------
FROM --platform=$BUILDPLATFORM rust:1.95-trixie AS wasm-builder

COPY cisco.cer /usr/local/share/ca-certificates/cisco.crt
RUN apt-get update && apt-get install -y --no-install-recommends \
    curl ca-certificates binaryen && \
    update-ca-certificates && \
    rm -rf /var/lib/apt/lists/*

RUN rustup target add wasm32-unknown-unknown

# Install trunk (pre-built binary for build host)
ARG TRUNK_VERSION=0.21.14
RUN case "$(uname -m)" in \
      x86_64) HOST_TARGET=x86_64-unknown-linux-gnu ;; \
      aarch64) HOST_TARGET=aarch64-unknown-linux-gnu ;; \
      *) echo "Unsupported arch" && exit 1 ;; \
    esac && \
    curl -fsSL "https://github.com/trunk-rs/trunk/releases/download/v${TRUNK_VERSION}/trunk-${HOST_TARGET}.tar.gz" \
    | tar xz -C /usr/local/cargo/bin

WORKDIR /app

# Copy workspace files
COPY Cargo.toml Cargo.lock ./
COPY crates/ crates/

ENV SQLX_OFFLINE=true

# Build the WASM frontend (output is platform-independent)
WORKDIR /app/crates/web
RUN trunk build --release

# ------ Stage 6: Runtime (target platform — lightweight, no compilation) ------
FROM debian:trixie-slim AS runtime

RUN apt-get update && apt-get install -y --no-install-recommends \
    ca-certificates curl && \
    rm -rf /var/lib/apt/lists/*

RUN groupadd --gid 1000 app && \
    useradd --uid 1000 --gid app --shell /bin/bash --create-home app

WORKDIR /app

# Copy the cross-compiled API binary
COPY --from=api-builder /app/open-tappd-api ./

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
