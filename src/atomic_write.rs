use std::fs::{self, File};
use std::io::{self, Write};
use std::path::Path;

use tempfile::NamedTempFile;

pub(crate) fn write_atomic(path: &Path, bytes: &[u8], private: bool) -> io::Result<()> {
    let parent = path
        .parent()
        .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidInput, "missing parent directory"))?;

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

    tmp.persist(path).map(|_| ()).map_err(io::Error::from)?;

    let dir = File::open(parent)?;
    dir.sync_all()?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::panic::catch_unwind;

    use super::*;

    #[test]
    fn overwrites_existing_file_atomically() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let path = tmp.path().join("state").join("cert.pem");

        write_atomic(&path, b"old", false).expect("write old");
        write_atomic(&path, b"new", false).expect("write new");

        assert_eq!(fs::read(&path).expect("read"), b"new");
    }

    #[test]
    fn panic_before_persist_keeps_original_file() {
        struct PanicBeforePersist {
            tmp: Option<NamedTempFile>,
            committed: bool,
        }

        impl PanicBeforePersist {
            fn new(parent: &Path, bytes: &[u8]) -> io::Result<Self> {
                let mut tmp = NamedTempFile::new_in(parent)?;
                tmp.write_all(bytes)?;
                tmp.as_file_mut().sync_all()?;
                Ok(Self {
                    tmp: Some(tmp),
                    committed: false,
                })
            }
        }

        impl Drop for PanicBeforePersist {
            fn drop(&mut self) {
                if !self.committed {
                    drop(self.tmp.take());
                    panic!("simulated panic before persist");
                }
            }
        }

        let tmp = tempfile::tempdir().expect("tempdir");
        let path = tmp.path().join("state").join("cert.pem");

        write_atomic(&path, b"old", false).expect("write old");

        let parent = path.parent().expect("parent");
        let result = catch_unwind(|| {
            let _tmp = PanicBeforePersist::new(parent, b"new").expect("temp write");
        });

        assert!(result.is_err(), "panic should be caught");
        assert_eq!(fs::read(&path).expect("read"), b"old");
    }

    #[test]
    fn creates_missing_parent_directories() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let path = tmp.path().join("nested").join("state").join("backoff.json");

        write_atomic(&path, br#"{"attempts":1}"#, false).expect("write");

        assert!(path.exists(), "file should exist");
    }

    #[cfg(unix)]
    #[test]
    fn writes_private_files_with_0600_permissions() {
        use std::os::unix::fs::PermissionsExt;

        let tmp = tempfile::tempdir().expect("tempdir");
        let path = tmp.path().join("state").join("account.json");

        write_atomic(&path, b"secret", true).expect("write");

        let mode = fs::metadata(&path).expect("metadata").permissions().mode() & 0o777;
        assert_eq!(mode, 0o600);
    }
}
