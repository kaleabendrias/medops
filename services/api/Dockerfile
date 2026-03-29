FROM rust:bookworm AS builder

WORKDIR /workspace
COPY . .
RUN cargo build --release -p api-service

FROM debian:bookworm-slim AS runtime

RUN apt-get update && apt-get install -y --no-install-recommends ca-certificates curl && rm -rf /var/lib/apt/lists/*

WORKDIR /app
COPY --from=builder /workspace/target/release/api-service /usr/local/bin/api-service
COPY --from=builder /workspace/services/api/config /app/config

EXPOSE 8000
CMD ["/usr/local/bin/api-service"]
