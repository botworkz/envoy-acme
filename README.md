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

> **Breaking change (v0.3):** Unknown keys in `acme:`, `cert_sink:`, `log:`, and
> the HTTP filter `filter_config` are now **rejected at startup** with an error
> that names the offending field.  Any config containing stale or mistyped keys
> (e.g. `directory_cluster:`, `celr_dir:`) will fail to load.  This is
> intentional — silent key-dropping hides typos and stale fields from operators.
> Update your configs to remove any unrecognised keys before upgrading to v0.3.

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
  required otherwise): the ACME directory URL.  Must use `https://`.  ACME
  directory traffic uses the embedded HTTPS client (via `instant-acme`), not
  an Envoy cluster.  Cluster-routed ACME is not yet supported.

- `allow_insecure_directory` (optional, default `false`): when `true`, permits
  a plain-`http://` `directory_uri` for the `custom` profile.  **Security
  warning:** nonces and credentials will traverse the network in cleartext.
  Only intended for local integration-test environments (e.g. Pebble without
  TLS).  This flag has no effect on `staging` or `production` profiles, which
  always require `https://`.

  Example — Pebble integration test with plain HTTP:
  ```yaml
  acme:
    directory_profile: custom
    directory_uri: http://pebble:14000/dir
    allow_insecure_directory: true   # TEST ONLY – plain HTTP accepted
    directory_ca_file: /etc/pebble/ca.pem
    contact: mailto:admin@example.test
    domains: [example.test]
    state_dir: /var/lib/envoy-acme
    cert_sink:
      type: filesystem
      cert_dir: /var/lib/envoy-acme/certs
  ```

- `directory_ca_file` (optional): path to a PEM CA bundle to trust when connecting to the ACME directory (e.g. Pebble's self-signed CA in integration tests). Omit to use the system native roots.
- `contact`: the operator contact address sent during ACME account registration.
  Must be a `mailto:` URI per [RFC 8555 §7.3](https://www.rfc-editor.org/rfc/rfc8555#section-7.3),
  e.g. `mailto:admin@example.test`. Other schemes are not accepted by any production CA and are
  rejected at config load.
- `tick_seconds` (default `60`): how often the renewal state machine timer fires. Set lower in integration environments.
- `acme.domains`: list of hostnames for which TLS certificates should be issued. Accepts internationalised domain names (IDNs) in either Unicode (U-label, e.g. `münchen.example`) or Punycode (A-label, e.g. `xn--mnchen-3ya.example`) form. Inputs are normalised to A-label form per RFC 5890 / UTS#46 nontransitional, matching the CA/Browser Forum Baseline Requirements profile used by Let's Encrypt and other modern CAs. The normalised A-label is what appears in metrics, logs, and on-disk filenames (e.g. `xn--mnchen-3ya.example.cert.pem`).

## HTTP filter config (`filter_config`)

The `acme_http` HTTP filter requires a separate `filter_config` entry in the
Envoy listener that lists the domains the filter should respond to.  The HTTP
filter only intercepts `GET /.well-known/acme-challenge/<token>` requests whose
`Host` / `:authority` header matches one of the configured domains (comparison
is case-insensitive and port-stripped).  Requests from any other virtual host
fall through to the rest of the filter chain unchanged.

> **Note:** the Envoy listener that carries this filter must be reachable from
> the internet (or the ACME CA's validation network) on the same domain names
> listed here.  HTTP-01 validation works by having the CA send an HTTP request
> to `http://<domain>/.well-known/acme-challenge/<token>`, so the listener
> **must** be reachable on port 80 for at least the configured domains.  This
> is implicit in how HTTP-01 works, but worth spelling out: if the listener is
> only reachable on a private network or on a domain not listed in `domains`,
> certificate issuance will silently fail at the CA validation step.

Example (matching `envoy/bootstrap.yaml`):

```yaml
filter_config:
  "@type": type.googleapis.com/google.protobuf.StringValue
  value: |
    domains:
      - example.com
```

The `domains` list must contain the same values as `acme.domains` in the
bootstrap extension config.  Domain matching is **case-insensitive** and
**port-stripped**, so `EXAMPLE.COM`, `example.com`, and `example.com:80` all
match a configured domain of `example.com`.  Any casing is accepted in the
config.  Requests whose `Host` / `:authority` header does not match any
configured domain are passed through to the next filter unchanged, so the HTTP
filter is safe to place in front of other virtual hosts on a shared listener.

## Cert sink (`cert_sink`)

`FilesystemSink` (the only sink type today) writes the certificate bundle to
files in `cert_dir`.  The **first domain** in `acme.domains` is used as the
canonical filename prefix for all output files:

| File | Description |
|---|---|
| `<first-domain>.cert.pem` | PEM certificate chain (leaf + intermediates) |
| `<first-domain>.key.pem` | PEM private key |
| `<first-domain>.secret.yaml` | Envoy SDS TLS-certificate secret |

For **multi-SAN certs** (`acme.domains` contains more than one name), the
issued certificate covers _all_ configured domains as Subject Alternative
Names (SANs), but the output files are still named after the **first domain**
in the list.  For example, with `domains: [a.example.com, b.example.com]`,
the files written are `a.example.com.cert.pem`, `a.example.com.key.pem`, and
`a.example.com.secret.yaml` — even though the cert inside covers both names.

The SDS `path_config_source` in the Envoy listener must point at
`<first-domain>.secret.yaml`.  SNI-based selection then works automatically:
Envoy reads the single cert file, which contains both SANs, and presents it
to clients regardless of which of the configured names they connect to.

> **Note:** if you need an explicit cert name independent of the domain list
> order, a `cert_name` config field may be added in a future release.  For
> now, reorder `domains` so the desired name comes first.


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
2. The module contacts Pebble and completes HTTP-01 challenge validation for **both** configured domains via the in-process HTTP filter.
3. `FilesystemSink` writes `a.example.test.cert.pem`, `a.example.test.key.pem`, and the Envoy SDS secret file `a.example.test.secret.yaml` (first domain as canonical filename prefix).
4. The issued cert contains SANs for **both** `a.example.test` and `b.example.test`.
5. Envoy's HTTPS listener warms up using the SDS secret file and serves traffic.
6. `curl --cacert pebble.minica.pem https://a.example.test:8443/` and `https://b.example.test:8443/` both return HTTP 200.
7. The certificate presented on each SNI name has SANs `[a.example.test, b.example.test]` and chains to Pebble's CA.

`challtestsrv` acts as a programmable DNS server: the CI step registers both `a.example.test` and `b.example.test` to the envoy container's IP via its management API on `:8055` before triggering issuance, so Pebble can perform real HTTP-01 validation for each name against Envoy.

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
    "https://github.com/botworkz/envoy-acme/.github/workflows/ci.yaml@refs/(heads/main|tags/v.*)" \
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

# Security

### `state_dir` permissions

envoy-acme expects `state_dir` to be mode `0700` (owner-only).  The directory
holds `account.json` (the ACME account EC private key), `backoff.json`, and
`bundle.ok`.  A world- or group-readable parent allows other local users to read
`account.json` and observe operational state.

At startup, if `state_dir` has any `g+r`, `g+w`, `g+x`, `o+r`, `o+w`, or
`o+x` bits set, envoy-acme logs a single `WARN` message naming the offending
mode bits and the recommended remediation:

```
WARN envoy-acme: state_dir is group- or world-accessible; ...
     Recommended: chmod 0700 /var/lib/envoy-acme.
```

To fix, run:

```bash
chmod 0700 /var/lib/envoy-acme   # substitute your state_dir path
```

If looser permissions are intentional (e.g. a monitoring agent reads state
files), set the environment variable `ENVOY_ACME_ALLOW_INSECURE_STATE_DIR=1`
to suppress the warning:

```yaml
# envoy bootstrap — static_resources or layered runtime
admin:
  ...
```

```bash
export ENVOY_ACME_ALLOW_INSECURE_STATE_DIR=1
```

This check is Unix-only.  On non-Unix targets it is a compile-time no-op.

### Filesystem placement of `state_dir` and `cert_sink.cert_dir`

Place both directories on the same filesystem when possible.  Atomic file
writes within each directory are correct regardless (the temp file is always
created in the same directory as the destination), but cross-filesystem
configurations surface `EXDEV` errors as `Permanent`-class issuance failures
if any code path renames between them.

At startup, if `state_dir` and `cert_dir` resolve to different filesystem
device numbers, envoy-acme logs a single `WARN` message:

```
WARN envoy-acme: state_dir and cert_dir are on different filesystems. ...
     Set ENVOY_ACME_ALLOW_CROSS_FS_DIRS=1 to suppress this warning.
```

If the configuration is intentional (e.g. local SSD for `state_dir`, shared
NFS/iSCSI volume for `cert_dir` that Envoy reads via SDS), set the environment
variable `ENVOY_ACME_ALLOW_CROSS_FS_DIRS=1` to suppress the warning:

```bash
export ENVOY_ACME_ALLOW_CROSS_FS_DIRS=1
```

This check is Unix-only.  On non-Unix targets it is a compile-time no-op.

## Metrics

The module exposes the following Prometheus metrics via Envoy's built-in stats sink.

| Metric | Type | Description |
|--------|------|-------------|
| `envoy_acme_issuance_total{result}` | Counter | Total issuance attempts, labelled by result (`success`, `failure`, `permanent`, `recovery_required`). |
| `envoy_acme_consecutive_failures{domain}` | Gauge | Number of consecutive issuance failures for a domain since the last success. |
| `envoy_acme_next_retry_at_seconds{domain}` | Gauge | Unix timestamp of the earliest permitted next retry (0 when not in backoff). |
| `envoy_acme_cert_not_after_seconds{domain}` | Gauge | Unix timestamp of the current certificate's `notAfter` field. |
| `envoy_acme_account_state{domain}` | Gauge | Current account health: `0` = healthy, `1` = recovery required (operator must delete `account.json` and re-register). |
| `envoy_acme_issuance_duration_seconds` | Histogram | Duration of the last issuance attempt in seconds (ceiling-rounded). |

### Recommended alert

```promql
# Page when an ACME account has been stuck in recovery-required state for more than 5 minutes.
envoy_acme_account_state == 1
```

## Known limitations

- HTTP-01 only.
- Single instance operation, no leader election.
- `FilesystemSink` only.
- ABI pinned to Envoy `v1.38.0` SDK.
- Panic in dynamic module can still affect proxy process; callbacks are guarded by SDK `catch_unwind` wrappers.
