# Contributing to envoy-acme

> ⚠️ **This is a prototype / play repository.** It exists to explore an idea, not
> to ship production software. Contributions are welcome, but please keep that
> framing in mind.

## Development setup

You need a recent stable Rust toolchain (the project targets Rust **1.88+**).
No system `protoc` is required — Envoy protobuf types come pre-generated from the
[`envoy-types`](https://crates.io/crates/envoy-types) crate (see
[`proto/README.md`](proto/README.md)).

```bash
# Build, lint, format-check and test in one go:
make check

# Or individually:
cargo build
cargo test
cargo clippy --all-targets -- -D warnings
cargo fmt --check
```

## Coding guidelines

- **Errors:** use [`thiserror`](https://docs.rs/thiserror) enums per module
  (see `src/errors.rs`). Reserve [`anyhow`](https://docs.rs/anyhow) for the
  binary boundary in `main.rs`.
- **No `unwrap()`/`expect()`** in library code paths. They are only acceptable in
  tests and in top-level process setup (`main.rs`).
- **Tracing:** instrument the ACME state machine and other meaningful async work
  with [`tracing`](https://docs.rs/tracing) spans.
- **Tests:** keep unit tests close to the code (`#[cfg(test)] mod tests`). Pure,
  testable helpers (path matching, renewal windows, secret encoding) are
  preferred over hard-to-test monoliths.
- **Formatting/lints:** code must pass `cargo fmt --check` and
  `cargo clippy --all-targets -- -D warnings`.

## Submitting changes

1. Fork and branch from `main`.
2. Make your change with accompanying tests.
3. Ensure `make check` passes.
4. Open a pull request describing the change and its motivation.

## Code of conduct

Be kind and constructive. This is a low-stakes experimental repo — assume good
intent.
