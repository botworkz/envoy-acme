use std::fs::{File, OpenOptions};
use std::io::Write;
use std::path::Path;

#[derive(Debug)]
pub(crate) struct StateLock {
    // Keep the File alive; dropping it releases the flock.
    _file: File,
}

impl StateLock {
    /// Acquire an exclusive non-blocking flock on `state_dir/.lock`.
    /// Writes `pid={pid}\nstarted_at={unix_ts}\n` into the lockfile.
    /// On contention, returns an Err whose message includes the
    /// existing lockfile contents (best-effort).
    pub(crate) fn acquire(state_dir: &Path) -> std::io::Result<Self> {
        std::fs::create_dir_all(state_dir)?;
        let lock_path = state_dir.join(".lock");

        let file = OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .truncate(false)
            .open(&lock_path)?;

        // rustix::fs::flock with LOCK_EX | LOCK_NB
        use rustix::fs::{flock, FlockOperation};
        flock(&file, FlockOperation::NonBlockingLockExclusive).map_err(|errno| {
            let existing = std::fs::read_to_string(&lock_path)
                .unwrap_or_else(|_| "<unreadable>".to_string());
            std::io::Error::new(
                std::io::ErrorKind::WouldBlock,
                format!(
                    "state_dir {state_dir:?} is locked by another process; existing lockfile contents: {existing}; errno: {errno}",
                ),
            )
        })?;

        // Lock acquired. Write our identity. Truncate first so a stale longer
        // string from a previous holder doesn't get partially overwritten.
        let mut file_w = &file;
        file_w.set_len(0)?;
        let ts = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0);
        writeln!(file_w, "pid={}", std::process::id())?;
        writeln!(file_w, "started_at={ts}")?;
        file_w.sync_all()?;

        Ok(Self { _file: file })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn acquire_succeeds_on_fresh_dir() {
        let dir = tempdir().unwrap();
        let lock = StateLock::acquire(dir.path());
        assert!(lock.is_ok(), "expected Ok, got {lock:?}");

        let contents = std::fs::read_to_string(dir.path().join(".lock")).unwrap();
        assert!(
            contents.contains("pid="),
            "lockfile missing pid=: {contents:?}"
        );
        assert!(
            contents.contains("started_at="),
            "lockfile missing started_at=: {contents:?}"
        );
    }

    #[test]
    fn second_acquire_fails_while_first_held() {
        let dir = tempdir().unwrap();
        let _first = StateLock::acquire(dir.path()).expect("first acquire should succeed");

        let second = StateLock::acquire(dir.path());
        assert!(second.is_err(), "expected Err on second acquire");
        let err = second.unwrap_err();
        assert_eq!(err.kind(), std::io::ErrorKind::WouldBlock);
        let msg = err.to_string();
        assert!(
            msg.contains("locked by another process"),
            "error should mention lock contention: {msg:?}"
        );
        assert!(
            msg.contains(&format!("pid={}", std::process::id())),
            "error should include the holding PID: {msg:?}"
        );
    }

    #[test]
    fn acquire_succeeds_after_first_dropped() {
        let dir = tempdir().unwrap();
        {
            let _first = StateLock::acquire(dir.path()).expect("first acquire should succeed");
        }
        // _first is dropped here; flock is released.
        let second = StateLock::acquire(dir.path());
        assert!(
            second.is_ok(),
            "expected Ok after first lock dropped, got {second:?}"
        );
    }
}
