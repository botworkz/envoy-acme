# Contributing

## Local checks

```bash
cargo fmt --all
cargo clippy --all-targets -- -D warnings
cargo test
```

## Coverage

Coverage uses `cargo-tarpaulin`.

```bash
cargo install cargo-tarpaulin --locked
make coverage
```

The HTML report is written to `target/tarpaulin/tarpaulin-report.html`.
Coverage is currently unit-test only; the docker compose / Envoy integration flow is not instrumented.
CI enforces a minimum overall line coverage of **60%** via
`cargo tarpaulin --fail-under 60`. This is a floor, not a ratchet: PRs that
drop coverage below the floor will fail, but drops that stay above it are
allowed. The floor is intentionally set slightly below current coverage to
absorb trivial line-count churn, and is raised manually in dedicated PRs as new
tests land.

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
