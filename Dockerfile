# ── Build stage ──────────────────────────────────────────────────────────────
FROM rust:1.88-slim AS builder

RUN apt-get update && apt-get install -y pkg-config libssl-dev && rm -rf /var/lib/apt/lists/*

WORKDIR /build
COPY . .

RUN cargo build --release -p rustpress-server

# ── Runtime stage ─────────────────────────────────────────────────────────────
FROM debian:bookworm-slim

RUN apt-get update && apt-get install -y ca-certificates libssl3 curl && rm -rf /var/lib/apt/lists/*

WORKDIR /app

COPY --from=builder /build/target/release/rustpress /app/rustpress
COPY --from=builder /build/templates /app/templates
COPY --from=builder /build/static /app/static
COPY --from=builder /build/languages /app/languages
COPY --from=builder /build/themes /app/themes

RUN mkdir -p /app/wp-content/uploads /app/wp-content/plugins

ENV RUSTPRESS_HOST=0.0.0.0
ENV RUSTPRESS_PORT=3000
ENV RUST_LOG=rustpress=info,tower_http=info

EXPOSE 3000

CMD ["/app/rustpress"]
