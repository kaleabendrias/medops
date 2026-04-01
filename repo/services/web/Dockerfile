FROM rust:bookworm AS builder

WORKDIR /workspace
RUN rustup target add wasm32-unknown-unknown
RUN set -eux; \
    TRUNK_VERSION="0.21.14"; \
    wget -qO- "https://github.com/trunk-rs/trunk/releases/download/v${TRUNK_VERSION}/trunk-x86_64-unknown-linux-gnu.tar.gz" \
      | tar -xzf- -C /usr/local/bin

COPY . .
WORKDIR /workspace/services/web
RUN trunk build --release --public-url /

FROM nginx:1.27-alpine AS runtime

RUN apk add --no-cache wget

COPY services/web/nginx.conf /etc/nginx/conf.d/default.conf
COPY --from=builder /workspace/services/web/dist /usr/share/nginx/html

EXPOSE 8080
CMD ["nginx", "-g", "daemon off;"]
