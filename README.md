# envoy-acme

Envoy dynamic module that issues and renews TLS certificates via ACME (HTTP-01),
publishes them to Envoy via filesystem SDS, and serves HTTP-01 challenges in-process.

Built against the Envoy `v1.38.0` SDK.

## Scope

- HTTP-01 challenges only.
- Single-instance operation; no leader election.
- `FilesystemSink` only (writes cert/key/SDS-secret files to a configured directory).
- x86_64 Linux.
- ABI pinned to Envoy `v1.38.0` SDK.

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

## Quick start

### Build

```bash
cargo build --release --target=x86_64-unknown-linux-gnu
```

### Run the local stack

```bash
make up
make logs
make down
```

### Minimal config

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

See `config/example.yaml` and `envoy/bootstrap.yaml` for complete examples.

## Configuration

See [docs/operator/configuration.md](docs/operator/configuration.md) for the full reference. Common fields:

- `directory_profile`: `staging`, `production`, or `custom` (required when not setting `directory_uri` directly).
- `contact`: operator email as a `mailto:` URI, sent during ACME account registration.
- `domains`: list of hostnames to issue certificates for (IDN/Unicode accepted).
- `state_dir`: directory for ACME account key and renewal state. Recommend mode `0700`.
- `cert_sink.cert_dir`: directory where cert/key/SDS-secret files are written.

## Operating envoy-acme

- [Configuration reference](docs/operator/configuration.md)
- [Deployment](docs/operator/deployment.md) — Docker image, UID handling
- [Security](docs/operator/security.md) — `state_dir` permissions, env-var overrides
- [Metrics](docs/operator/metrics.md) — Prometheus stats, recommended alerts
- [Release verification](docs/operator/release-verification.md) — cosign, SLSA attestations

## Changes

See [CHANGELOG.md](CHANGELOG.md) for version-by-version changes.

## Contributing

See [CONTRIBUTING.md](CONTRIBUTING.md).

## License

[LICENSE](LICENSE)

