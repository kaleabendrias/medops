FROM rust:bookworm AS builder

WORKDIR /workspace
ENV CARGO_HTTP_TIMEOUT=120
ENV CARGO_NET_RETRY=10

RUN rustup target add wasm32-unknown-unknown
RUN set -eux; \
    TRUNK_VERSION="0.21.14"; \
    wget -qO- "https://github.com/trunk-rs/trunk/releases/download/v${TRUNK_VERSION}/trunk-x86_64-unknown-linux-gnu.tar.gz" \
      | tar -xzf- -C /usr/local/bin

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

# Pre-fetch and build dependencies (cached unless Cargo.toml changes)
RUN cargo build --release --target wasm32-unknown-unknown -p web-app 2>/dev/null || true
RUN cargo build --release --target wasm32-unknown-unknown -p contracts 2>/dev/null || true

# Layer 2: Copy real source and build
COPY . .
RUN touch services/web/src/main.rs crates/contracts/src/lib.rs
WORKDIR /workspace/services/web
RUN trunk build --release --public-url /

FROM nginx:1.27-alpine AS runtime

RUN apk add --no-cache wget

COPY services/web/nginx.conf /etc/nginx/conf.d/default.conf
COPY --from=builder /workspace/services/web/dist /usr/share/nginx/html

EXPOSE 8080
CMD ["nginx", "-g", "daemon off;"]
