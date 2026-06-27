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
  │   └─ publishes cert/key + SDS secret file -> FilesystemSink
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

Notable config options in `acme:`:
- `directory_ca_file` (optional): path to a PEM CA bundle to trust when connecting to the ACME directory (e.g. Pebble's self-signed CA in integration tests). Omit to use the system native roots.
- `tick_seconds` (default `60`): how often the renewal state machine timer fires. Set lower in integration environments.

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
 │  │ :8055 (mgmt)   │              │  hashicorp/http-  │  │
 │  └────────────────┘              │  echo ":8080"     │  │
 │                                  └───────────────────┘  │
 └──────────────────────────────────────────────────────────┘
```

What the integration job verifies:
1. Envoy starts with the dynamic module loaded.
2. The module contacts Pebble and completes HTTP-01 challenge validation via the in-process HTTP filter.
3. `FilesystemSink` writes `example.test.cert.pem`, `example.test.key.pem`, and the Envoy SDS secret file `example.test.secret.yaml`.
4. Envoy's HTTPS listener warms up using the SDS secret file and serves traffic.
5. `curl --cacert pebble.minica.pem https://example.test:8443/` returns HTTP 200 with body `hello from upstream`.
6. The certificate SAN contains `DNS:example.test` and chains to Pebble's CA.

`challtestsrv` acts as a programmable DNS server: the CI step registers `example.test → envoy container IP` via its management API on `:8055` before triggering issuance, so Pebble can perform real HTTP-01 validation against Envoy.

## Known limitations

- HTTP-01 only.
- Single instance operation, no leader election.
- `FilesystemSink` only.
- ABI pinned to Envoy `v1.38.0` SDK.
- Panic in dynamic module can still affect proxy process; callbacks are guarded by SDK `catch_unwind` wrappers.
- `directory_cluster` in config is reserved for a future `send_http_callout`-based ACME transport; instant-acme currently owns its own HTTPS transport.
