//! Atomic file writes — `write-temp + rename` pattern.
//!
//! `fs::write` is two operations under the hood: truncate + write. A
//! crash between them leaves a partial file. For outl this could mean
//! a `.md` with half a page or a sidecar with a broken JSON. Either
//! way you'd need [`crate::reconcile`] to clean up.
//!
//! The fix is universal in POSIX: write to a sibling `*.tmp` then
//! `rename(tmp, final)`. `rename` is atomic — readers see either the
//! old file or the new file, never a half-written one.
//!
//! On Windows the same `std::fs::rename` works on the same volume
//! (which `.outl/` always is).

use std::fs;
use std::io;
use std::path::Path;

/// Write `contents` to `path` atomically.
///
/// Steps:
/// 1. Compute a sibling temp path: `path` with `.tmp` appended to the
///    filename (so `pages/foo.md` becomes `pages/foo.md.tmp`).
/// 2. Write the temp file fully and sync it to disk.
/// 3. `rename(tmp, path)` — atomic on a single filesystem.
///
/// If anything fails, the temp file is cleaned up so we don't leave
/// `*.tmp` litter behind.
pub fn write_atomic<P: AsRef<Path>>(path: P, contents: &[u8]) -> io::Result<()> {
    let path = path.as_ref();
    let parent = path.parent().ok_or_else(|| {
        io::Error::new(
            io::ErrorKind::InvalidInput,
            "atomic write requires a path with a parent directory",
        )
    })?;
    // Make sure the parent exists — `outl init` creates these but a
    // user moving files around could delete `pages/` and we'd hit a
    // raw IO error otherwise.
    if !parent.as_os_str().is_empty() && !parent.exists() {
        fs::create_dir_all(parent)?;
    }

    let tmp = tmp_path(path);

    // Write + fsync the temp.
    {
        let mut file = fs::OpenOptions::new()
            .create(true)
            .write(true)
            .truncate(true)
            .open(&tmp)?;
        use std::io::Write;
        file.write_all(contents)?;
        // Flush kernel buffers to disk so a power loss between write
        // and rename can't replay garbage.
        file.sync_all()?;
    }

    // Atomic swap. On error, clean up the temp.
    if let Err(e) = fs::rename(&tmp, path) {
        let _ = fs::remove_file(&tmp);
        return Err(e);
    }
    Ok(())
}

/// `path` plus a `.tmp` suffix on the filename.
fn tmp_path(path: &Path) -> std::path::PathBuf {
    let mut name = path.file_name().unwrap_or_default().to_os_string();
    name.push(".tmp");
    path.with_file_name(name)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    #[test]
    fn writes_and_renames() {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("foo.md");
        write_atomic(&path, b"hello").unwrap();
        assert_eq!(fs::read_to_string(&path).unwrap(), "hello");
    }

    #[test]
    fn no_temp_file_left_behind() {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("foo.md");
        write_atomic(&path, b"hello").unwrap();
        let tmp = dir.path().join("foo.md.tmp");
        assert!(!tmp.exists(), "temp file should be gone after rename");
    }

    #[test]
    fn overwrites_existing_file() {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("foo.md");
        fs::write(&path, "old").unwrap();
        write_atomic(&path, b"new").unwrap();
        assert_eq!(fs::read_to_string(&path).unwrap(), "new");
    }

    #[test]
    fn creates_missing_parent_dir() {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("deep").join("nested").join("x.md");
        write_atomic(&path, b"hi").unwrap();
        assert!(path.exists());
    }
}
