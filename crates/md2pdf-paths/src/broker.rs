//! Open / read / write / exists — every filesystem call in the app.
//!
//! Nothing outside this crate calls `std::fs`, and `scripts/check-boundaries.sh`
//! enforces it. Until now that guard has been watching a crate with no filesystem
//! code in it at all; this is the module it was written for.

use std::fs;
use std::path::{Path, PathBuf};

/// Every way touching the filesystem can fail, each naming the path.
///
/// A batch converts fifty files; an error that does not say *which* one is nearly
/// useless, so the path is carried in every variant rather than logged separately.
#[derive(Debug, thiserror::Error)]
pub enum PathError {
    #[error("{path} could not be read: {source}")]
    Read {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
    #[error("{path} could not be written: {source}")]
    Write {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
    #[error("{path} already exists")]
    AlreadyExists { path: PathBuf },
    #[error("{path} is not valid UTF-8")]
    NotUtf8 { path: PathBuf },
}

/// The one door to the filesystem.
///
/// A struct rather than a trait: there is exactly one implementation, and the seam
/// that actually matters is *inside* it. If a sandboxed build ever needs
/// security-scoped bookmarks, this grows a table of resolved handles and every call
/// site stays as it is — which is the whole reason the crate exists.
///
/// Constructed rather than used as a unit value for the same reason: adding that state
/// later must not change a single caller.
#[derive(Debug, Default)]
pub struct PathBroker {
    _private: (),
}

impl PathBroker {
    pub fn new() -> Self {
        Self::default()
    }

    /// Read a Source as text.
    ///
    /// Invalid UTF-8 is an **error**, not a lossy conversion. A markdown file in some
    /// other encoding would silently convert into mojibake that looks like a
    /// successful run, and a converter that quietly corrupts its input is worse than
    /// one that refuses it.
    pub fn read_to_string(&self, path: &Path) -> Result<String, PathError> {
        let bytes = self.read_bytes(path)?;
        String::from_utf8(bytes).map_err(|_| PathError::NotUtf8 {
            path: path.to_path_buf(),
        })
    }

    pub fn read_bytes(&self, path: &Path) -> Result<Vec<u8>, PathError> {
        fs::read(path).map_err(|source| PathError::Read {
            path: path.to_path_buf(),
            source,
        })
    }

    /// Write, refusing to replace anything that is already there.
    ///
    /// **The safe path is the default path.** Overwriting requires calling
    /// [`PathBroker::overwrite`] by name, so destroying a file is always a visible
    /// decision at the call site rather than a consequence of the ordinary verb.
    /// Output is never silently overwritten — that is a product decision, and this is
    /// where it is actually enforced.
    pub fn write_new(&self, path: &Path, bytes: &[u8]) -> Result<(), PathError> {
        if self.exists(path) {
            return Err(PathError::AlreadyExists {
                path: path.to_path_buf(),
            });
        }
        self.write_inner(path, bytes)
    }

    /// Write, replacing whatever is there.
    ///
    /// Only correct once the user has been asked — the Resolution a collision prompt
    /// produced. Named so that a call site reads as the decision it is.
    pub fn overwrite(&self, path: &Path, bytes: &[u8]) -> Result<(), PathError> {
        self.write_inner(path, bytes)
    }

    /// True only for a file that exists and is actually a file.
    ///
    /// A directory named `diagram.png` exists and cannot be read as an image, so
    /// plain existence would be the wrong question to answer.
    pub fn exists(&self, path: &Path) -> bool {
        path.is_file()
    }

    /// Parent directories are created as needed.
    ///
    /// Batch output mirrors the source tree, so writing `dest/sub/dir/x.pdf` into a
    /// destination that has no `sub/dir` yet is the ordinary case, not an exception.
    /// Refusing would only push the same `create_dir_all` up into the engine, where it
    /// would be filesystem code outside this crate.
    fn write_inner(&self, path: &Path, bytes: &[u8]) -> Result<(), PathError> {
        if let Some(parent) = path.parent() {
            if !parent.as_os_str().is_empty() {
                fs::create_dir_all(parent).map_err(|source| PathError::Write {
                    path: path.to_path_buf(),
                    source,
                })?;
            }
        }
        fs::write(path, bytes).map_err(|source| PathError::Write {
            path: path.to_path_buf(),
            source,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A directory of our own, removed on drop even if the test panics.
    struct TempDir(PathBuf);

    impl TempDir {
        fn new(tag: &str) -> Self {
            let unique = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .expect("clock")
                .as_nanos();
            let dir = std::env::temp_dir().join(format!("md2pdf-{tag}-{unique}"));
            fs::create_dir_all(&dir).expect("temp dir");
            Self(dir)
        }
        fn path(&self, name: &str) -> PathBuf {
            self.0.join(name)
        }
    }

    impl Drop for TempDir {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    #[test]
    fn writes_and_reads_back() {
        let tmp = TempDir::new("rw");
        let broker = PathBroker::new();
        let file = tmp.path("notes.md");

        broker.write_new(&file, b"# Title\n").expect("write");
        assert_eq!(broker.read_to_string(&file).expect("read"), "# Title\n");
        assert_eq!(broker.read_bytes(&file).expect("read"), b"# Title\n");
    }

    #[test]
    fn write_new_refuses_to_replace() {
        // The product guarantee: output is never silently overwritten.
        let tmp = TempDir::new("norep");
        let broker = PathBroker::new();
        let file = tmp.path("out.pdf");

        broker.write_new(&file, b"first").expect("write");
        let err = broker.write_new(&file, b"second").expect_err("must refuse");
        assert!(matches!(err, PathError::AlreadyExists { .. }));
        assert_eq!(
            broker.read_bytes(&file).expect("read"),
            b"first",
            "the original was modified despite the refusal"
        );
    }

    #[test]
    fn overwrite_replaces_but_must_be_asked_for_by_name() {
        let tmp = TempDir::new("over");
        let broker = PathBroker::new();
        let file = tmp.path("out.pdf");

        broker.write_new(&file, b"first").expect("write");
        broker.overwrite(&file, b"second").expect("overwrite");
        assert_eq!(broker.read_bytes(&file).expect("read"), b"second");
    }

    #[test]
    fn parent_directories_are_created() {
        // Batch mirrors the source tree, so writing into a directory that does not
        // exist yet is ordinary.
        let tmp = TempDir::new("parents");
        let broker = PathBroker::new();
        let nested = tmp.path("sub/deeper/out.pdf");

        broker.write_new(&nested, b"pdf").expect("write");
        assert!(broker.exists(&nested));
    }

    #[test]
    fn a_missing_file_reports_which_file() {
        let tmp = TempDir::new("missing");
        let broker = PathBroker::new();
        let file = tmp.path("nope.md");

        let err = broker.read_to_string(&file).expect_err("must fail");
        assert!(
            err.to_string().contains("nope.md"),
            "error does not name the file: {err}"
        );
    }

    #[test]
    fn invalid_utf8_is_refused_rather_than_mangled() {
        let tmp = TempDir::new("utf8");
        let broker = PathBroker::new();
        let file = tmp.path("latin1.md");

        // 0xFF is not valid UTF-8 in any position.
        broker
            .write_new(&file, &[b'#', b' ', 0xFF, b'\n'])
            .expect("write");
        let err = broker.read_to_string(&file).expect_err("must refuse");
        assert!(matches!(err, PathError::NotUtf8 { .. }));
        // The bytes are still readable — only the text interpretation is refused.
        assert!(broker.read_bytes(&file).is_ok());
    }

    #[test]
    fn a_directory_is_not_a_file() {
        // `exists` answers "can I read this as a file?", not "is there something here?"
        let tmp = TempDir::new("dir");
        let broker = PathBroker::new();
        let dir = tmp.path("diagram.png");
        fs::create_dir_all(&dir).expect("dir");

        assert!(!broker.exists(&dir), "a directory was reported as a file");
    }
}
