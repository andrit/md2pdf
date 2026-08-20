//! Test support that other crates borrow, so `std::fs` stays in this crate.
//!
//! The engine's tests need real directories. Writing `std::fs::create_dir_all` there
//! would fail `scripts/check-boundaries.sh`, which greps every `.rs` under `crates/`
//! except this one — **test files included**.
//!
//! The alternatives were all worse: exempting `tests/` weakens a guard that has
//! already caught a real leak, and adding `remove_dir_all` to [`PathBroker`] would be
//! product API invented to serve tests, since the app never deletes anything.
//! Lending the helper from here keeps INV-9 exactly as written.
//!
//! [`PathBroker`]: crate::PathBroker

use std::fs;
use std::path::{Path, PathBuf};

/// A directory of our own, removed on drop — even if the test panics.
pub struct TempDir(PathBuf);

impl TempDir {
    /// Create a uniquely named directory under the system temp directory.
    ///
    /// `tag` only has to make the directory recognisable if a crash ever leaves one
    /// behind; uniqueness comes from the clock and the thread id, so parallel tests
    /// never collide.
    pub fn new(tag: &str) -> Self {
        let nanos = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .expect("system clock before the epoch")
            .as_nanos();
        let thread = std::thread::current().id();
        let dir = std::env::temp_dir().join(format!("md2pdf-{tag}-{nanos}-{thread:?}"));
        fs::create_dir_all(&dir).expect("could not create temp dir");
        Self(dir)
    }

    pub fn path(&self) -> &Path {
        &self.0
    }

    /// A path inside this directory. Nothing is created.
    pub fn join(&self, name: &str) -> PathBuf {
        self.0.join(name)
    }

    /// Write a fixture, creating parents. Panics on failure — a test that cannot set
    /// itself up has nothing useful to say.
    pub fn write(&self, name: &str, bytes: &[u8]) -> PathBuf {
        let path = self.join(name);
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).expect("could not create fixture parent");
        }
        fs::write(&path, bytes).expect("could not write fixture");
        path
    }
}

impl Drop for TempDir {
    fn drop(&mut self) {
        // Best effort: a test that already failed should not fail again in cleanup.
        let _ = fs::remove_dir_all(&self.0);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn creates_and_cleans_up() {
        let path;
        {
            let tmp = TempDir::new("selftest");
            path = tmp.path().to_path_buf();
            assert!(path.is_dir());
            tmp.write("a/b/c.txt", b"hi");
            assert!(tmp.join("a/b/c.txt").is_file());
        }
        assert!(!path.exists(), "temp dir survived its owner");
    }

    #[test]
    fn two_dirs_never_collide() {
        let a = TempDir::new("collide");
        let b = TempDir::new("collide");
        assert_ne!(a.path(), b.path());
    }
}
