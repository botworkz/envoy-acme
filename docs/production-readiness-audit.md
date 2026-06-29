# Production-Readiness Audit

> Audit of `botworkz/envoy-acme` at `main`.
>
> This is a pre-production ACME dynamic-module for Envoy: it performs
> certificate issuance in-proxy and publishes material via `FilesystemSink` for
> Envoy's filesystem SDS. It has passed integration tests against Pebble but has
> not been run against real Let's Encrypt in production.
>
> Each section identifies the specific code paths involved, gaps relative to
> production operation, and the rough shape of the fix.

---

## Table of contents

1. [Durability of on-disk state](#1-durability-of-on-disk-state)
2. [Correctness of the renewal state machine](#2-correctness-of-the-renewal-state-machine)
3. [ACME protocol robustness](#3-acme-protocol-robustness)
4. [Operability and observability](#4-operability-and-observability)
5. [Security posture](#5-security-posture)
6. [Supply chain and build](#6-supply-chain-and-build)
7. [Reliability of the Envoy integration](#7-reliability-of-the-envoy-integration)
8. [Configuration safety](#8-configuration-safety)
9. [Test coverage gaps](#9-test-coverage-gaps)
10. [Documentation gaps](#10-documentation-gaps)
11. [Milestone plan](#milestone-plan)

---

## 1. Durability of on-disk state

### What the code does today

| File | Written by | How |
|---|---|---|
| `state_dir/cert.pem` | `AcmeStateMachine::persist_bundle` (`src/acme/mod.rs:217-222`) | `tokio::fs::write` — no temp+rename, no fsync |
| `state_dir/key.pem` | same | same |
| `state_dir/account.json` | `account::load_or_create_account` (`src/acme/account.rs:162-164`) | `tokio::fs::write` — no temp+rename, no fsync |
| `state_dir/backoff.json` | `persist_backoff` (`src/acme/mod.rs:255-259`) | `tokio::fs::write` — no temp+rename, no fsync |
| `cert_dir/<domain>.cert.pem` | `FilesystemSink::write_atomic` (`src/cert_sink/filesystem.rs:75-97`) | `NamedTempFile` → `tmp.persist(path)` + `sync_all` on temp + parent dir |
| `cert_dir/<domain>.key.pem` | same | same; explicit `chmod 0600` on Unix (`src/cert_sink/filesystem.rs:86-91`) |
| `cert_dir/<domain>.secret.yaml` | `FilesystemSink::write_sds_secret` (`src/cert_sink/filesystem.rs:105-136`) | atomic write, written last |

**`FilesystemSink`** (the Envoy-facing output) is correctly atomic and durable.
**All `state_dir` files** (the renewal-side bookkeeping) are *not* atomic: a process
crash between the `cert.pem` and `key.pem` writes leaves a mismatched pair on
disk, and there is no fsync.

File permissions for `account.json`, `cert.pem`, `key.pem`, and `backoff.json` in
`state_dir` are **whatever the process umask allows**; there is no explicit
`set_permissions` call for these files.

There is no inter-process locking. Two processes sharing a `state_dir` will race.

### What's missing for production

| Gap | Severity |
|---|---|
| `state_dir/cert.pem` + `key.pem` written non-atomically; crash between writes leaves mismatched pair | **will-cause-outage** |
| `account.json` / `backoff.json` can be torn on crash due to non-atomic write | **will-lose-data** |
| Private key (`state_dir/key.pem`) and `account.json` rely on umask, not explicit `0600` | **will-cry-at-3am** |
| No advisory lock on `state_dir`; multi-process race | **will-cause-outage** (multi-instance) |

### Suggested fix shape

- Introduce one shared atomic-write helper (tempfile in same directory → write → `sync_all` → rename → `sync_all` parent dir) and use it for *every* state file.
- Set explicit `0600` on `account.json` and `state_dir/key.pem`.
- Commit the cert/key pair atomically: write both to versioned temp paths, then rename in sequence, or use a sentinel marker.
- Add an advisory lock file (`state_dir/.lock`) using `fcntl`/`flock`.

**Effort:** 1–2 days.

---

## 2. Correctness of the renewal state machine

### What the code does today

The heart of the loop is `AcmeStateMachine::tick_at` (`src/acme/mod.rs:121-196`).

1. Creates `state_dir` if missing.
2. **Reloads `backoff.json` from disk every tick** (`src/acme/mod.rs:125`). Parse
   errors are silently collapsed to `BackoffState::default()` via `unwrap_or_default`
   (`src/acme/mod.rs:250`).
3. If blocked, logs and returns `Ok(())`.
4. Calls `load_cached_bundle` (`src/acme/mod.rs:204-215`): checks file existence,
   reads both PEM files, then calls `renewal::cert_not_after_unix` which parses
   only the *first* certificate in the chain (`src/acme/renewal.rs:55-62`). There
   is **no check that the cert is for the configured domain(s)**, no issuer check,
   no key-match.
5. If the cert is not inside the renewal window, republishes it and returns.
6. Otherwise, issues a new certificate.

The renewal window uses a deterministic FNV-1a domain offset (`src/acme/renewal.rs:27-42`).
There is no explicit clock-skew safety buffer.

If `domains` is empty, `domain` in the tick context becomes `""` (`src/acme/mod.rs:127-133`).
`needs_renewal_at_with_domain_offset` documents this and yields a zero offset
(`src/acme/renewal.rs:34-35`). The ACME order call then passes an empty identifiers
slice (`src/acme/order.rs:22-27`), which instant-acme will likely reject at the
server or be treated as an invalid order.

Wildcards and IDN domains are passed through without normalization.

### What's missing for production

| Gap | Severity |
|---|---|
| Corrupted `backoff.json` silently resets backoff state (loss of rate-limit protection) | **will-cry-at-3am** |
| Cached cert not validated against configured domain(s) — operator-injected self-signed cert accepted | **will-cause-outage** |
| No SANs/issuer/key-type check on cached bundle | **will-cause-outage** |
| `domains: []` passes empty identifier list to ACME without error at config time | **will-cause-outage** |
| No explicit clock-skew buffer near cert expiry | **will-cry-at-3am** |
| Wildcard/IDN domains not normalized or validated | **will-cause-outage** |

### Suggested fix shape

- Treat `backoff.json` parse failure as an error-logged quarantine event, not silent default.
- After loading the cached cert, validate that the SAN list covers all configured domains, the key material is self-consistent, and the cert is from the expected issuer profile.
- Add config-time validation for empty domains, wildcard structure, and IDNA canonicalization.
- Add a configurable clock-skew safety buffer (e.g. `renewal_window_days + 1 day`) to renew slightly earlier.

**Effort:** 2–3 days.

---

## 3. ACME protocol robustness

### What the code does today

**Challenge type:** HTTP-01 only. `order::issue_certificate` (`src/acme/order.rs:55-64`)
picks the first `ChallengeType::Http01` challenge; if none exists,
`AcmeError::NoChallenge` is returned. Tokens are inserted into the global
`ChallengeStore` and served by the HTTP filter on the Envoy worker thread.

**Order polling:** A fixed loop of up to 30 polls at 2-second intervals
(`src/acme/order.rs:12-13`, `67-88`). If the loop exhausts without `Ready`/`Valid`,
the order fails. Certificate bytes are accepted as returned (`src/acme/order.rs:98-116`)
without PEM/chain validation.

**Account lifecycle:** `account::load_or_create_account` (`src/acme/account.rs:125-165`)
loads existing credentials or creates a new account. There is no key-rotation path and
no explicit recovery when `account.json` is lost after production issuance (re-create
silently creates a new account).

**Error classification** (`src/acme/backoff.rs:54-88`):
- `rateLimited` / HTTP 429 → `RateLimited` (correct).
- `badNonce` → `Transient` (correct; instant-acme retries nonces internally).
- `accountDoesNotExist` → `Transient` (problematic: should trigger account recovery).
- All other `Problem` documents → `Transient`.

**Per-tick deadline:** There is no timeout wrapping the full issuance call. A stalled
HTTPS connection to the ACME directory can hold the runtime thread indefinitely.

### What's missing for production

| Gap | Severity |
|---|---|
| Empty/malformed cert chain from server accepted without validation | **will-cause-outage** |
| `accountDoesNotExist` classified as transient, not permanent/recovery-required | **will-cry-at-3am** |
| Lost `account.json` silently creates a new account (rate-limit implications) | **will-cry-at-3am** |
| No per-tick end-to-end deadline | **will-cry-at-3am** |
| No key-rotation mechanism | **papercut** |

### Suggested fix shape

- Validate returned cert chain: parse all PEM blocks, check SANs, verify chain structure before publishing.
- Expand ACME problem classification to distinguish `accountDoesNotExist`, `unauthorized`, and similar permanent/recovery states.
- Document and optionally enforce a `account.json` recovery path (re-register on explicit operator signal, not silently).
- Wrap `issuer.issue()` in a bounded `tokio::time::timeout`.

**Effort:** 2–4 days.

---

## 4. Operability and observability

### What the code does today

`tracing` is used for key events:

- Rate-limit backoff block detected (`src/acme/mod.rs:137-140`): `debug!` with `domain`, `next_retry_at` fields.
- Rate-limited by ACME server (`src/acme/mod.rs:184-189`): `info!` with `domain`, `next_retry_at`, `consecutive_failures`.
- Certificate published (`src/acme/mod.rs:233`): `info!` with `domain`, `marker`.
- `tick()` failure (`src/runtime.rs:61-63`): Envoy log error macro.
- Runtime thread panic (`src/runtime.rs:75-79`): Envoy log error macro.

There are **no counters, gauges, or histograms**. There is no HTTP introspection
endpoint beyond Envoy's own admin API. There is no structured "last successful
renewal" state exposed at runtime.

The `AcmeBootstrapConfig` and `AcmeBootstrapExtension` log nothing on construction
(`src/bootstrap.rs:22-38`, `src/bootstrap.rs:85-92`); a fresh start and a restart
with existing state are observationally identical at startup.

### What's missing for production

| Gap | Severity |
|---|---|
| No metrics — cannot alert on `consecutive_failures > N` | **will-cry-at-3am** |
| No runtime introspection (next tick, current backoff, last success, cert expiry) | **will-cry-at-3am** |
| Startup does not distinguish fresh vs. cached-state restart | **papercut** |
| No way to know renewal engine died (runtime thread panic → silent cert expiry) | **will-cause-outage** |

### Suggested fix shape

- Expose structured metrics: issuance success/failure counters, `consecutive_failures` gauge, `next_retry_at` gauge, cert `not_after` gauge, issuance duration histogram.
- Emit structured startup log with cert state (no cert, cert valid until X, renewal due in Y days).
- Add a periodic status heartbeat log (or admin endpoint) for `last_renewal_at`, `next_attempt_at`, `cert_expiry`.
- Expose runtime-thread liveness (watchdog ticker, or emit a heartbeat metric).

**Effort:** 2–3 days.

---

## 5. Security posture

### What the code does today

**Directory URL:** Accepted from config verbatim (`src/config.rs:97-107`). No scheme
validation is enforced at config-load time. `directory_ca_file` (`src/config.rs:43`)
pins a custom CA and is used to build a custom `rustls` client
(`src/acme/account.rs:98-123`). An empty PEM file is accepted without error
(`src/acme/account.rs:178-187`, test assertion `assert!(build_custom_client(&ca_path).is_ok())`).

**HTTP-01 challenge responder** (`src/http_filter.rs`): matches only `:path` starting
with `/.well-known/acme-challenge/` and a token of up to 256 characters that contains
no `/`. There is no check on the `:authority`/`Host` header.

**Challenge store** (`src/challenge_store.rs`): a global `Arc<RwLock<HashMap>>` with
no cap, no TTL, and no eviction policy. Token cleanup is best-effort (two `remove`
paths in `order.rs`). Leaked tokens (e.g. from a crashed issuance mid-flight) remain
until `clear_challenges()` is called at drain time (`src/runtime.rs:67`).

**Key exposure:** No logging of key material or account credentials was found. The
`account::load_or_create_account` function writes credentials to disk without logging
the JSON content.

**`state_dir` threat model:** An attacker with write access to `state_dir` can replace
`cert.pem` / `key.pem` with their own material, set `backoff.json` to block renewal
indefinitely, or corrupt `account.json` to force a new account creation.

### What's missing for production

| Gap | Severity |
|---|---|
| No HTTPS scheme enforcement on `directory_uri` for non-test profiles | **security risk** |
| Host-agnostic challenge responses: any vhost could serve tokens | **will-cry-at-3am** |
| `state_dir` has no integrity model; write access = cert-rotation attack surface | **will-cause-outage** |
| Leaked challenge tokens accumulate until drain | **papercut** |

### Suggested fix shape

- Validate that `directory_uri` starts with `https://` for `staging`/`production` profiles; gate insecure URLs to `custom` only with explicit operator acknowledgment.
- Add host-binding to the challenge responder (validate `Host`/`:authority` against configured domains).
- Document `state_dir` ownership and permission expectations; enforce at startup.
- Add TTL-based eviction for challenge-store entries.

**Effort:** 2–3 days.

---

## 6. Supply chain and build

### What the code does today

**`Cargo.toml`** (`Cargo.toml:13-38`):
- All registry dependencies are semver-ranged (no `*`; no git-pinned non-SDK deps).
- One git dependency for the Envoy SDK is pinned to an exact commit SHA (`rev = "f1dd21b16c244bda00edfb5ffce577e12d0d2ec2"`).

**`Cargo.lock`:** Committed at repo root.

**CI** (`.github/workflows/ci.yaml`):
- Runs on `push` to `main` and `pull_request`.
- Jobs: `fmt`, `clippy` (`-D warnings`), `build`, `unit`, `integration`, `release`.
- Uses `Swatinem/rust-cache@v2` for caching.
- No `cargo audit` or `cargo deny` step.

**Dockerfile** (`Dockerfile:1-52`): Multi-stage build (Rust builder + `envoyproxy/envoy:v1.38-latest`). Builds `x86_64-unknown-linux-gnu` only. No signing step, no SBOM generation.

**Release flow** (`ci.yaml:155-225`): On push to `main`, reads `VERSION`, creates a GitHub release (if not `*-dev` and not already tagged), attaches `.so` + `.sha256`. No Sigstore/cosign signing, no SLSA provenance.

### What's missing for production

| Gap | Severity |
|---|---|
| No `cargo audit` / `cargo deny` in CI — vulnerabilities in dependencies go undetected | **will-cry-at-3am** |
| Release artifacts unsigned and unattested (no SLSA provenance, no SBOM) | **will-cry-at-3am** |
| Single-architecture release (x86_64 only) | **papercut** |

### Suggested fix shape

- Add `cargo deny check` with an `allow.toml` policy; or `cargo audit` as a CI gate on every push.
- Add Sigstore/cosign image signing and SLSA provenance generation to the release workflow.
- Decide and document supported architecture matrix; extend release if needed.

**Effort:** 1–2 days.

---

## 7. Reliability of the Envoy integration

### What the code does today

**Timer and jitter** (`src/bootstrap.rs:58-63`): After each tick, `on_timer_fired`
computes `jittered = tick_seconds * rand([0.9, 1.1])` and re-enables the timer.
If `tick_seconds` is `0`, the cast `(0f64 * jitter) as u64` yields `0`, and
`timer.enable(Duration::from_secs(0))` re-arms immediately — a tight loop.

**Concurrency:** Commands (`Start`, `Tick`, `Shutdown`) are sent over an unbounded
`mpsc` channel to a single runtime thread (`src/runtime.rs:28-30`). The state machine
processes them serially (`src/runtime.rs:57-71`). If an issuance takes longer than
`tick_seconds`, the next `Tick` queues up without bound.

**Panic recovery** (`src/runtime.rs:48-80`): `std::panic::catch_unwind` wraps
`block_on`. A panic is caught, logged via `envoy_log_error!`, and the thread exits
cleanly. The channel sender in `RuntimeBridge` remains alive; subsequent `tick()` calls
return `RuntimeError::Stopped`. The renewal engine is dead but Envoy continues
serving — silently.

**Challenge store contention:** Worker threads call `challenge_store::lookup`
(read lock) while the runtime thread calls `insert`/`remove` (write lock).
`parking_lot::RwLock` is fair; no unbounded starvation. No contention instrumentation.

### What's missing for production

| Gap | Severity |
|---|---|
| `tick_seconds: 0` causes a busy loop consuming CPU | **will-cause-outage** |
| Unbounded tick queue if issuance takes longer than `tick_seconds` | **will-cry-at-3am** |
| Runtime thread death is silent after initial log — cert expiry not detected | **will-cause-outage** |

### Suggested fix shape

- Enforce `tick_seconds >= 1` in config validation (or at bootstrap, clamp with a warning).
- Add a "tick in flight" flag: drop or coalesce incoming `Tick` commands while one is running.
- Add a watchdog mechanism: if the runtime thread exits unexpectedly, escalate to an Envoy fatal log or an observable error state.

**Effort:** 1–2 days.

---

## 8. Configuration safety

### What the code does today

**`TryFrom<RawAcmeConfig>`** (`src/config.rs:56-109`) validates the
`directory_profile` / `directory_uri` matrix well: rejects missing URI on `custom`,
rejects URI mismatches on `staging`/`production`.

Everything else is **not validated at load time**:
- `tick_seconds: 0` is accepted.
- `domains: []` is accepted.
- `contact: ""` / non-`mailto:` strings are accepted.
- `renewal_window_days: 0` is accepted.
- `state_dir` / `cert_dir` existence or writability are not checked.
- `cert_sink.type` is stored but the runtime always constructs `FilesystemSink` regardless of its value (`src/runtime.rs:50-53`).

Unknown config keys are silently ignored (serde default; no `deny_unknown_fields`).
The test bootstrap (`envoy/bootstrap.test.yaml:16`) still contains a stale
`directory_cluster` key that is silently ignored.

There is no config schema version field.

### What's missing for production

| Gap | Severity |
|---|---|
| `tick_seconds: 0` accepted — causes busy loop | **will-cause-outage** |
| `domains: []` accepted — causes broken ACME order at runtime | **will-cause-outage** |
| Unwritable `state_dir`/`cert_dir` discovered only at first tick, not at startup | **will-cry-at-3am** |
| `cert_sink.type` not enforced; silently ignored | **papercut** |
| No schema versioning; stale keys silently ignored | **papercut** |

### Suggested fix shape

- Add a `validate()` method on `AcmeConfig` checking: `tick_seconds >= 1`, `!domains.is_empty()`, each domain non-empty, `contact` is non-empty (optionally `mailto:` prefix), `renewal_window_days >= 1`.
- Probe `state_dir` and `cert_sink.cert_dir` for existence and writability at bootstrap.
- Enforce `cert_sink.type == "filesystem"` or prepare for future sink types.
- Consider adding `deny_unknown_fields` or a config schema version field with a migration guard.

**Effort:** 1–2 days.

---

## 9. Test coverage gaps

### What the code does today

**Unit tests** exist in every module:

| Module | Tests |
|---|---|
| `src/acme/mod.rs` | 8 tests: renewal window, rate-limit backoff lifecycle, jitter spread |
| `src/acme/backoff.rs` | 4 tests: classify errors, backoff arithmetic |
| `src/acme/account.rs` | 3 tests: custom client PEM parsing, filter_tokenless_challenges |
| `src/acme/renewal.rs` | 3 tests: window logic, determinism, spread |
| `src/config.rs` | 10 tests: profile/URI matrix |
| `src/cert_sink/filesystem.rs` | 3 tests: layout, SDS YAML, resource-name derivation |
| `src/http_filter.rs` | 2 tests: pass-through, 200 hit |
| `src/challenge_store.rs` | 1 test: insert/lookup/remove |

**Integration test** (`.github/workflows/ci.yaml:50-153`): Runs a full Pebble+Envoy stack, verifies issuance, SAN, chain, and HTTPS end-to-end. This is a good smoke test.

### What's missing for production

| Gap | Severity |
|---|---|
| No failure-injection tests (crash mid-write, torn state file, ACME 5xx, network drop) | **will-cry-at-3am** |
| No multi-domain or wildcard config test | **will-cause-outage** |
| No test for corrupt `cert.pem` / `key.pem` in cache (parse failure path) | **will-cry-at-3am** |
| No test for SDS file rotation under concurrent Envoy reload | **will-cry-at-3am** |
| No soak / longevity test (run for hours, verify renewal fires) | **will-cry-at-3am** |
| Integration polling (120 s timeout) is timing-sensitive; can be flaky under CI load | **papercut** |

### Suggested fix shape

- Add fault-injection unit tests: inject `Err` from a mock filesystem at controlled points, assert correct state transitions.
- Add tests for `load_cached_bundle` with a corrupt / truncated PEM.
- Add integration scenario for multi-domain config (two domains, one cert).
- Add a nightly or manual-trigger soak test (loop ticks with accelerated clock for simulated weeks).

**Effort:** 3–5 days.

---

## 10. Documentation gaps

### What the code does today

- `README.md` covers: prototype warning, architecture diagram, build, run, config reference, integration test topology, known limitations.
- `CONTRIBUTING.md` covers: local check commands, stack invocation.
- No operations runbook, no threat model document, no migration guide.

Known limitations listed in `README.md:149-156` are:
- HTTP-01 only
- Single-instance only
- `FilesystemSink` only
- ABI pinned to Envoy v1.38.0 SDK

The limitations do **not** mention the durability, observability, or configuration-safety gaps identified in this audit.

### What's missing for production

| Gap | Severity |
|---|---|
| No operator runbook ("cert failed to renew, what now?") | **will-cry-at-3am** |
| No threat model doc (state-dir trust assumptions, process isolation) | **will-cry-at-3am** |
| No migration/compatibility guide for config changes | **papercut** |
| Known limitations section is incomplete | **papercut** |

### Suggested fix shape

- Add `docs/operations.md`: certificate lifecycle, how to inspect state, how to force renewal, what to check when renewal fails, log message reference.
- Add `docs/security.md`: threat model, `state_dir` ownership requirements, `directory_ca_file` usage policy.
- Add a "config compatibility" section to `README.md` noting which fields were removed/renamed and what to migrate.
- Extend "known limitations" with the durability and observability gaps.

**Effort:** 1–2 days.

---

## Milestone plan

Each milestone is a small, concrete set of issues. Single-instance correctness is
addressed before multi-instance concerns.

### v0.1 — Don't lose data

*Goal: make the single-instance case crash-safe.*

1. Atomic + durable writes for all `state_dir` files (`account.json`, `cert.pem`,
   `key.pem`, `backoff.json`), with correct file permissions.
2. Atomically committed cert/key pair in `state_dir` (no torn pair on crash).
3. Config validation at load time: reject `tick_seconds: 0`, `domains: []`, and
   obviously bad values.
4. Advisory lock on `state_dir` (single-writer enforcement).

### v0.2 — Operable

*Goal: an operator can diagnose problems without reading the source code.*

1. Structured metrics: issuance counters, consecutive-failures gauge, next-retry
   timestamp gauge, cert `not_after` gauge, issuance latency histogram.
2. Startup log distinguishing fresh vs. cached-state restart.
3. Periodic status heartbeat log (current cert expiry, last renewal, next attempt).
4. Runtime-thread liveness watchdog with escalated logging on unexpected exit.

### v0.3 — Shippable

*Goal: safe to run against production Let's Encrypt.*

1. Cached-cert validation against configured SANs + key consistency check.
2. Per-tick issuance deadline (bounded `timeout`).
3. Expanded ACME problem classification (`accountDoesNotExist` → recovery state).
4. HTTPS scheme enforcement for `staging`/`production` profiles.
5. Host-bound challenge responder.
6. `cargo deny` / `cargo audit` in CI; release artifact signing.

### v1.0 — Supported

*Goal: production-quality, documented, and maintainable.*

1. Failure-injection + soak test suite.
2. Operator runbook (`docs/operations.md`) and threat model (`docs/security.md`).
3. Multi-instance strategy (either enforced single-instance contract with tooling, or
   leader-election design).
4. Supported architecture matrix; SLSA provenance on release artifacts.
5. Config schema versioning + migration guide.
