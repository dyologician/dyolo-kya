FROM rust:1.88-slim AS builder

WORKDIR /build

# Copy all Cargo manifests so the dependency graph can be resolved before
# any real source is copied — this keeps the dep-fetch layer cached across
# source-only changes.
COPY Cargo.toml ./
COPY a1-redis/Cargo.toml       a1-redis/Cargo.toml
COPY a1-gateway/Cargo.toml     a1-gateway/Cargo.toml
COPY a1-cli/Cargo.toml         a1-cli/Cargo.toml
COPY a1-pg/Cargo.toml          a1-pg/Cargo.toml
COPY a1-identity/Cargo.toml    a1-identity/Cargo.toml

# Lockfile is optional — present in CI, may be absent in fresh checkouts.
COPY Cargo.loc[k] ./

# Copy studio source files so a1-gateway/build.rs can assemble studio/index.html
# during the dependency-cache build step. Only needs src/ — not the built index.html.
COPY studio/src/ studio/src/

# Stub every workspace member so Cargo can fetch and cache all dependencies
# without needing real source. Includes the bench file declared in Cargo.toml.
RUN mkdir -p benches && touch benches/chain_bench.rs && \
    mkdir -p src && echo "pub fn _stub(){}" > src/lib.rs && \
    for d in a1-redis a1-pg a1-identity; do \
      mkdir -p $d/src && echo "pub fn _stub(){}" > $d/src/lib.rs;\
    done && \
    mkdir -p a1-gateway/src && echo "fn main(){}" > a1-gateway/src/main.rs && \
    mkdir -p a1-cli/src     && echo "fn main(){}" > a1-cli/src/main.rs

RUN cargo build --release -p a1-gateway

# Copy real source and rebuild only what changed.
COPY src/                src/
COPY build.rs            build.rs
COPY cbindgen.toml       cbindgen.toml
COPY benches/            benches/
COPY a1-redis/    a1-redis/
COPY a1-pg/       a1-pg/
COPY a1-identity/ a1-identity/
COPY a1-gateway/  a1-gateway/

RUN touch src/lib.rs a1-gateway/src/main.rs && \
    cargo build --release -p a1-gateway

# ── Runtime ───────────────────────────────────────────────────────────────────
FROM debian:bookworm-slim

RUN apt-get update && apt-get install -y --no-install-recommends \
    ca-certificates libssl3 curl && \
    rm -rf /var/lib/apt/lists/*

COPY --from=builder /build/target/release/a1-gateway /usr/local/bin/a1-gateway

ENV GATEWAY_ADDR=0.0.0.0:8080
EXPOSE 8080

HEALTHCHECK --interval=10s --timeout=5s --retries=3 \
    CMD curl -f http://localhost:8080/healthz || exit 1

# Set A1_SIGNING_KEY_HEX and A1_MAC_KEY_HEX at runtime via environment
# or secrets manager. Omitting them generates ephemeral keys — acceptable for
# development, but certs will be unverifiable after a container restart.
ENTRYPOINT ["/usr/local/bin/a1-gateway"]
