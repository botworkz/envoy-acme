# Configuration Reference

Config bytes are parsed as JSON first, then YAML.

For a working example, see `config/example.yaml` and `envoy/bootstrap.yaml`.

See [CHANGELOG.md](../../CHANGELOG.md) for version-by-version changes, including breaking changes.

## `acme:`

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

- `domains`: list of hostnames for which TLS certificates should be issued. Accepts internationalised domain names (IDNs) in either Unicode (U-label, e.g. `münchen.example`) or Punycode (A-label, e.g. `xn--mnchen-3ya.example`) form. Inputs are normalised to A-label form per RFC 5890 / UTS#46 nontransitional, matching the CA/Browser Forum Baseline Requirements profile used by Let's Encrypt and other modern CAs. The normalised A-label is what appears in metrics, logs, and on-disk filenames (e.g. `xn--mnchen-3ya.example.cert.pem`).

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
a single file in `cert_dir`.  The **first domain** in `acme.domains` is used as the
canonical filename prefix:

| File | Description |
|---|---|
| `<first-domain>.secret.yaml` | Envoy SDS TLS-certificate secret with cert chain and private key embedded as `inline_string` |

The cert chain and private key are embedded directly in the YAML using Envoy's
`inline_string` data source.  This means each renewal is a **single atomic file
rename**, so Envoy's SDS directory watcher fires exactly once and always sees a
consistent cert+key pair.  The file is written with mode `0o600` because it
contains the private key.

For **multi-SAN certs** (`acme.domains` contains more than one name), the
issued certificate covers _all_ configured domains as Subject Alternative
Names (SANs), but the output file is still named after the **first domain**
in the list.  For example, with `domains: [a.example.com, b.example.com]`,
the file written is `a.example.com.secret.yaml` — even though the cert inside
covers both names.

The SDS `path_config_source` in the Envoy listener must point at
`<first-domain>.secret.yaml`.  SNI-based selection then works automatically:
Envoy reads the single cert file, which contains both SANs, and presents it
to clients regardless of which of the configured names they connect to.

> **Note:** if you need an explicit cert name independent of the domain list
> order, a `cert_name` config field may be added in a future release.  For
> now, reorder `domains` so the desired name comes first.
