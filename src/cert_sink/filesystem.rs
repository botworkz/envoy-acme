use std::fs::{self, File};
use std::io::Write;
use std::path::{Path, PathBuf};

use tempfile::NamedTempFile;

use crate::cert_sink::{CertBundle, CertSink};
use crate::config::Layout;
use crate::errors::SinkError;

pub struct FilesystemSink {
    dir: PathBuf,
    layout: Layout,
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

    fn write_atomic(path: &Path, bytes: &[u8], private: bool) -> Result<(), SinkError> {
        let parent = path.parent().ok_or_else(|| {
            std::io::Error::new(std::io::ErrorKind::InvalidInput, "missing parent directory")
        })?;

        fs::create_dir_all(parent)?;

        let mut tmp = NamedTempFile::new_in(parent)?;
        tmp.write_all(bytes)?;
        tmp.as_file_mut().sync_all()?;

        #[cfg(unix)]
        if private {
            use std::os::unix::fs::PermissionsExt;
            tmp.as_file()
                .set_permissions(fs::Permissions::from_mode(0o600))?;
        }

        tmp.persist(path)?;

        let dir = File::open(parent)?;
        dir.sync_all()?;

        Ok(())
    }
}

impl CertSink for FilesystemSink {
    fn publish(&self, name: &str, bundle: &CertBundle) -> Result<(), SinkError> {
        Self::write_atomic(&self.cert_path(name), &bundle.cert_pem, false)?;
        Self::write_atomic(&self.key_path(name), &bundle.key_pem, true)?;
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
}
