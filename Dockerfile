ARG RUST_VERSION=1.78
FROM rust:${RUST_VERSION} AS build
WORKDIR /src
COPY . .
RUN cargo build --release --target=x86_64-unknown-linux-gnu

FROM envoyproxy/envoy:v1.36-latest
COPY --from=build /src/target/x86_64-unknown-linux-gnu/release/libenvoy_acme.so /etc/envoy/modules/libenvoy_acme.so
COPY envoy/bootstrap.yaml /etc/envoy/bootstrap.yaml
COPY config/example.yaml /etc/envoy/envoy-acme.yaml
COPY config/pebble-certs /etc/envoy/pebble-certs
CMD ["envoy", "-c", "/etc/envoy/bootstrap.yaml", "--service-cluster", "envoy-acme-demo"]
