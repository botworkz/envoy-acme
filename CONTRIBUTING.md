# Contributing

## Local checks

```bash
cargo fmt --all
cargo clippy --all-targets -- -D warnings
cargo test
```

## End-to-end stack

```bash
make up
make logs
make down
```

The local stack requires `config/pebble-certs/pebble.minica.pem` to be present.
This file is vendored in the repository — no download step is needed.

The CA cert is used by the envoy container to trust Pebble's self-signed
certificate when contacting the ACME directory, and by the host `curl` invocation
in the integration test to verify the issued certificate.

See `config/pebble-certs/NOTICE` for provenance and licensing details.
