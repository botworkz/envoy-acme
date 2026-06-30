# Security

## `state_dir` permissions

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

```bash
export ENVOY_ACME_ALLOW_INSECURE_STATE_DIR=1
```

This check is Unix-only.  On non-Unix targets it is a compile-time no-op.

## Filesystem placement of `state_dir` and `cert_sink.cert_dir`

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

## `cert_dir` file permissions

`FilesystemSink` writes `<first-domain>.secret.yaml` with mode `0o600` because
the file embeds the private key as an `inline_string`.  Only the process user
(typically `envoy`, uid `101`) can read it.

If you need to inspect the embedded certificate manually, extract it with:

```bash
awk '/-----BEGIN CERTIFICATE-----/,/-----END CERTIFICATE-----/' \
  /var/lib/envoy-acme/certs/a.example.test.secret.yaml \
  | openssl x509 -noout -issuer -subject -ext subjectAltName
```
