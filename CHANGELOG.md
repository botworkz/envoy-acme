# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/).

## [Unreleased]

### Changed

- **Breaking:** Unknown keys in `acme:`, `cert_sink:`, `log:`, and the HTTP
  filter `filter_config` are now **rejected at startup** with an error that
  names the offending field. Any config containing stale or mistyped keys
  (e.g. `directory_cluster:`, `celr_dir:`) will fail to load. Update your
  configs to remove any unrecognised keys before upgrading.
- `issuance_timeout_seconds` default raised from 120 s to 300 s (#140).
- ACME directory URI must use `https://` unless `allow_insecure_directory: true`
  is set (custom profile only).

### Added

- `directory_profile` field (`staging`, `production`, `custom`) to select a
  known ACME directory by name (#16).
- IDNA domain normalisation: Unicode (U-label) and Punycode (A-label) inputs
  are accepted and normalised to A-label per RFC 5890 / UTS#46 (#121, #144).
- `contact` field validation: only `mailto:` URIs are accepted (#141).
- `envoy_acme_account_state` gauge for operator alerting on account health (#108).
- `envoy_acme_issuance_duration_seconds` histogram (#42).
- Startup warning when `state_dir` is group- or world-readable (#107).
- Startup warning when `state_dir` and `cert_dir` are on different filesystems
  (#151). Set `ENVOY_ACME_ALLOW_CROSS_FS_DIRS=1` to suppress.
- Challenge token TTL eviction for `ChallengeStore` (#93).
- RAII drop-guard for challenge tokens on order teardown (#145).
- Bundle sentinel integrity check (`bundle.ok` SHA-256 verification) (#39, #64).
- Liveness watchdog escalation (#40).
- Periodic heartbeat logs (#43).
- Backoff with jitter on issuance failure (#14).
- Atomic writes for all state files (#27, #38).

## [0.0.1] - 2026-06-29

Initial implementation.

[Unreleased]: https://github.com/botworkz/envoy-acme/compare/v0.0.1...HEAD
[0.0.1]: https://github.com/botworkz/envoy-acme/releases/tag/v0.0.1
