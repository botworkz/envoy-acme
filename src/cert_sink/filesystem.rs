use std::path::PathBuf;

use serde::Serialize;

use crate::cert_sink::{CertBundle, CertSink};
use crate::config::Layout;
use crate::errors::SinkError;

pub struct FilesystemSink {
    dir: PathBuf,
    layout: Layout,
}

// Internal types for serialising the Envoy SDS path-config-source secret file.
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
    certificate_chain: FileDataSource<'a>,
    private_key: FileDataSource<'a>,
}

#[derive(Serialize)]
struct FileDataSource<'a> {
    filename: &'a str,
}

impl FilesystemSink {
    pub fn new(dir: PathBuf, layout: Layout) -> Self {
        Self { dir, layout }
    }

    fn cert_path(&self, name: &str) -> PathBuf {
        match self.layout {
            Layout::PerDomain => self.dir.join(format!("{name}.cert.pem")),
        }
    }

    fn key_path(&self, name: &str) -> PathBuf {
        match self.layout {
            Layout::PerDomain => self.dir.join(format!("{name}.key.pem")),
        }
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

    /// Build the Envoy SDS secret YAML for this domain and write it atomically.
    ///
    /// The secret file is written *after* cert+key are durable so that Envoy
    /// never reloads partial state.
    fn write_sds_secret(&self, name: &str) -> Result<(), SinkError> {
        let cert_path = self.cert_path(name);
        let key_path = self.key_path(name);
        let secret_path = self.secret_path(name);

        let cert_filename = cert_path
            .to_str()
            .ok_or_else(|| std::io::Error::other("cert path is not valid UTF-8"))?;
        let key_filename = key_path
            .to_str()
            .ok_or_else(|| std::io::Error::other("key path is not valid UTF-8"))?;
        let resource_name = Self::sds_resource_name(name);

        let payload = SdsFile {
            resources: vec![SdsSecret {
                type_url: "type.googleapis.com/envoy.extensions.transport_sockets.tls.v3.Secret",
                name: &resource_name,
                tls_certificate: TlsCertificate {
                    certificate_chain: FileDataSource {
                        filename: cert_filename,
                    },
                    private_key: FileDataSource {
                        filename: key_filename,
                    },
                },
            }],
        };

        let yaml = serde_yaml::to_string(&payload).map_err(std::io::Error::other)?;

        crate::atomic_write::write_atomic(&secret_path, yaml.as_bytes(), false)?;
        Ok(())
    }
}

impl CertSink for FilesystemSink {
    fn publish(&self, name: &str, bundle: &CertBundle) -> Result<(), SinkError> {
        // Write cert and key first so they are durable before the SDS file.
        crate::atomic_write::write_atomic(&self.cert_path(name), &bundle.cert_pem, false)?;
        crate::atomic_write::write_atomic(&self.key_path(name), &bundle.key_pem, true)?;
        // Write the Envoy SDS secret file last: Envoy reloads on this path so
        // cert+key must already be in place.
        self.write_sds_secret(name)?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn writes_expected_layout() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let sink = FilesystemSink::new(tmp.path().to_path_buf(), Layout::PerDomain);
        let bundle = CertBundle {
            cert_pem: b"cert".to_vec(),
            key_pem: b"key".to_vec(),
        };

        sink.publish("example.test", &bundle).expect("publish");

        let cert = std::fs::read(tmp.path().join("example.test.cert.pem")).expect("cert read");
        let key = std::fs::read(tmp.path().join("example.test.key.pem")).expect("key read");
        assert_eq!(cert, b"cert");
        assert_eq!(key, b"key");
    }

    #[test]
    fn writes_sds_secret_yaml() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let sink = FilesystemSink::new(tmp.path().to_path_buf(), Layout::PerDomain);
        let bundle = CertBundle {
            cert_pem: b"cert".to_vec(),
            key_pem: b"key".to_vec(),
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
        // Cert filename present.
        assert!(
            raw.contains("example.test.cert.pem"),
            "secret yaml missing cert filename"
        );
        // Key filename present.
        assert!(
            raw.contains("example.test.key.pem"),
            "secret yaml missing key filename"
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
