//! Filesystem-backed [`CertSink`](super::CertSink) that writes an Envoy SDS secret with cert+key inlined.
use std::path::PathBuf;

use serde::Serialize;

use crate::cert_sink::{CertBundle, CertSink};
use crate::config::Layout;
use crate::errors::SinkError;

/// Writes issued certificate bundles to the local filesystem in Envoy-SDS-compatible layout.
///
/// Each renewal produces a single atomic rename of `<first-domain>.secret.yaml` with the
/// certificate chain and private key embedded as `inline_string` values.  Envoy's SDS file
/// watcher therefore observes exactly one filesystem event per renewal, eliminating the
/// cert/key mismatch window that would exist if cert and key were written as separate files.
pub struct FilesystemSink {
    dir: PathBuf,
}

// Internal types for serialising the Envoy SDS inline-secret file.
#[derive(Serialize)]
struct SdsFile<'a> {
    resources: Vec<SdsSecret<'a>>,
}

#[derive(Serialize)]
struct SdsSecret<'a> {
    #[serde(rename = "@type")]
    type_url: &'a str,
    name: &'a str,
    tls_certificate: TlsCertificate<'a>,
}

#[derive(Serialize)]
struct TlsCertificate<'a> {
    certificate_chain: DataSource<'a>,
    private_key: DataSource<'a>,
}

#[derive(Serialize)]
struct DataSource<'a> {
    inline_string: &'a str,
}

impl FilesystemSink {
    /// Create a new `FilesystemSink` that writes files into `dir` using the specified `layout`.
    pub fn new(dir: PathBuf, _layout: Layout) -> Self {
        Self { dir }
    }

    fn secret_path(&self, name: &str) -> PathBuf {
        self.dir.join(format!("{name}.secret.yaml"))
    }

    /// Derive an SDS resource name deterministically from a domain name by
    /// replacing every `.` and `-` with `_` and appending `_tls`.
    ///
    /// E.g. `example.test` → `example_test_tls`.
    fn sds_resource_name(domain: &str) -> String {
        let base: String = domain
            .chars()
            .map(|c| if c == '.' || c == '-' { '_' } else { c })
            .collect();
        format!("{base}_tls")
    }

    /// Build the Envoy SDS secret YAML with cert+key inlined and write it atomically.
    ///
    /// Cert and key are embedded as `inline_string` values so that the single atomic rename
    /// of the secret file is the only filesystem event Envoy's SDS watcher ever sees per
    /// renewal.  The file is written with mode `0o600` because it now contains the private key.
    fn write_sds_secret(&self, name: &str, cert_pem: &str, key_pem: &str) -> Result<(), SinkError> {
        let secret_path = self.secret_path(name);
        let resource_name = Self::sds_resource_name(name);

        let payload = SdsFile {
            resources: vec![SdsSecret {
                type_url: "type.googleapis.com/envoy.extensions.transport_sockets.tls.v3.Secret",
                name: &resource_name,
                tls_certificate: TlsCertificate {
                    certificate_chain: DataSource {
                        inline_string: cert_pem,
                    },
                    private_key: DataSource {
                        inline_string: key_pem,
                    },
                },
            }],
        };

        let yaml = serde_yaml::to_string(&payload).map_err(std::io::Error::other)?;

        crate::atomic_write::write_atomic(&secret_path, yaml.as_bytes(), true)?;
        Ok(())
    }
}

impl CertSink for FilesystemSink {
    fn publish(&self, name: &str, bundle: &CertBundle) -> Result<(), SinkError> {
        let cert_pem = std::str::from_utf8(&bundle.cert_pem).map_err(|e| {
            SinkError::from(std::io::Error::other(format!(
                "cert is not valid UTF-8: {e}"
            )))
        })?;
        let key_pem = std::str::from_utf8(&bundle.key_pem).map_err(|e| {
            SinkError::from(std::io::Error::other(format!(
                "key is not valid UTF-8: {e}"
            )))
        })?;
        self.write_sds_secret(name, cert_pem, key_pem)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn writes_sds_secret_yaml() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let sink = FilesystemSink::new(tmp.path().to_path_buf(), Layout::PerDomain);
        let bundle = CertBundle {
            cert_pem: b"-----BEGIN CERTIFICATE-----\nMIIcert\n-----END CERTIFICATE-----\n".to_vec(),
            key_pem: b"-----BEGIN PRIVATE KEY-----\nMIIkey\n-----END PRIVATE KEY-----\n".to_vec(),
        };

        sink.publish("example.test", &bundle).expect("publish");

        let secret_path = tmp.path().join("example.test.secret.yaml");
        assert!(secret_path.exists(), "SDS secret file should exist");

        let raw = std::fs::read_to_string(&secret_path).expect("read secret yaml");
        // Must contain the Envoy Secret type URL.
        assert!(
            raw.contains("envoy.extensions.transport_sockets.tls.v3.Secret"),
            "secret yaml missing type URL"
        );
        // Resource name derived from domain.
        assert!(
            raw.contains("example_test_tls"),
            "secret yaml missing resource name"
        );
        // Must use inline_string (not filename) for atomic single-event reload.
        assert!(
            raw.contains("inline_string"),
            "secret yaml must use inline_string, not filename"
        );
        assert!(
            !raw.contains("filename"),
            "secret yaml must not reference external filenames"
        );
        // Cert PEM content must be embedded (serde_yaml uses block scalar notation
        // so individual lines appear in the output, not the raw \n-delimited string).
        assert!(
            raw.contains("-----BEGIN CERTIFICATE-----"),
            "secret yaml missing cert PEM header"
        );
        assert!(raw.contains("MIIcert"), "secret yaml missing cert PEM body");
        // Key PEM content must be embedded.
        assert!(
            raw.contains("-----BEGIN PRIVATE KEY-----"),
            "secret yaml missing key PEM header"
        );
        assert!(raw.contains("MIIkey"), "secret yaml missing key PEM body");

        // File must be mode 0o600 on Unix (contains private key).
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let perms = std::fs::metadata(&secret_path)
                .expect("stat secret yaml")
                .permissions();
            assert_eq!(
                perms.mode() & 0o777,
                0o600,
                "secret.yaml must be 0o600 (contains private key)"
            );
        }
    }

    #[test]
    fn publish_writes_only_secret_yaml() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let sink = FilesystemSink::new(tmp.path().to_path_buf(), Layout::PerDomain);
        sink.publish(
            "example.test",
            &CertBundle {
                cert_pem: b"-----BEGIN CERT-----\nMII...\n-----END CERT-----\n".to_vec(),
                key_pem: b"-----BEGIN PRIVATE KEY-----\nMII...\n-----END PRIVATE KEY-----\n"
                    .to_vec(),
            },
        )
        .expect("publish");

        let mut entries: Vec<_> = std::fs::read_dir(tmp.path())
            .expect("read_dir")
            .filter_map(|e| e.ok().map(|e| e.file_name().into_string().unwrap()))
            .collect();
        entries.sort();
        assert_eq!(
            entries,
            vec!["example.test.secret.yaml".to_string()],
            "publish must write exactly one file (got {entries:?})"
        );
    }

    #[test]
    fn sds_resource_name_derivation() {
        assert_eq!(
            FilesystemSink::sds_resource_name("example.test"),
            "example_test_tls"
        );
        assert_eq!(
            FilesystemSink::sds_resource_name("my-domain.example.com"),
            "my_domain_example_com_tls"
        );
    }
}
