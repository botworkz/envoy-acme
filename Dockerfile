ARG RUST_VERSION=1.93
FROM rust:${RUST_VERSION}-bullseye AS build
RUN apt-get update \
 && apt-get install -y --no-install-recommends \
      clang \
      libclang-dev \
      openssl \
      pkg-config \
 && rm -rf /var/lib/apt/lists/*
WORKDIR /src
COPY . .
RUN cargo build --release --target=x86_64-unknown-linux-gnu

# Self-signed placeholder so Envoy's `path_config_source` resolves at
# startup. FilesystemSink overwrites these atomically on first ACME
# success; Envoy's file watcher reloads the new material in place.
RUN mkdir -p /sds-placeholder \
 && openssl req -x509 -newkey rsa:2048 -nodes \
      -keyout /sds-placeholder/example.test.key.pem \
      -out    /sds-placeholder/example.test.cert.pem \
      -days 1 -subj '/CN=example.test' \
      -addext 'subjectAltName=DNS:example.test' \
 && chmod 0644 /sds-placeholder/example.test.key.pem \
 && printf '%s\n' \
      'resources:' \
      '  - "@type": type.googleapis.com/envoy.extensions.transport_sockets.tls.v3.Secret' \
      '    name: example_test_tls' \
      '    tls_certificate:' \
      '      certificate_chain:' \
      '        filename: /var/lib/envoy-acme/certs/example.test.cert.pem' \
      '      private_key:' \
      '        filename: /var/lib/envoy-acme/certs/example.test.key.pem' \
      > /sds-placeholder/example.test.secret.yaml

FROM envoyproxy/envoy:v1.38-latest
# ENVOY_UID is intentionally not used in the chown below; it exists as an
# override surface for downstream FROM users who rebase on a base image where
# the envoy user is mapped to a different uid (pass --build-arg ENVOY_UID=...).
ARG ENVOY_UID=101
COPY --from=build /src/target/x86_64-unknown-linux-gnu/release/libenvoy_acme.so /etc/envoy/modules/libenvoy_acme.so
COPY --from=build /sds-placeholder/ /var/lib/envoy-acme/certs/
COPY envoy/bootstrap.yaml /etc/envoy/bootstrap.yaml
COPY config/example.yaml /etc/envoy/envoy-acme.yaml
COPY config/pebble-certs /etc/envoy/pebble-certs
# Hand /var/lib/envoy-acme to the envoy user so the unprivileged process
# can write account.json, cert.pem, key.pem and *.secret.yaml. The COPY
# above leaves the tree owned by root:root which would EACCES on first
# tick.
USER root
RUN chown -R envoy:envoy /var/lib/envoy-acme
USER envoy
CMD ["envoy", "-c", "/etc/envoy/bootstrap.yaml", "--service-cluster", "envoy-acme-demo"]
