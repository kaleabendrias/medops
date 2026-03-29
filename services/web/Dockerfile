FROM rust:bookworm AS builder

WORKDIR /workspace
RUN rustup target add wasm32-unknown-unknown
RUN cargo install trunk --locked

COPY . .
WORKDIR /workspace/services/web
RUN trunk build --release --public-url /

FROM nginx:1.27-alpine AS runtime

RUN apk add --no-cache wget

COPY services/web/nginx.conf /etc/nginx/conf.d/default.conf
COPY --from=builder /workspace/services/web/dist /usr/share/nginx/html

EXPOSE 8080
CMD ["nginx", "-g", "daemon off;"]
