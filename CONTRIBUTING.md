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
