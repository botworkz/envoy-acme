# Release Artifact Verification

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

## Verify the `.so` with cosign

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

## Verify SLSA provenance with the GitHub CLI

```bash
gh attestation verify \
  "libenvoy_acme-${VERSION}-x86_64-unknown-linux-gnu.so" \
  --repo botworkz/envoy-acme
```
