//! Workspace-level mutual exclusion.
//!
//! Two `outl` processes opening the same workspace would race on
//! writes to `log.db` and the `.md` files. An exclusive flock on
//! `.outl/.lock` prevents that: the second process gets a clean
//! [`LockError::AlreadyHeld`] and can prompt the user, instead of
//! corrupting state.
//!
//! The lock is *advisory* (POSIX flock semantics, Windows LockFileEx).
//! It only protects against well-behaved `outl` processes — a `rm
//! .outl/.lock` won't break anything, just makes a second `outl`
//! think the workspace is free.

use fs2::FileExt;
use std::fs::{self, File};
use std::path::{Path, PathBuf};
use thiserror::Error;

/// Failure modes when acquiring the lock.
#[derive(Debug, Error)]
pub enum LockError {
    /// Another `outl` process already holds this workspace.
    #[error("workspace is already open in another outl process: {0}")]
    AlreadyHeld(PathBuf),
    /// Filesystem error creating or opening the lock file.
    #[error("io error on {path}: {source}")]
    Io {
        /// Path of the lock file.
        path: PathBuf,
        /// Underlying I/O error.
        #[source]
        source: std::io::Error,
    },
}

/// Holds an exclusive lock on `<workspace>/.outl/.lock`.
///
/// Drop releases the lock automatically. Don't try to release manually
/// — `Drop` is the API.
#[must_use = "the lock is released when the value is dropped; bind it to a variable"]
#[derive(Debug)]
pub struct WorkspaceLock {
    file: File,
    /// Path of the lock file. Kept for diagnostics; not stripped on drop
    /// because other processes may legitimately reuse the file.
    path: PathBuf,
}

impl WorkspaceLock {
    /// Try to acquire the workspace lock. Returns immediately —
    /// blocking semantics would deadlock the TUI on a stale lock.
    pub fn acquire(workspace_root: &Path) -> Result<Self, LockError> {
        let dir = workspace_root.join(".outl");
        if !dir.exists() {
            fs::create_dir_all(&dir).map_err(|e| LockError::Io {
                path: dir.clone(),
                source: e,
            })?;
        }
        let lock_path = dir.join(".lock");
        let file = std::fs::OpenOptions::new()
            .create(true)
            .read(true)
            .write(true)
            .truncate(false)
            .open(&lock_path)
            .map_err(|e| LockError::Io {
                path: lock_path.clone(),
                source: e,
            })?;
        // Try-lock — return immediately if held by another process.
        if file.try_lock_exclusive().is_err() {
            return Err(LockError::AlreadyHeld(lock_path));
        }
        Ok(Self {
            file,
            path: lock_path,
        })
    }

    /// Path of the lock file (for diagnostics).
    pub fn path(&self) -> &Path {
        &self.path
    }
}

impl Drop for WorkspaceLock {
    fn drop(&mut self) {
        let _ = FileExt::unlock(&self.file);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn acquire_creates_lock_file() {
        let dir = TempDir::new().unwrap();
        let lock = WorkspaceLock::acquire(dir.path()).unwrap();
        assert!(lock.path().exists());
    }

    #[test]
    fn second_acquire_fails_while_first_held() {
        let dir = TempDir::new().unwrap();
        let _first = WorkspaceLock::acquire(dir.path()).unwrap();
        match WorkspaceLock::acquire(dir.path()) {
            Err(LockError::AlreadyHeld(_)) => {}
            other => panic!("expected AlreadyHeld, got {other:?}"),
        }
    }

    #[test]
    fn drop_releases_lock() {
        let dir = TempDir::new().unwrap();
        {
            let _lock = WorkspaceLock::acquire(dir.path()).unwrap();
        }
        // Should succeed now.
        let _second = WorkspaceLock::acquire(dir.path()).unwrap();
    }

    #[test]
    fn acquire_creates_missing_outl_dir() {
        let dir = TempDir::new().unwrap();
        // Don't pre-create .outl/.
        let lock = WorkspaceLock::acquire(dir.path()).unwrap();
        assert!(lock.path().parent().unwrap().is_dir());
    }
}
