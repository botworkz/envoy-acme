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
- `envoy/bootstrap.yaml` — production-shaped bootstrap baked into the Docker image
- `envoy/bootstrap.test.yaml` — integration-test variant used by `compose.yaml` (mounts over the baked-in file at runtime)

Config bytes are parsed as JSON first, then YAML.

Notable config options in `acme:`:

- `directory_profile` (optional): selects a known ACME directory by name.
  When set, `directory_uri` is resolved automatically and need not be
  specified (though it may be supplied for documentation purposes, in which
  case it must exactly match the resolved URL or the config will be rejected).

  | Value | Directory URL used |
  |---|---|
  | `staging` | `https://acme-staging-v02.api.letsencrypt.org/directory` |
  | `production` | `https://acme-v02.api.letsencrypt.org/directory` |
  | `custom` | value of `directory_uri` (required) |

  Example — staging (no `directory_uri` needed):
  ```yaml
  acme:
    directory_profile: staging
    contact: mailto:admin@example.com
    domains: [example.com]
    state_dir: /var/lib/envoy-acme
    cert_sink:
      type: filesystem
      cert_dir: /var/lib/envoy-acme/certs
  ```

  Example — custom (e.g. Pebble in integration tests):
  ```yaml
  acme:
    directory_profile: custom
    directory_uri: https://pebble:14000/dir
    contact: mailto:admin@example.test
    domains: [example.test]
    state_dir: /var/lib/envoy-acme
    cert_sink:
      type: filesystem
      cert_dir: /var/lib/envoy-acme/certs
  ```

  When `directory_profile` is not set, `directory_uri` is required and used
  verbatim (equivalent to `custom`).

- `directory_uri` (optional when a `staging` or `production` profile is set,
  required otherwise): the ACME directory URL.  ACME directory traffic uses
  the embedded HTTPS client (via `instant-acme`), not an Envoy cluster.
  Cluster-routed ACME is not yet supported.

- `directory_ca_file` (optional): path to a PEM CA bundle to trust when connecting to the ACME directory (e.g. Pebble's self-signed CA in integration tests). Omit to use the system native roots.
- `tick_seconds` (default `60`): how often the renewal state machine timer fires. Set lower in integration environments.

## Docker image

The published image runs as the `envoy` user (uid `101` in the upstream `envoyproxy/envoy` base image).

To override the uid at build time (e.g. when rebasing on a base image where the `envoy` user has a different uid):

```bash
docker build --build-arg ENVOY_UID=1001 .
```

The `ENVOY_UID` build argument is provided as an override surface; the `chown` step uses the named `envoy` user directly.

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

## Release artifact verification

Every tagged release publishes the following files alongside the `.so`:

| File | Purpose |
|---|---|
| `libenvoy_acme-<ver>-x86_64-unknown-linux-gnu.so` | The dynamic module |
| `libenvoy_acme-<ver>-x86_64-unknown-linux-gnu.so.sha256` | SHA-256 checksum |
| `libenvoy_acme-<ver>-x86_64-unknown-linux-gnu.so.sig` | Sigstore / cosign signature |
| `libenvoy_acme-<ver>-x86_64-unknown-linux-gnu.so.pem` | Signing certificate |
| `envoy_acme-<ver>.sbom.json` | CycloneDX SBOM |
| `envoy_acme-<ver>.sbom.json.sig` | SBOM Sigstore signature |
| `envoy_acme-<ver>.sbom.json.pem` | SBOM signing certificate |

A Sigstore-attested SLSA v1 provenance document is also published to the GitHub
transparency log and can be inspected with `gh attestation verify`.

### Verify the `.so` with cosign

```bash
VERSION=0.3.0   # substitute the actual release version

cosign verify-blob \
  --certificate-identity-regexp \
    "https://github.com/botworkz/envoy-acme/.github/workflows/ci.yaml@refs/heads/main" \
  --certificate-oidc-issuer https://token.actions.githubusercontent.com \
  --signature "libenvoy_acme-${VERSION}-x86_64-unknown-linux-gnu.so.sig" \
  --certificate "libenvoy_acme-${VERSION}-x86_64-unknown-linux-gnu.so.pem" \
  "libenvoy_acme-${VERSION}-x86_64-unknown-linux-gnu.so"
```

### Verify SLSA provenance with the GitHub CLI

```bash
gh attestation verify \
  "libenvoy_acme-${VERSION}-x86_64-unknown-linux-gnu.so" \
  --repo botworkz/envoy-acme
```

## Known limitations

- HTTP-01 only.
- Single instance operation, no leader election.
- `FilesystemSink` only.
- ABI pinned to Envoy `v1.38.0` SDK.
- Panic in dynamic module can still affect proxy process; callbacks are guarded by SDK `catch_unwind` wrappers.
