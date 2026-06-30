# Production-Readiness Audit \#2 — 2026-Q3

| Field | Value |
|---|---|
| **Audit date** | 2026-06-30 |
| **Target SHA** (`main` HEAD) | `6b292abd49622515be77a51c951c91deba878e6e` |
| **Baseline (Audit \#1)** | PR \#46 head SHA `adf8260fce7aa00297bb8184d910d03994fd7f41` · blob SHA `c3bdab0375c40169c3b1057a78c2e53662aaa46c` · `docs/production-readiness-audit.md` |
| **Auditor** | GitHub Copilot deep-research agent |
| **Scope** | `src/`, `Cargo.toml`, `Cargo.lock`, `deny.toml`, `envoy/`, `config/` |

---

## Executive Summary

- **Not yet recommended for production against the real Let's Encrypt endpoint.** Two blockers must be resolved first: the default `issuance_timeout_seconds` of 120 s is exactly equal to the worst-case polling budget (two loops × 30 polls × 2 s = 120 s), leaving zero headroom for network latency and guaranteeing sporadic timeouts on any CA that is not millisecond-fast (Finding F-03); and `acme.contact` is not validated for the required `mailto:` URI prefix, so a common misconfiguration results in an opaque `malformed` ACME error rather than a startup rejection (Finding F-06).
- **Audit \#1's durability, config-safety, and security findings are substantially resolved.** Atomic writes, the `bundle.ok` sentinel, IDNA normalisation (PR \#121), HTTPS scheme enforcement, host-bound challenge responder, challenge-TTL eviction, bounded tick channel, issuance timeout, and `cargo deny` CI integration are all in place.
- **The `std::sync::Mutex` in `src/metrics.rs` is poisonable from the Envoy worker thread.** A panic in the runtime thread while holding the metrics lock poisons it; the next call on the Envoy callback thread calls `.lock().unwrap()` and re-panics (Finding F-01). This is a crash path through Envoy's own event loop.
- **`flock`-based state locking provides no cross-host mutual exclusion on NFS/EFS.** The lock works for same-host multi-process prevention, but a deployment on a shared network filesystem with multiple replicas (a documented deployment shape in the README-level expectations) will obtain the lock on every replica simultaneously (Finding F-08).
- **`serde_yaml 0.9.34+deprecated` is an unmaintained dependency** used in both config parsing and SDS YAML generation. The `deny.toml` does not track it, so `cargo deny check` will pass silently (Finding F-11).
- **Smallest change-set to reach production readiness:** (1) raise `issuance_timeout_seconds` default to 300 s; (2) validate `acme.contact` for `mailto:` prefix at config time; (3) replace `std::sync::Mutex` in `metrics.rs` with `parking_lot::Mutex` (poison-free); (4) add a prose note to state-dir documentation warning that `flock` is ineffective on NFS.

---

## Baseline-Compliance Pass

Every numbered finding from Audit \#1 (`docs/production-readiness-audit.md`, PR \#46) is re-verified against `main` HEAD `6b292abd`.

| \# | Audit \#1 Finding | Status | Evidence on `main` |
|---|---|---|---|
| §1-a | Non-atomic writes to `state_dir` (cert.pem / key.pem / account.json) | **SUPERSEDED-BY-CODE** | `src/atomic_write.rs`; every `write_atomic` call uses `NamedTempFile::new_in(parent)` + `tmp.persist(path)` (rename) + `sync_all`. Used for all state-dir files. |
| §1-b | No integrity sentinel guarding cert+key pair across crash | **SUPERSEDED-BY-CODE** | `src/acme/mod.rs:561-583` (`persist_bundle`): writes cert → key → `bundle.ok` (SHA-256 of cert.pem). `load_cached_bundle` (line 502-558) rejects the pair if the sentinel is absent or does not match. |
| §1-c | Private key written with world-readable permissions | **SUPERSEDED-BY-CODE** | `src/atomic_write.rs`: `write_atomic(path, bytes, /*private=*/true)` calls `set_permissions(0o600)` before rename. Key and account files pass `true`. |
| §1-d | No advisory lock; multi-process race on `state_dir` | **PARTIALLY-FIXED** | `src/state_lock.rs` implements `flock(LOCK_EX|LOCK_NB)`. Effective for same-host multi-process. **Not effective on NFS** (see Finding F-08). |
| §2-a | Corrupted `backoff.json` silently reset to defaults | **SUPERSEDED-BY-CODE** | `src/acme/mod.rs:372-392`: corrupt backoff JSON triggers `quarantine_corrupt_file` (renames to `backoff.json.corrupt.<timestamp>.<nonce>`) and logs at `error!`. |
| §2-b | Cached cert not validated against configured domains | **SUPERSEDED-BY-CODE** | `src/acme/mod.rs:667-734` (`validate_bundle`): checks SAN set covers every configured domain, and SPKI match between cert and key. Called on both cache-load and post-issuance. |
| §2-c | `domains: []` not rejected at config time | **SUPERSEDED-BY-CODE** | `src/config.rs:184`: `if raw.domains.is_empty() { return Err(...) }`. |
| §2-d | No IDN / wildcard normalisation | **SUPERSEDED-BY-CODE** | PR \#121 (`6b292abd`): `src/config.rs:383-405` (`normalise_domain`) uses `idna::uts46::Uts46::to_ascii` with `AsciiDenyList::STD3 + DnsLength::Verify`, nontransitional. Wildcards rejected at line 362. |
| §3-a | Empty or malformed cert chain accepted without structural check | **SUPERSEDED-BY-CODE** | `validate_bundle` runs after issuance; `assemble_bundle` (`src/acme/order.rs:104-115`) rejects `None` cert chain with `OrderFailed`. |
| §3-b | `accountDoesNotExist` classified as `Transient` → infinite retry | **SUPERSEDED-BY-CODE** | `src/acme/backoff.rs:90-113`: `accountDoesNotExist` and `unauthorized` → `ErrorClass::RecoveryRequired`. Rate-limited `error!` log once per 60 ticks. |
| §3-c | No per-tick end-to-end deadline | **SUPERSEDED-BY-CODE** | `src/acme/mod.rs:331`: `tokio::time::timeout(Duration::from_secs(self.config.issuance_timeout_seconds), ...)` wraps entire issuance. Field range-validated to [5, 600], default 120. |
| §3-d | Missing `badNonce` retry | **SUPERSEDED-BY-CODE** | `instant-acme 0.7` handles `badNonce` internally with a single retry per request. |
| §4-a | No metrics | **SUPERSEDED-BY-CODE** | `src/metrics.rs`: Envoy-native counters/gauges/histograms — `envoy_acme_issuance_total{result}`, `envoy_acme_consecutive_failures{domain}`, `envoy_acme_next_retry_at_seconds{domain}`, `envoy_acme_cert_not_after_seconds{domain}`, `envoy_acme_account_state{domain}`, `envoy_acme_issuance_duration_seconds`. Heartbeat log every 60 ticks. |
| §4-b | Runtime thread death silent | **SUPERSEDED-BY-CODE** | `src/bootstrap.rs`: `runtime_alive` `AtomicBool`; `handle_runtime_tick_result` emits rate-limited `error!` (once per 60 s) when runtime is dead. |
| §5-a | No HTTPS scheme enforcement on `directory_uri` | **SUPERSEDED-BY-CODE** | `src/config.rs:136-168`: `staging`/`production` require HTTPS; `custom` + HTTP requires `allow_insecure_directory: true`; all other cases rejected. |
| §5-b | Challenge responder not host-bound | **SUPERSEDED-BY-CODE** | `src/http_filter.rs`: `:authority` header extracted via `normalize_host`; challenge served only when authority matches a configured domain. Falls through to `Continue` otherwise. |
| §5-c | Challenge tokens never evicted | **SUPERSEDED-BY-CODE** | `src/challenge_store.rs`: TTL = 600 s, sweep-on-insert, double-checked expiry on `lookup`. |
| §6-a | No `cargo audit` / `cargo deny` in CI | **SUPERSEDED-BY-CODE** | `.github/workflows/ci.yaml` runs `cargo deny check`; `deny.toml` present with `RUSTSEC-2025-0134` exception (rustls-pemfile). |
| §6-b | Release artefacts unsigned | **STILL-OPEN** | No sigstore / SLSA provenance. Out of scope for this audit cycle. |
| §7-a | `tick_seconds: 0` → busy-loop | **SUPERSEDED-BY-CODE** | `src/config.rs:170`: `if raw.tick_seconds == 0 { return Err(...) }`. |
| §7-b | Unbounded tick queue → memory growth | **SUPERSEDED-BY-CODE** | `src/runtime.rs`: `mpsc::channel(1)`; excess ticks are dropped with `debug!` log and `dropped_tick_count` counter. |
| §8-a | `renewal_window_days` out-of-range not rejected | **SUPERSEDED-BY-CODE** | `src/config.rs:175-178`: checked in [1, 365]. |
| §8-b | Unknown config keys silently ignored | **SUPERSEDED-BY-CODE** | `#[serde(deny_unknown_fields)]` on `RawAcmeConfig` (config.rs:30) and `RawCertSinkConfig`. |
| §8-c | `cert_sink.type` not enforced | **SUPERSEDED-BY-CODE** | `CertSinkType` is a closed enum; unknown variants rejected by `deny_unknown_fields`. |
| §8-d | No writability probe at startup | **SUPERSEDED-BY-CODE** | `src/bootstrap.rs`: `probe_writable` creates + deletes a test file in `state_dir` and `cert_dir` at bootstrap. |
| §9-a | No failure-injection / adversarial tests | **STILL-OPEN** | Unit tests exercise the mock ACME client for happy path + some error states; no chaos/injection tests for IO failures, disk-full, or partial-write scenarios. |
| §9-b | Integration timing sensitive (hardcoded 120 s wait) | **STILL-OPEN** | `.github/workflows/ci.yaml` integration job polls for 120 s (60 × 2 s); this is tight against slow CI runners. |
| §9-c | No concurrent reload / SDS atomicity test | **STILL-OPEN** | No test verifies Envoy SDS never observes a stale-key + new-cert pair. |
| §10 | Documentation gaps | **OUT-OF-SCOPE** | Excluded per audit \#2 scope definition. |

---

## Findings

### F-01 · `std::sync::Mutex` in metrics.rs is poisonable on the Envoy callback thread

**Severity:** `BLOCKER`  
**Status:** `NEW-FINDING`  
**Lens:** `[CODE-QUALITY]` / `[ROBUSTNESS]`  
**Code citation:** `src/metrics.rs:85, 99, 189, 207, 291`

**Observed behaviour:** All five production-path lock acquisitions in `metrics.rs` call `std::sync::Mutex::lock().unwrap()`. Rust's `std::sync::Mutex` becomes *poisoned* if a thread panics while holding it. The metrics lock is also held in the `runtime` thread (via `enqueue_many`, which calls `state.pending.lock().unwrap()` at line 99) and in the Envoy callback thread (via `init`, `get_state`, `drop_state`). If the Tokio runtime thread panics while holding `pending` (e.g. an unexpected `None` unwrap inside a metrics helper), the mutex is poisoned. The next call to `.lock().unwrap()` on the Envoy event-loop thread will panic, crashing Envoy's extension thread.

**Why it matters:** This is an inter-thread failure amplification: a panic in the background runtime thread cascades into a crash on the Envoy worker thread. The Envoy process may then terminate the entire extension host.

**Smallest fix:** Replace `std::sync::Mutex` with `parking_lot::Mutex` throughout `src/metrics.rs`. `parking_lot::Mutex` never poisons — a lock acquired after a holder panic simply succeeds. The crate is already a dependency (`Cargo.toml:17`).

---

### F-02 · Default `issuance_timeout_seconds = 120` is equal to worst-case polling time

**Severity:** `BLOCKER`  
**Status:** `NEW-FINDING`  
**Lens:** `[ACME]` / `[ROBUSTNESS]`  
**Code citation:** `src/acme/order.rs:14-15` (`MAX_POLLS = 30`, `POLL_INTERVAL = 2 s`); `src/config.rs:208-213` (default 120, range [5, 600]); `src/acme/mod.rs:331` (timeout wraps entire issuance)

**Observed behaviour:** `issue_certificate` runs two sequential polling loops, each bounded to `MAX_POLLS × POLL_INTERVAL = 30 × 2 s = 60 s`. Total worst-case polling time is 120 s. The outer `tokio::time::timeout(issuance_timeout_seconds)` wraps the entire issuance path, including account creation/fetch, `newOrder`, challenge setup, `set_challenge_ready`, both polling loops, `finalize`, and the certificate download loop. Against Pebble (sub-millisecond) this is fine. Against real Let's Encrypt, where each HTTPS round-trip typically takes 100–500 ms, the account fetch + order creation + 30 authorization polls + finalize + 30 certificate polls can comfortably exceed 120 s on a slow connection or under LE load. When the timeout fires, the error is classified as `Transient` (a timed-out `AcmeError`) and retried on the next tick, but challenge tokens for the abandoned order are cleaned up correctly.

**Why it matters:** With the default configuration, valid issuance attempts against the real LE production endpoint will timeout intermittently on any host with >4 ms average round-trip time to LE. Operators will see recurring `Transient` backoff without obvious cause.

**Smallest fix:** Raise the default `issuance_timeout_seconds` to 300 s (config.rs line 209). The upper bound of 600 s in the validator is adequate. Optionally document that lowering below 200 s is only appropriate for Pebble/staging environments.

---

### F-03 · `acme.contact` not validated for `mailto:` URI prefix

**Severity:** `BLOCKER`  
**Status:** `NEW-FINDING`  
**Lens:** `[LE]`  
**Code citation:** `src/config.rs:199-201`

**Observed behaviour:**
```rust
if raw.contact.trim().is_empty() {
    return Err("acme.contact must be non-empty".to_string());
}
```
The only check is non-empty. A value of `"admin@example.com"` (bare email, missing `mailto:` prefix) or `"ftp://admin@example.com"` passes config validation. RFC 8555 §7.3 requires contact entries to be absolute URIs. Let's Encrypt specifically requires `mailto:` URIs and rejects others with a `malformed` ACME problem document. This manifests as an opaque error on the first account-creation attempt, not at process startup.

**Why it matters:** `mailto:` prefix omission is the most common misconfiguration in ACME clients. The current error (surfaced via `AcmeError::Acme(instant_acme::Error::...)` classified as `Permanent`) will permanently disable issuance with a confusing error log, and operators will not have a clear signal about which config field to fix.

**Smallest fix:** In `TryFrom<RawAcmeConfig>` (config.rs around line 199), reject any `contact` that does not begin with `mailto:`. A single `starts_with("mailto:")` check suffices; RFC 8555 does not mandate validating the mailbox portion.

---

### F-04 · `Hyphens::Allow` in IDNA normalization accepts labels violating RFC 5891 §5.4

**Severity:** `SHOULD-FIX`  
**Status:** `NEW-FINDING`  
**Lens:** `[LE]` / `[ACME]`  
**Code citation:** `src/config.rs:387, 394`

**Observed behaviour:** `normalise_domain` uses `idna::uts46::Hyphens::Allow`, which skips position-of-hyphen checks entirely. The comment at line 387 documents this as a deliberate choice ("matches original behaviour"). RFC 5891 §5.4 and CA/Browser Forum BR §7.1.4.2 both require that no A-label may have a hyphen in the third-and-fourth character positions unless the label begins with `xn--`. `Hyphens::Allow` skips this check, so `"ab--cd.example.com"` passes `normalise_domain` and is sent to the CA. Let's Encrypt will reject the CSR at the order level with `rejectedIdentifier`, not at config time.

**Why it matters:** BR-violating domain names fail at the CA rather than locally. The failure is classified as `Permanent` (from instant-acme's error response) and permanently disables issuance for that domain. Using `Hyphens::CheckThirdAndFourth` would catch the same errors at startup, giving operators a clear rejection reason.

**Smallest fix:** Change `Hyphens::Allow` to `Hyphens::CheckThirdAndFourth` at `config.rs:394`. Update the comment accordingly. Verify that the existing IDNA config tests still pass (they use ASCII domains without third-position hyphens, so no test changes should be needed).

---

### F-05 · `Retry-After` header from ACME server is not honoured

**Severity:** `SHOULD-FIX`  
**Status:** `NEW-FINDING`  
**Lens:** `[ACME]` / `[LE]`  
**Code citation:** `src/acme/backoff.rs:90-113`; `src/acme/account.rs` (no header-reading code)

**Observed behaviour:** When the ACME server returns HTTP 429 with a `rateLimited` problem document, the code classifies the error as `ErrorClass::RateLimited` and applies its own exponential backoff schedule (`base_backoff_seconds = 60`, doubling up to `max_backoff_seconds = 86400`). The `Retry-After` header from the server's response is not read. RFC 8555 §6.6 says: "If a server does not wish to respond to a request immediately, it may respond with a `Retry-After` header field." Let's Encrypt's rate-limit responses include explicit `Retry-After` values (e.g. 3600 s for weekly limits) that may differ substantially from the client's 60 s base.

**Why it matters:** Ignoring `Retry-After` means the client may retry far sooner than the CA expects, generating additional failed-request noise, or (more commonly) retry far later than needed (if the server-suggested wait is shorter). `instant-acme 0.7` does not surface the `Retry-After` header from the transport layer, so implementing this requires either a custom middleware layer on the HTTP client or a version of instant-acme that exposes response headers.

**Smallest fix:** File an upstream issue against `instant-acme` requesting `Retry-After` header exposure. In the meantime, document in the backoff module that `Retry-After` is not honoured and that the base 60 s backoff is intentionally conservative relative to LE's actual rate-limit windows.

---

### F-06 · ACME directory resource is re-fetched on every issuance tick but not on transient 5xx mid-issuance

**Severity:** `SHOULD-FIX`  
**Status:** `NEW-FINDING`  
**Lens:** `[ACME]`  
**Code citation:** `src/acme/account.rs:127-169` (`load_or_create_account`); `src/acme/mod.rs:322-340`

**Observed behaviour:** `load_or_create_account` is called at the start of every issuance tick that requires renewal. When `account.json` exists, it calls `Account::from_credentials(credentials).await?`, which in `instant-acme 0.7` fetches the ACME directory to initialise the account's nonce store and URL map. This is correct re-fetch behaviour (not caching stale directory). However, if the directory fetch itself returns 5xx, the resulting error is `AcmeError::Acme(...)` and is classified as `Transient` — which triggers exponential backoff, not immediate retry. A brief ACME CDN blip causing a single-tick 503 will result in a 60 s (or longer) backoff delay before the next attempt, even though the cert may be close to its renewal window.

Additionally, the ACME directory's `meta.termsOfService` URL is never read or surfaced. LE rotates its Terms of Service periodically; the code unconditionally passes `terms_of_service_agreed: true` on account creation (`account.rs:147`). Post-creation, ToS changes are not detected — this is standard behaviour for most ACME clients.

**Why it matters:** A short CDN disruption upstream of the ACME directory endpoint causes unnecessary backoff delay. Operators cannot observe ToS changes until they cause account-level errors.

**Smallest fix:** For directory 5xx: lower the initial backoff for `Transient` errors to a shorter floor (e.g. 10 s for the first retry). For ToS: the current behaviour (accept once at creation) matches the practice of all major ACME clients and is acceptable.

---

### F-07 · Authorization `expires` field not honoured during polling

**Severity:** `SHOULD-FIX`  
**Status:** `NEW-FINDING`  
**Lens:** `[ACME]`  
**Code citation:** `src/acme/order.rs:148-168` (authorization polling loop)

**Observed behaviour:** RFC 8555 §7.1.4 specifies that an `Authorization` object has an `expires` field after which the authorization and any pending challenges are no longer valid. The `evaluate_authorization` and polling code in `order.rs` never reads `authz.expires`. If the outer `issuance_timeout_seconds` budget is generous (e.g. 300 s) and the CA issues an authorization with `expires` in < 300 s, the polling loop will continue beyond the authorization's validity window, sending `set_challenge_ready` on a stale authorization. The CA will respond with `malformed` or `orderNotReady`, which is classified as `Permanent` by `classify_problem`, permanently blocking the domain.

**Why it matters:** Authorizations on Let's Encrypt's production endpoint are valid for 30 days; this is unlikely to trigger in practice. However, Pebble in CI sets authorization lifetimes as low as a few seconds in some configurations, and this code path has not been hardened.

**Smallest fix:** In `evaluate_authorization`, check `authz.expires` against the current time and return an error (classified as `Transient`, not `Permanent`) if the authorization has already expired, allowing the next tick to create a fresh order.

---

### F-08 · `flock`-based state lock provides no cross-host exclusion on NFS / EFS

**Severity:** `SHOULD-FIX`  
**Status:** `STILL-OPEN` (partially addresses audit \#1 §1-d)  
**Lens:** `[FITNESS]`  
**Code citation:** `src/state_lock.rs:1-82`

**Observed behaviour:** `StateLock::acquire` calls `flock(fd, LOCK_EX | LOCK_NB)`. On Linux NFSv3, `flock` uses a byte-range lock internally but the kernel's NFS client does not forward `flock` advisory locks to the server by default; two processes on different NFS-client hosts can both `flock` the same file and both succeed. NFSv4 does propagate POSIX byte-range locks, but `flock` is still advisory and its cross-host behaviour is mount-option-dependent. The result: on an EFS or NFS shared `state_dir` (a common deployment pattern for Kubernetes persistent volumes), multiple replicas will each acquire the lock independently and issue certificates simultaneously.

**Why it matters:** Simultaneous issuance from multiple replicas against the same ACME directory URL and contact creates duplicate certificates, inflates LE's new-certificate rate counters, and may trigger LE's duplicate-certificate rate limit (5 identical SANs per rolling 7-day window). It does not corrupt state (each replica writes atomically to its own set of files within a shared directory) but it is operationally unsound.

**Smallest fix:** Document in the state-lock module and in operator-facing documentation that `flock` provides same-host mutual exclusion only, and that multi-host deployments on shared storage must ensure only one replica actively manages the state directory at a time (e.g. via a Kubernetes `StatefulSet` with a single-primary replica or a sidecar-based leader election). A note in `state_lock.rs` explaining the NFS limitation is the minimal change.

---

### F-09 · Challenge tokens not cleaned up on `Shutdown` if issuance is mid-flight

**Severity:** `SHOULD-FIX`  
**Status:** `NEW-FINDING`  
**Lens:** `[FITNESS]` / `[ROBUSTNESS]`  
**Code citation:** `src/runtime.rs:Command::Shutdown` handler; `src/acme/order.rs:182-184` (cleanup on `Err`); `src/challenge_store.rs`

**Observed behaviour:** `issue_certificate` cleans up challenge tokens in two places: on the error path at line 162-164 (before returning `Err`) and on success at line 182-184. If `tokio::time::timeout` fires while the task is awaiting `order.refresh()` inside the polling loop (not at a cleanup point), the `timeout` error unwinds the `tick_at` coroutine. The `challenge_tokens` `Vec` drops without calling `challenge_store::remove`. Tokens inserted at line 141 remain in the store until the 600 s TTL expires.

During an Envoy hot-restart, `Shutdown` is dispatched to the runtime, which calls `sm.clear_challenges()` (wipes all stored tokens) and then drops the Tokio runtime. Any in-flight `tick_at` coroutine is cancelled at its next await point. Whether `clear_challenges` runs before or after the in-flight tick is cancelled depends on `runtime.rs` channel ordering. However, the 600 s TTL on individual tokens is an adequate safety net.

**Why it matters:** Stale challenge tokens are served by the HTTP filter for up to 600 s after the issuance attempt that registered them. A CA that re-validates after the order has expired (unlikely but RFC-valid) would receive a correct response for a stale token. This is not exploitable (the CA controls what is validated) but is technically incorrect state.

**Smallest fix:** Wrap `challenge_tokens` in a `scopeguard` or `Drop` guard inside `issue_certificate` that calls `remove` for each token on drop. This ensures cleanup regardless of how the function exits.

---

### F-10 · `cert_dir` cross-filesystem placement causes `persist` failure at runtime

**Severity:** `SHOULD-FIX`  
**Status:** `NEW-FINDING`  
**Lens:** `[ROBUSTNESS]` / `[FITNESS]`  
**Code citation:** `src/atomic_write.rs:1-60`; `src/cert_sink/filesystem.rs`

**Observed behaviour:** `write_atomic` creates a `NamedTempFile` in `path.parent()` (the target directory), then calls `tmp.persist(path)` which executes `std::fs::rename`. `rename` is atomic only within a single filesystem. If `cert_dir` and `state_dir` are on different filesystem mount points (common in Docker/Kubernetes configurations where `/var/lib/envoy-acme` is a persistent volume and `/tmp` is `tmpfs`), and `cert_dir` is set to a path whose parent is on a different filesystem from the temp directory, `persist` returns `PersistError`. Since the temp file is created in `cert_dir`'s parent directory, and the rename is within the same directory, this is only a problem if `cert_dir` itself is on a mount boundary. In practice, setting `cert_dir` to `/var/lib/envoy-acme/certs` (on the same volume) is safe; `/mnt/certs` (separate mount) would fail.

**Why it matters:** The failure surfaces as `AcmeError::Io(PersistError)` classified as `Permanent` by `ErrorClass`, permanently disabling issuance until the deployment configuration is corrected. The error message does not mention the cross-filesystem cause, making debugging difficult.

**Smallest fix:** At startup in `probe_writable`, verify that `cert_dir` and `state_dir` resolve to the same device number (`fs::metadata(path).dev()` on Linux). If they differ, log a warning that cross-filesystem temp→target rename will be used (i.e. non-atomic copy+rename fallback). Alternatively, document the same-filesystem requirement in config.

---

### F-11 · `serde_yaml 0.9.34+deprecated` is unmaintained and untracked in `deny.toml`

**Severity:** `SHOULD-FIX`  
**Status:** `NEW-FINDING`  
**Lens:** `[CODE-QUALITY]`  
**Code citation:** `Cargo.lock:1358` (`version = "0.9.34+deprecated"`); `deny.toml` (no exception for serde_yaml); `src/config.rs:412` (`serde_yaml::from_slice`); `src/cert_sink/filesystem.rs` (SDS YAML output via `serde_yaml::to_string`)

**Observed behaviour:** The `serde_yaml` crate's maintainer has explicitly marked it deprecated on crates.io (signalled by the `+deprecated` version suffix). There is no published `RUSTSEC` advisory yet, but `cargo deny check` with the current `deny.toml` will warn under the `[advisories]` policy about the unmaintained status once one is filed. The crate is used for two purposes: parsing YAML config (`Config::from_bytes` fallback path) and serializing the Envoy SDS secret YAML (`cert_sink/filesystem.rs`).

**Why it matters:** Unmaintained YAML parsing libraries are a security surface risk (YAML has a history of unsafe-deserialization CVEs). The SDS output use-case does not require a full YAML serializer; a template-string approach or a maintained alternative (e.g. `serde-yaml2`, `marked-yaml`, or custom serialization) would eliminate the dependency.

**Smallest fix:** Add `serde_yaml` to `deny.toml`'s `[advisories]` exceptions block (matching the `rustls-pemfile` precedent at line 26) with a `reason` noting the intended migration. Open a tracking issue for replacing it with a maintained alternative.

---

### F-12 · `validate_bundle` only parses the leaf certificate; intermediate chain not verified structurally

**Severity:** `NICE-TO-HAVE`  
**Status:** `NEW-FINDING`  
**Lens:** `[ACME]`  
**Code citation:** `src/acme/mod.rs:672` (`x509_parser::pem::parse_x509_pem(&bundle.cert_pem)`); `src/acme/renewal.rs:58` (same call)

**Observed behaviour:** `x509_parser::pem::parse_x509_pem` parses the first PEM block in the buffer. `cert_pem` (stored in `CertBundle`) contains the full chain returned by the ACME server (leaf + intermediates). Both `validate_bundle` (SAN + key check) and `cert_not_after_unix` (renewal window) operate only on the first PEM block — the leaf certificate. This is correct for validity checking (the leaf's SANs and expiry are what matter) and for Envoy SDS (which receives the full chain as `certificate_chain` and validates it). However, if the CA returns an empty chain or a chain with a malformed second block (e.g. a truncated intermediate), neither function detects it. Envoy may then fail to build a complete TLS chain, leading to TLS handshake failures with clients that require the intermediate.

**Why it matters:** A CA-side chain encoding bug or an intermediate rotation event could produce a cert PEM that validates at the leaf level but fails in Envoy. The error would surface as Envoy TLS handshake failures rather than as an issuance error, making diagnosis harder.

**Smallest fix:** In `validate_bundle`, after validating the leaf, iterate the remaining PEM blocks and verify each parses as a valid X.509 certificate. Log a warning (not an error) for any block that fails to parse, and return `Err` if the chain has zero blocks after the leaf (i.e. leaf-only, when the CA is expected to return an intermediate).

---

### F-13 · `not_before > now()` not checked; cert valid only in the future served immediately to Envoy SDS

**Severity:** `NICE-TO-HAVE`  
**Status:** `NEW-FINDING`  
**Lens:** `[ROBUSTNESS]`  
**Code citation:** `src/acme/mod.rs:109` (checks `not_after <= now`); `src/acme/mod.rs:667-734` (no `not_before` check)

**Observed behaviour:** `inspect_state` at line 109 catches expired certs (`not_after <= now`). Neither `validate_bundle` nor `inspect_state` checks `not_before`. If the CA returns a cert with a `not_before` timestamp slightly in the future (Let's Encrypt currently uses `notBefore = issuance time`, so this does not occur in practice), the cert is published to Envoy SDS and TLS clients that enforce `notBefore` strictly will reject it. This is also the scenario if the host clock runs ahead of the CA's clock.

**Why it matters:** Not an issue with current Let's Encrypt behavior, but would manifest as TLS client errors rather than an issuance-level rejection. A simple check (`not_before > now + some_skew_buffer`) costs nothing.

**Smallest fix:** In `validate_bundle`, read `tbs_certificate.validity.not_before` and reject the cert if `not_before_unix > now_unix + 300` (5-minute skew tolerance). Return `CertCachedButInvalid` with a descriptive reason, triggering re-issuance.

---

### F-14 · `challenge_store::ChallengeMap` type alias is `pub`, leaking internal lock type

**Severity:** `NICE-TO-HAVE`  
**Status:** `NEW-FINDING`  
**Lens:** `[CODE-QUALITY]`  
**Code citation:** `src/challenge_store.rs:16`

**Observed behaviour:** `pub type ChallengeMap = Arc<RwLock<HashMap<String, Entry>>>` is declared `pub`. The `init`, `insert`, `remove`, `lookup`, and `get` free functions are also all `pub`. Since the crate is `crate-type = ["cdylib"]`, external Rust crates cannot link against it, so no external consumer is affected. However, the `pub` visibility is a maintenance hazard: it allows any code in the same compilation unit (e.g. future modules) to bypass the module's intended API and obtain a direct `Arc<RwLock<...>>` reference.

**Smallest fix:** Change to `pub(crate)` for `ChallengeMap`, `Entry`, `init`, `insert`, `remove`, `lookup`, and `get`. This makes the abstraction boundary explicit and does not require any call-site changes.

---

### F-15 · `tick_seconds` < `issuance_timeout_seconds` combination not validated or warned

**Severity:** `NICE-TO-HAVE`  
**Status:** `NEW-FINDING`  
**Lens:** `[ROBUSTNESS]`  
**Code citation:** `src/config.rs:170-172` (`tick_seconds ≥ 1` only); no cross-field validation between `tick_seconds` and `issuance_timeout_seconds`

**Observed behaviour:** If `tick_seconds = 5` and `issuance_timeout_seconds = 120`, the timer fires every 5 s but each tick can block the runtime thread for up to 120 s. The bounded channel (capacity 1) coalesces excess ticks, so at most one extra tick is queued. After the in-flight tick completes, the queued tick starts immediately — effectively removing the `tick_seconds` gap between retries during issuance. On repeated failure, the retry interval collapses to near-zero + backoff, which then interacts with LE's 5-per-hour failed-validation rate limit.

**Smallest fix:** Add a startup warning (not an error) when `tick_seconds < issuance_timeout_seconds / 10`, noting that the effective retry interval during issuance will be dominated by the issuance timeout rather than `tick_seconds`. Alternatively, document this interaction in config comments.

---

### F-16 · `std::thread::sleep` in `challenge_store` tests is timing-sensitive

**Severity:** `NICE-TO-HAVE`  
**Status:** `NEW-FINDING`  
**Lens:** `[CODE-QUALITY]`  
**Code citation:** `src/challenge_store.rs:148, 165`

**Observed behaviour:** TTL eviction tests use `std::thread::sleep(Duration::from_millis(100))` to advance past a 50 ms TTL. On a heavily loaded CI runner, the sleep may not be sufficient to guarantee the entry is expired before the subsequent assertion. This makes these tests sporadically flaky.

**Smallest fix:** Replace the fixed-TTL + real-sleep pattern with a configurable TTL (passed to `ChallengeStore::new_with_ttl` or similar) and set the TTL to 1 ms in tests, making them robust to scheduling jitter without relying on wall-clock precision.

---

## Suggested Next Steps

The following `BLOCKER`- and `SHOULD-FIX`-severity findings should become tracked GitHub issues in priority order:

1. **F-02 — Default `issuance_timeout_seconds = 120` too tight.** Raise default to 300 s. This is a one-line config change; lowest risk and highest impact for immediate LE production readiness.

2. **F-03 — `acme.contact` missing `mailto:` validation.** Add `starts_with("mailto:")` check in `TryFrom<RawAcmeConfig>`. One-line fix; prevents the most common first-run misconfiguration.

3. **F-01 — `std::sync::Mutex` poisonable in `metrics.rs`.** Replace with `parking_lot::Mutex`. Eliminates a crash-on-Envoy-worker-thread failure mode.

4. **F-11 — `serde_yaml` deprecated and untracked.** Add to `deny.toml` exceptions immediately; schedule replacement.

5. **F-08 — `flock` NFS limitation undocumented.** Add prose to operator documentation and a code comment in `state_lock.rs`. Low effort; prevents silent multi-host issuance storms.

6. **F-04 — `Hyphens::Allow` in IDNA normalisation.** Tighten to `Hyphens::CheckThirdAndFourth` for BR compliance.

7. **F-05 — `Retry-After` header not honoured.** File upstream instant-acme issue; add inline documentation comment.

8. **F-09 — Challenge token cleanup not `Drop`-guarded.** Wrap `challenge_tokens` in a `scopeguard` for correctness on all exit paths.
