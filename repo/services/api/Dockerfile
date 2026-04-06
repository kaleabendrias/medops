FROM rust:bookworm AS builder

WORKDIR /workspace
ENV CARGO_HTTP_TIMEOUT=120
ENV CARGO_NET_RETRY=10

# Layer 1: Cache dependency downloads by copying manifests first
COPY Cargo.toml Cargo.toml
COPY crates/contracts/Cargo.toml crates/contracts/Cargo.toml
COPY services/api/Cargo.toml services/api/Cargo.toml
COPY services/web/Cargo.toml services/web/Cargo.toml

# Create stub source files so cargo can resolve the workspace
RUN mkdir -p crates/contracts/src services/api/src services/web/src \
    && echo "fn main() {}" > services/api/src/main.rs \
    && echo "fn main() {}" > services/web/src/main.rs \
    && touch crates/contracts/src/lib.rs

# Build dependencies only (cached unless Cargo.toml changes)
RUN cargo build --release -p api-service 2>/dev/null || true
RUN cargo build --release -p contracts 2>/dev/null || true

# Layer 2: Copy real source and build
COPY . .
RUN touch services/api/src/main.rs crates/contracts/src/lib.rs \
    && cargo build --release -p api-service

FROM debian:bookworm-slim AS runtime

RUN apt-get update && apt-get install -y --no-install-recommends ca-certificates curl && rm -rf /var/lib/apt/lists/*

WORKDIR /app
COPY --from=builder /workspace/target/release/api-service /usr/local/bin/api-service
COPY --from=builder /workspace/services/api/config /app/config

EXPOSE 8000
CMD ["/usr/local/bin/api-service"]
