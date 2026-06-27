# envoy-acme

> # ⚠️ PROTOTYPE / PLAY REPOSITORY — DO NOT USE IN PRODUCTION ⚠️
>
> This is an **experimental prototype** exploring how a small Rust service can
> issue and renew Let's Encrypt / ACME certificates for [Envoy](https://www.envoyproxy.io/)
> using Envoy's **external processing (ext_proc)** and **Secret Discovery
> Service (SDS)** APIs. It is unfinished, unaudited, and intentionally minimal.
> Do not rely on it for anything real.

`envoy-acme` is a single Rust binary that combines three cooperating pieces:

1. An **ext_proc gRPC server** that answers ACME HTTP-01 challenges inline, so
   the challenge request never has to reach an upstream.
2. An **ACME manager** (built on [`instant-acme`](https://crates.io/crates/instant-acme))
   that obtains and renews certificates via the HTTP-01 challenge.
3. An **SDS gRPC server** that hot-pushes the issued certificate/key to Envoy,
   so TLS certificates rotate with no restart.

## Architecture

```
                         ┌──────────────────────────────────────────┐
                         │                envoy-acme                  │
                         │                                            │
   ACME server  ◄────────┤  ACME manager  ──►  ChallengeStore         │
 (Let's Encrypt /        │  (instant-acme)     (token → keyAuth)       │
   Pebble)               │        │                    ▲              │
       ▲                 │        │ writes cert         │ reads        │
       │ HTTP-01         │        ▼                    │              │
       │ validation      │   CertStore (watch) ──► SDS server :9001    │
       │                 │        │                    ▲              │
       │                 │        │                    │ ext_proc :9000│
       │                 └────────┼────────────────────┼──────────────┘
       │                          │ SDS push           │ mirror headers
       │                          ▼                    │
   ┌───────────────────────────────────────────────────────────────┐
   │                              Envoy                              │
   │  :80  HTTP  ── ext_proc filter ─► (ACME challenges answered)    │
   │  :443 HTTPS ── TLS via SDS secret "acme_cert" ─► upstream       │
   └───────────────────────────────────────────────────────────────┘
```

Flow:

1. The ACME manager creates an order and registers each HTTP-01 token →
   key-authorization pair in the in-memory `ChallengeStore`.
2. The ACME server fetches `http://<domain>/.well-known/acme-challenge/<token>`.
   Envoy mirrors the request headers to the ext_proc server, which looks up the
   token and returns the key authorization directly (HTTP 200), or 404.
3. Once validated, the manager finalizes the order, downloads the chain, and
   publishes it to the `CertStore`.
4. The SDS server watches the `CertStore` and streams the new `TlsCertificate`
   secret to Envoy, which hot-reloads the listener certificate.

## Proto handling (no `protoc` required)

The build is **hermetic** — only `cargo` is needed. Envoy protobuf types are
consumed from the [`envoy-proto`](https://github.com/phlax/envoy-proto-rs) crate (`phlax/envoy-proto-rs`)
crate, so there is **no system `protoc` dependency** and no vendored protos to
compile. See [`proto/README.md`](proto/README.md) for details.

## Quickstart

### Run locally

```bash
# Build and test.
make build
make test

# Run against a config file (defaults to config/example.yaml).
cargo run -- --config config/example.yaml
```

The service starts:

- ext_proc gRPC on `0.0.0.0:9000`
- SDS gRPC on `0.0.0.0:9001`

### End-to-end with Docker Compose

A full local stack (a [Pebble](https://github.com/letsencrypt/pebble) ACME test
server, `envoy-acme`, Envoy, and a demo upstream) is provided:

```bash
make up      # docker compose up --build
# ... Envoy listens on :8080 (HTTP) and :8443 (HTTPS) ...
make down    # docker compose down -v
```

> Pebble stands in for Let's Encrypt so you can exercise the full issuance flow
> without hitting real rate limits.

## Configuration

Configuration is loaded from a YAML file (see
[`config/example.yaml`](config/example.yaml)) and can be overridden with
environment variables using the `ENVOY_ACME__SECTION__FIELD` convention:

```bash
ENVOY_ACME__ACME__DIRECTORY_URL=https://acme-v02.api.letsencrypt.org/directory
ENVOY_ACME__ACME__DOMAINS=example.com
ENVOY_ACME__SDS__RESOURCE_NAME=acme_cert
```

| Section     | Key                   | Default            | Description                              |
| ----------- | --------------------- | ------------------ | ---------------------------------------- |
| `acme`      | `directory_url`       | —                  | ACME directory URL.                      |
| `acme`      | `contact`             | —                  | Contact URI, e.g. `mailto:a@example.com`.|
| `acme`      | `domains`             | —                  | Domains for the certificate.             |
| `acme`      | `renewal_window_days` | `30`               | Renew when within N days of expiry.      |
| `acme`      | `state_dir`           | —                  | Persistent state directory.              |
| `ext_proc`  | `listen`              | `0.0.0.0:9000`     | ext_proc gRPC listen address.            |
| `sds`       | `listen`              | `0.0.0.0:9001`     | SDS gRPC listen address.                 |
| `sds`       | `resource_name`       | `acme_cert`        | SDS secret name Envoy subscribes to.     |
| `log`       | `format`              | `json`             | `json` or `pretty`.                      |
| `log`       | `level`               | `info`             | Log level (also honours `RUST_LOG`).     |

## Project layout

```
src/
  main.rs            entry point: CLI, logging, task wiring, signal handling
  config.rs          figment-based config (YAML + env)
  errors.rs          thiserror error types
  challenge_store.rs HTTP-01 token → key-authorization store
  cert_store.rs      current cert bundle with a watch channel for SDS
  ext_proc.rs        Envoy ext_proc server (answers ACME challenges)
  sds.rs             Envoy SDS server (pushes the cert to Envoy)
  acme.rs            ACME order/renewal state machine (instant-acme)
config/              example config + Pebble config
envoy/               example Envoy bootstrap
proto/               proto-handling notes (we use envoy-proto from phlax/envoy-proto-rs)
```

## Development

See [CONTRIBUTING.md](CONTRIBUTING.md). In short:

```bash
make check   # fmt-check + clippy (-D warnings) + tests
```

## License

Licensed under the [Apache License, Version 2.0](LICENSE).
