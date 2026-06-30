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
CI enforces a minimum overall line coverage of **90%** via
`cargo tarpaulin --fail-under 90`. This is a floor, not a ratchet: PRs that
drop coverage below the floor will fail, but drops that stay above it are
allowed. The floor is intentionally set slightly below current coverage to
absorb trivial line-count churn, and is raised manually in dedicated PRs as new
tests land.

## Coverage exclusions

`make coverage` and the CI `coverage` job pass `--exclude-files 'src/lib.rs'`
to `cargo tarpaulin`. This is the only exclusion currently in force.

**Policy.** Files may only be excluded from coverage measurement if they
are genuinely untestable by unit tests — typically because they consist
solely of FFI boundary code that requires a host loader to invoke. Adding
a new exclusion requires:

1. A short paragraph in this section justifying why the file cannot be
   unit-tested.
2. A reference to the integration test or other harness that does cover
   the code (so that "untestable by unit test" does not slide into
   "untested").
3. Reviewer agreement that no reasonable unit-test design would close the
   gap.

Adding a file to the exclusion list to improve the coverage percentage
without satisfying the criteria above is a regression of test discipline.

**Current exclusions:**

- `src/lib.rs` — `#[no_mangle] extern "C"` entry points that satisfy the
  Envoy dynamic-modules ABI. They unpack `*mut c_void` pointers from
  Envoy's loader and cannot be invoked from a Rust test. They are covered
  by the `integration` CI job, which loads the built `.so` into Envoy and
  runs end-to-end ACME issuance against Pebble.

- `src/test_stubs.rs` — stub implementations of Envoy SDK callback symbols
  required to make the test binary link under `cargo tarpaulin`. The SDK's
  `Drop` impls call `envoy_dynamic_module_callback_*` symbols provided by
  Envoy's loader at `dlopen` time; without these stubs the linker fails.
  The stubs contain no logic and cannot be meaningfully unit-tested.

- `src/acme/account_test_server.rs` — in-process TLS mock ACME server used
  as a test fixture by `src/acme/account.rs` tests. This is test
  infrastructure (the thing tests use, not the thing tests assert against),
  so it is not itself unit-testable in the usual sense. Bugs in this file
  surface as failures in the tests that consume it.

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

## Integration test topology

The CI `integration` job validates a real end-to-end certificate issuance flow:

```text
 ┌──────────────────────────────────────────────────────────┐
 │  Docker Compose network                                   │
 │                                                           │
 │  ┌──────────┐  HTTP-01 validation   ┌────────────────┐  │
 │  │  pebble  │ ─────────────────────▶│     envoy      │  │
 │  │  :14000  │                       │  :80 (HTTP)    │  │
 │  └──────────┘                       │  :443 (HTTPS)  │  │
 │       │ DNS query                   └────────────────┘  │
 │       ▼                                      │           │
 │  ┌────────────────┐                          ▼           │
 │  │ challtestsrv   │              ┌───────────────────┐  │
 │  │ :8053 (DNS)    │              │     upstream      │  │
 │  │ :8055 (mgmt)   │              │  envoyproxy/      │  │
 │  └────────────────┘              │  toolshed echo    │  │
 │                                  │  ":8080"          │  │
 │                                  └───────────────────┘  │
 └──────────────────────────────────────────────────────────┘
```

What the integration job verifies:
1. Envoy starts with the dynamic module loaded.
2. The module contacts Pebble and completes HTTP-01 challenge validation for **both** configured domains via the in-process HTTP filter.
3. `FilesystemSink` atomically writes `a.example.test.secret.yaml` (first domain as canonical filename prefix) with the cert chain and private key embedded as `inline_string` values, so Envoy's SDS file watcher sees exactly one filesystem event per renewal.
4. The issued cert contains SANs for **both** `a.example.test` and `b.example.test`.
5. Envoy's HTTPS listener warms up using the SDS secret file and serves traffic.
6. `curl --cacert pebble.minica.pem https://a.example.test:8443/` and `https://b.example.test:8443/` both return HTTP 200.
7. The certificate presented on each SNI name has SANs `[a.example.test, b.example.test]` and chains to Pebble's CA.

`challtestsrv` acts as a programmable DNS server: the CI step registers both `a.example.test` and `b.example.test` to the envoy container's IP via its management API on `:8055` before triggering issuance, so Pebble can perform real HTTP-01 validation for each name against Envoy.
