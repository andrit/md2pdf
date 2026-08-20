//! `SourceSet` discovery; records the `SourceRoot` that `mirror` resolves against.

use std::fs;
use std::path::{Path, PathBuf};

use crate::broker::PathError;

/// Extensions that make a file a Source. Nothing else is markdown.
const SOURCE_EXTENSIONS: &[&str] = &["md", "markdown"];

/// The Sources selected for one Job, and the root they were walked from.
///
/// The root is carried rather than recomputed because `mirror::output_path` needs it,
/// and deriving it a second time elsewhere would be a second source of truth for the
/// thing that decides where every output lands.
///
/// Lives here rather than in `md2pdf-domain` for the same reason `ImageManifest` lives
/// in `md2pdf-convert`: nothing *below* this crate needs the type, and widening the
/// shared vocabulary for a single consumer is the wrong trade.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SourceSet {
    /// The directory the walk started from. Every OutputPath mirrors a Source's path
    /// relative to this.
    pub root: PathBuf,
    /// Sources in a deterministic order — sorted by path.
    pub sources: Vec<PathBuf>,
}

impl SourceSet {
    pub fn len(&self) -> usize {
        self.sources.len()
    }
    pub fn is_empty(&self) -> bool {
        self.sources.is_empty()
    }
}

/// Walk `root` recursively, collecting every Source.
///
/// Rules, each chosen so the walk is predictable rather than clever:
///
/// - **`.md` and `.markdown` only**, case-insensitively. Nothing else is a Source.
/// - **Hidden entries are skipped** — anything whose name starts with `.`. Descending
///   into `.git` would be slow and pointless.
/// - **Symlinks are not followed.** A cycle would hang the walk, and a hang is a worse
///   failure than a missing file because nothing reports it.
/// - **Order is sorted.** Non-deterministic order would make batch output vary run to
///   run, which undermines INV-7 in spirit and makes tests flaky for no reason.
/// - **An unreadable directory fails the walk** rather than being skipped. Silently
///   converting fewer files than the user has is the failure mode that looks like
///   success.
pub(crate) fn walk(root: &Path) -> Result<SourceSet, PathError> {
    if !root.is_dir() {
        return Err(PathError::Read {
            path: root.to_path_buf(),
            source: std::io::Error::new(
                std::io::ErrorKind::NotADirectory,
                "a SourceSet is walked from a directory; convert a single file with ConvertSource",
            ),
        });
    }

    let mut sources = Vec::new();
    collect(root, &mut sources)?;
    sources.sort();

    Ok(SourceSet {
        root: root.to_path_buf(),
        sources,
    })
}

fn collect(dir: &Path, out: &mut Vec<PathBuf>) -> Result<(), PathError> {
    let entries = fs::read_dir(dir).map_err(|source| PathError::Read {
        path: dir.to_path_buf(),
        source,
    })?;

    for entry in entries {
        let entry = entry.map_err(|source| PathError::Read {
            path: dir.to_path_buf(),
            source,
        })?;
        let path = entry.path();

        if is_hidden(&path) {
            continue;
        }

        // `symlink_metadata` does not follow the link, which is the point: a symlinked
        // directory pointing at an ancestor would otherwise loop forever.
        let meta = fs::symlink_metadata(&path).map_err(|source| PathError::Read {
            path: path.clone(),
            source,
        })?;
        if meta.file_type().is_symlink() {
            continue;
        }

        if meta.is_dir() {
            collect(&path, out)?;
        } else if is_source(&path) {
            out.push(path);
        }
    }
    Ok(())
}

fn is_hidden(path: &Path) -> bool {
    path.file_name()
        .and_then(|n| n.to_str())
        .is_some_and(|n| n.starts_with('.'))
}

fn is_source(path: &Path) -> bool {
    path.extension()
        .and_then(|e| e.to_str())
        .map(|e| e.to_ascii_lowercase())
        .is_some_and(|e| SOURCE_EXTENSIONS.contains(&e.as_str()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::testing::TempDir;

    fn names(set: &SourceSet) -> Vec<String> {
        set.sources
            .iter()
            .map(|p| {
                p.strip_prefix(&set.root)
                    .unwrap_or(p)
                    .to_string_lossy()
                    .replace('\\', "/")
            })
            .collect()
    }

    #[test]
    fn finds_markdown_recursively_in_sorted_order() {
        let tmp = TempDir::new("walk");
        tmp.write("b.md", b"");
        tmp.write("a.md", b"");
        tmp.write("sub/deep/c.md", b"");
        tmp.write("sub/d.markdown", b"");

        let set = walk(tmp.path()).expect("walk");
        assert_eq!(
            names(&set),
            vec!["a.md", "b.md", "sub/d.markdown", "sub/deep/c.md"]
        );
        assert_eq!(set.root, tmp.path());
    }

    #[test]
    fn ignores_everything_that_is_not_markdown() {
        let tmp = TempDir::new("walkext");
        tmp.write("keep.md", b"");
        tmp.write("skip.txt", b"");
        tmp.write("skip.pdf", b"");
        tmp.write("skip.png", b"");
        tmp.write("noext", b"");

        assert_eq!(names(&walk(tmp.path()).expect("walk")), vec!["keep.md"]);
    }

    #[test]
    fn extensions_are_case_insensitive() {
        let tmp = TempDir::new("walkcase");
        tmp.write("SHOUT.MD", b"");
        tmp.write("Mixed.Markdown", b"");

        assert_eq!(walk(tmp.path()).expect("walk").len(), 2);
    }

    #[test]
    fn hidden_files_and_directories_are_skipped() {
        // Descending into `.git` would be slow and pointless, and a dotfile is not a
        // document the user meant to convert.
        let tmp = TempDir::new("walkhidden");
        tmp.write("visible.md", b"");
        tmp.write(".hidden.md", b"");
        tmp.write(".git/objects/thing.md", b"");

        assert_eq!(names(&walk(tmp.path()).expect("walk")), vec!["visible.md"]);
    }

    #[test]
    fn an_empty_directory_is_an_empty_set_not_an_error() {
        let tmp = TempDir::new("walkempty");
        let set = walk(tmp.path()).expect("walk");
        assert!(set.is_empty());
        assert_eq!(set.root, tmp.path());
    }

    #[test]
    fn walking_a_file_is_refused_with_a_useful_message() {
        let tmp = TempDir::new("walkfile");
        let file = tmp.write("notes.md", b"");
        let err = walk(&file).expect_err("a file is not a SourceSet");
        assert!(
            err.to_string().contains("ConvertSource"),
            "the error should point at the right command: {err}"
        );
    }

    #[test]
    fn a_missing_directory_names_itself() {
        let tmp = TempDir::new("walkmissing");
        let err = walk(&tmp.join("nope")).expect_err("must fail");
        assert!(err.to_string().contains("nope"), "{err}");
    }

    #[test]
    fn the_order_is_stable_across_walks() {
        // Not cosmetic: unsorted order comes from the filesystem and would make batch
        // output vary between runs on the same input.
        let tmp = TempDir::new("walkstable");
        for name in ["m.md", "a.md", "z.md", "sub/b.md", "sub/a.md"] {
            tmp.write(name, b"");
        }
        let first = walk(tmp.path()).expect("walk");
        let again = walk(tmp.path()).expect("walk");
        assert_eq!(first, again);
    }
}

#[cfg(all(test, unix))]
mod symlink_tests {
    use super::*;
    use crate::testing::TempDir;

    #[test]
    fn symlinked_directories_are_not_followed() {
        // A link pointing at an ancestor would otherwise recurse forever, and a hang is
        // worse than a missing file because nothing reports it.
        let tmp = TempDir::new("walksym");
        tmp.write("real.md", b"");
        tmp.write("sub/inner.md", b"");
        std::os::unix::fs::symlink(tmp.path(), tmp.join("loop")).expect("symlink");

        let set = walk(tmp.path()).expect("walk must terminate");
        assert_eq!(set.len(), 2, "followed the link: {:?}", set.sources);
    }

    #[test]
    fn symlinked_files_are_skipped_too() {
        let tmp = TempDir::new("walksymfile");
        let real = tmp.write("real.md", b"");
        std::os::unix::fs::symlink(&real, tmp.join("alias.md")).expect("symlink");

        let set = walk(tmp.path()).expect("walk");
        assert_eq!(set.len(), 1, "a symlink produced a duplicate Source");
    }
}
