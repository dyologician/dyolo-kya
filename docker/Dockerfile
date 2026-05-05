FROM rust:1.78-slim@sha256:4065b64ab22c828d819a0e2e77b9e68b20623dee8aa15e1987fb57302728ebe5 AS builder

WORKDIR /build

# Cache dependency fetch separately from source compilation.
# Cargo.lock is optional — present in CI, absent in fresh checkouts.
COPY Cargo.toml ./
COPY dyolo-kya-redis/Cargo.toml       dyolo-kya-redis/Cargo.toml
COPY dyolo-kya-gateway/Cargo.toml     dyolo-kya-gateway/Cargo.toml
COPY dyolo-kya-cli/Cargo.toml         dyolo-kya-cli/Cargo.toml
COPY dyolo-kya-pg/Cargo.toml          dyolo-kya-pg/Cargo.toml
COPY dyolo-kya-identity/Cargo.toml    dyolo-kya-identity/Cargo.toml

# Copy lockfile if it exists (Docker ignores missing COPY sources when using glob)
COPY Cargo.loc[k] ./

# Stub every member's lib/bin so Cargo can resolve the dependency graph
RUN for d in . dyolo-kya-redis dyolo-kya-pg dyolo-kya-identity; do \
      mkdir -p $d/src && echo "pub fn _stub(){}" > $d/src/lib.rs; \
    done && \
    mkdir -p dyolo-kya-gateway/src && echo "fn main(){}" > dyolo-kya-gateway/src/main.rs && \
    mkdir -p dyolo-kya-cli/src     && echo "fn main(){}" > dyolo-kya-cli/src/main.rs

RUN cargo build --release -p dyolo-kya-gateway 2>&1 | tail -5 || true

# Now compile from real source
COPY src/             src/
COPY build.rs         build.rs
COPY cbindgen.toml    cbindgen.toml
COPY dyolo-kya-redis/ dyolo-kya-redis/
COPY dyolo-kya-pg/    dyolo-kya-pg/
COPY dyolo-kya-identity/ dyolo-kya-identity/
COPY dyolo-kya-gateway/  dyolo-kya-gateway/

RUN touch src/lib.rs dyolo-kya-gateway/src/main.rs && \
    cargo build --release -p dyolo-kya-gateway

# ── Runtime ───────────────────────────────────────────────────────────────────
FROM debian:bookworm-slim@sha256:865980b94764cc4ba2e384501e3130fb7fa648847bc42d45f5be7f71f57d5d0f

RUN apt-get update && apt-get install -y --no-install-recommends \
    ca-certificates libssl3 curl && \
    rm -rf /var/lib/apt/lists/*

COPY --from=builder /build/target/release/dyolo-kya-gateway /usr/local/bin/dyolo-kya-gateway

ENV GATEWAY_ADDR=0.0.0.0:8080
EXPOSE 8080

HEALTHCHECK CMD curl -f http://localhost:8080/health || exit 1

# DYOLO_SIGNING_KEY_HEX and DYOLO_MAC_KEY_HEX should be set at runtime.
# Omitting them generates ephemeral keys — fine for development, wrong for
# production (certs issued before restart become unverifiable after).

ENTRYPOINT ["/usr/local/bin/dyolo-kya-gateway"]
