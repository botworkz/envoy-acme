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
