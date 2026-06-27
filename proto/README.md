# Proto handling

This project does **not** vendor Envoy's protobuf definitions and does **not**
require `protoc` at build time.

Instead, it depends on the [`envoy-proto`](https://github.com/phlax/envoy-proto-rs)
crate (crate name: `envoy-proto`, repo: `phlax/envoy-proto-rs`), which ships
pre-generated `prost`/`tonic` Rust types for the full Envoy data-plane API.
This keeps the build hermetic (only `cargo` is needed) and avoids a system
`protoc` dependency.

## Why `envoy-proto`?

`protoc` is not assumed to be installed in CI or developer environments. The
Envoy proto set (ext_proc, SDS, discovery, TLS transport sockets) pulls in a
large web of imports (`xds`, `udpa`, `validate`, `google/api`, ...), which is
painful to vendor and compile by hand. `envoy-proto` solves this once, tracking
the official Envoy data-plane API.

## Types used

| Purpose          | Rust path (via `envoy_proto`)                                                   |
| ---------------- | ------------------------------------------------------------------------------- |
| ext_proc service | `envoy::service::ext_proc::v3::external_processor_server::ExternalProcessor`    |
| ext_proc msgs    | `envoy::service::ext_proc::v3::{ProcessingRequest, ProcessingResponse, ...}`    |
| SDS service      | `envoy::service::secret::v3::secret_discovery_service_server::SecretDiscoveryService` |
| Discovery msgs   | `envoy::service::discovery::v3::{DiscoveryRequest, DiscoveryResponse}`          |
| TLS secret       | `envoy::extensions::transport_sockets::tls::v3::{Secret, TlsCertificate}`      |
| Core types       | `envoy::config::core::v3::{HeaderMap, HeaderValue, DataSource}`                 |

Note: `google.protobuf.Any` (used in `DiscoveryResponse.resources`) is mapped to
`prost_types::Any` in the generated code, so import it from the `prost-types` crate.

## Version pin

The Envoy API version is tracked by `phlax/envoy-proto-rs`. The Cargo.lock
records the exact git commit SHA used. To upgrade the Envoy API surface, update
the `envoy-proto` git dependency in `Cargo.toml` (or once published to crates.io,
bump the version constraint).

The proto source in `phlax/envoy-proto-rs` is generated from Envoy
[`v1.38.0`](https://github.com/envoyproxy/envoy/tree/v1.38.0) APIs.

## If you ever need to vendor protos instead

If a future requirement forces hand-vendoring (e.g. a type missing from
`envoy-proto`), the recommended hermetic approach is:

1. Add `tonic-build` and `protoc-bin-vendored` to `[build-dependencies]`.
2. In `build.rs`, set `PROTOC` from `protoc_bin_vendored::protoc_bin_path()` and
   call `tonic_build::configure().compile_protos(...)`.
3. Vendor the minimal proto set under `proto/` with the upstream Envoy SHA
   recorded here.

This avoids a system `protoc` while still compiling from source.
