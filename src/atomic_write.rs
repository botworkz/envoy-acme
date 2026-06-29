//! Atomic, durable file writes for state under `state_dir`.
//!
//! # Contract
//!
//! `write_atomic(path, bytes, private)` guarantees that, on POSIX
//! filesystems:
//!
//! 1. **Atomic replacement** — the target file is either fully replaced with
//!    `bytes` or untouched. There is no observable in-between state.
//! 2. **Crash durability** — once the function returns `Ok(())`, the new
//!    contents and the directory entry pointing at them have both been
//!    flushed to disk. A power loss or kernel crash immediately after the
//!    call cannot lose the data.
//! 3. **Cleanup on failure** — if any step before the rename fails (or the
//!    process panics partway through), the temp file is removed by
//!    `tempfile::NamedTempFile`'s own `Drop` impl. No partial state and no
//!    stray temp files are left behind.
//! 4. **Optional `0600` on Unix** — when `private == true`, the file mode
//!    is set to `0o600` before the rename, so the target is never visible
//!    to other users at any wider permission.
//!
//! These guarantees rely on:
//!
//! - The temp file being created in the **same directory** as the target,
//!   so the `rename(2)` is atomic on the same filesystem.
//! - `sync_all` on the temp file before rename (durability of contents).
//! - `sync_all` on the parent directory after rename (durability of the
//!   rename itself).
//!
//! The only step that touches `path` is `tmp.persist(path)`; anything that
//! goes wrong before that point leaves `path` exactly as it was.

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

    /// Demonstrates the cleanup-on-failure half of the contract.
    ///
    /// `write_atomic` only touches the target via `tmp.persist(path)`; if
    /// the process panics before that step (or any earlier step returns
    /// `Err`), the target file must keep its previous content and the
    /// temp file must be removed by `NamedTempFile`'s `Drop`.
    ///
    /// We can't easily inject a panic *inside* `write_atomic` itself
    /// without a feature-gated hook, so this test reproduces the
    /// relevant sequence inline — create + write + sync a temp file in
    /// the target's parent directory, then panic — and asserts both
    /// invariants that `write_atomic` relies on:
    ///
    /// - The target file is unchanged.
    /// - No stray temp file is left in the directory.
    #[test]
    fn panic_before_persist_leaves_target_and_directory_clean() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let path = tmp.path().join("cert.pem");
        write_atomic(&path, b"old", false).expect("write old");

        let parent = path.parent().expect("parent").to_path_buf();
        let result = catch_unwind(move || {
            let mut t = NamedTempFile::new_in(&parent).expect("temp create");
            t.write_all(b"new").expect("temp write");
            t.as_file_mut().sync_all().expect("temp sync");
            panic!("simulated panic before persist");
        });
        assert!(result.is_err(), "panic should be caught");

        // Target is unchanged.
        assert_eq!(fs::read(&path).expect("read"), b"old");

        // No stray temp files left behind by NamedTempFile's Drop.
        let leftover: Vec<_> = fs::read_dir(tmp.path())
            .expect("read_dir")
            .filter_map(|e| e.ok())
            .filter(|e| e.file_name() != "cert.pem")
            .map(|e| e.file_name())
            .collect();
        assert!(
            leftover.is_empty(),
            "stray files in state dir: {leftover:?}"
        );
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
