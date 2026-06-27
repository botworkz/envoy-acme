# envoy-acme

> ⚠️ Prototype only. This is an experiment to run ACME issuance logic as an Envoy dynamic module.

This repository supersedes the sidecar approach in [PR #1](https://github.com/botworkz/envoy-acme/pull/1) and explores a more in-proxy design using Envoy dynamic modules.

Why this exists:
- Envoy has no first-party ACME flow today: [envoy#96](https://github.com/envoyproxy/envoy/issues/96), [envoy#13828](https://github.com/envoyproxy/envoy/issues/13828).
- Dynamic modules allow the ACME logic and challenge responder to run in-process.

## Sidecar vs dynamic module

| Approach | Pros | Cons |
|---|---|---|
| ext_proc + SDS sidecar (PR #1) | process isolation, simpler blast radius | more deployment pieces, not truly in-proxy |
| dynamic module (this repo) | in-proxy behavior, single `.so` artifact | ABI coupling, panic risk in process |

## Architecture

```text
Envoy main thread
  ├─ Bootstrap extension
  │   ├─ timer -> ACME state machine (single-thread tokio runtime)
  │   └─ publishes cert/key -> FilesystemSink
  └─ Envoy filesystem SDS reloads TLS material from cert dir

Envoy worker thread
  └─ HTTP dynamic module filter
      └─ path prefix check -> ChallengeStore lookup -> local reply (200/404)
```

## Build

```bash
cargo build --release --target=x86_64-unknown-linux-gnu
```

## Run local stack

```bash
make up
make logs
make down
```

## Configuration

See:
- `config/example.yaml`
- `envoy/bootstrap.yaml`

Config bytes are parsed as JSON first, then YAML.

## Known limitations

- HTTP-01 only.
- Single instance operation, no leader election.
- `FilesystemSink` only.
- ABI pinned to Envoy `v1.36.9` SDK.
- Panic in dynamic module can still affect proxy process; callbacks are guarded by SDK `catch_unwind` wrappers.
