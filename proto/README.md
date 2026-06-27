# Proto handling

This project does **not** vendor Envoy's protobuf definitions and does **not**
require `protoc` at build time.

Instead, it depends on the [`envoy-types`](https://crates.io/crates/envoy-types)
crate, which ships pre-generated `prost`/`tonic` Rust types for the Envoy data
plane APIs. This keeps the build hermetic (only `cargo` is needed) and avoids a
system `protoc` dependency.

## Why `envoy-types`?

`protoc` is not assumed to be installed in CI or developer environments. The
Envoy proto set (ext_proc, SDS, discovery, TLS transport sockets) pulls in a
large web of imports (`xds`, `udpa`, `validate`, `google/api`, ...), which is
painful to vendor and compile by hand. `envoy-types` solves this once.

## Types used

| Purpose          | Rust path (via `envoy_types::pb`)                                             |
| ---------------- | ---------------------------------------------------------------------------- |
| ext_proc service | `envoy::service::ext_proc::v3::external_processor_server::ExternalProcessor`  |
| ext_proc msgs    | `envoy::service::ext_proc::v3::{ProcessingRequest, ProcessingResponse, ...}`  |
| SDS service      | `envoy::service::secret::v3::secret_discovery_service_server::SecretDiscoveryService` |
| Discovery msgs   | `envoy::service::discovery::v3::{DiscoveryRequest, DiscoveryResponse}`        |
| TLS secret       | `envoy::extensions::transport_sockets::tls::v3::{Secret, TlsCertificate}`     |
| Core types       | `envoy::config::core::v3::{HeaderMap, HeaderValue, DataSource}`               |

## Version pin

The Envoy API version is whatever `envoy-types` (see `Cargo.toml`) was generated
from. To upgrade the Envoy API surface, bump the `envoy-types` dependency.

## If you ever need to vendor protos instead

If a future requirement forces hand-vendoring (e.g. a type missing from
`envoy-types`), the recommended hermetic approach is:

1. Add `tonic-build` and `protoc-bin-vendored` to `[build-dependencies]`.
2. In `build.rs`, set `PROTOC` from `protoc_bin_vendored::protoc_bin_path()` and
   call `tonic_build::configure().compile_protos(...)`.
3. Vendor the minimal proto set under `proto/` with the upstream Envoy SHA
   recorded here.

This avoids a system `protoc` while still compiling from source.
