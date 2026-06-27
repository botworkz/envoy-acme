# Two-stage build for a small, hermetic envoy-acme image.

# ---- Build stage ----
FROM rust:1.88-bookworm AS builder

WORKDIR /app

# Cache dependencies first.
COPY Cargo.toml Cargo.lock ./
RUN mkdir src && echo 'fn main() {}' > src/main.rs \
    && cargo build --release \
    && rm -rf src

# Build the real sources.
COPY src ./src
RUN touch src/main.rs && cargo build --release

# ---- Runtime stage ----
FROM debian:bookworm-slim

RUN apt-get update \
    && apt-get install -y --no-install-recommends ca-certificates \
    && rm -rf /var/lib/apt/lists/*

RUN useradd --system --uid 10001 --home-dir /var/lib/envoy-acme --create-home envoy-acme

COPY --from=builder /app/target/release/envoy-acme /usr/local/bin/envoy-acme
COPY config/example.yaml /etc/envoy-acme/config.yaml

USER envoy-acme
WORKDIR /var/lib/envoy-acme

EXPOSE 9000 9001

ENTRYPOINT ["/usr/local/bin/envoy-acme"]
CMD ["--config", "/etc/envoy-acme/config.yaml"]
