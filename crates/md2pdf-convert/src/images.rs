//! Image path resolution, relative to the source `.md`, and remote-image policy.
//!
//! Paths are *resolved* here but never *read* — reading belongs to `md2pdf-paths`,
//! the only crate permitted to touch the filesystem. Existence is not knowable by
//! string arithmetic, so it arrives through an injected [`ImageProbe`].
//!
//! Planned in `design/plan-images.md` §T11.

use std::path::{Component, Path, PathBuf};

use md2pdf_domain::fnv1a;

/// Formats Typst 0.15.1 can load, read from its source rather than recalled
/// (`typst-library/src/visualize/image/mod.rs:352-357`).
///
/// The extension decides the format, so an unrecognised one must never reach Typst:
/// it would fail the **whole document**, not one element.
const SUPPORTED: &[&str] = &["png", "jpg", "jpeg", "gif", "webp", "svg", "svgz", "pdf"];

/// Does this path exist? Injected, because a pure crate cannot ask the filesystem.
///
/// The real implementation lives in `md2pdf-paths`; tests use a map.
pub trait ImageProbe {
    fn exists(&self, path: &Path) -> bool;
}

/// A probe for callers with no filesystem — stdin, tests. Every image degrades to a
/// placeholder, which is exactly Stage 1 behaviour.
pub struct NoImages;

impl ImageProbe for NoImages {
    fn exists(&self, _path: &Path) -> bool {
        false
    }
}

/// What a markdown image destination resolved to.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ImageRef {
    /// Embed it. `virtual_name` goes into the markup, `absolute` into the manifest.
    Resolved {
        virtual_name: String,
        absolute: PathBuf,
    },
    /// A local path that is not there. Named, so the placeholder and the Compromise
    /// can both say *which* file.
    Missing { shown: String },
    /// Fetching would break "no network" — skipped by decision (D4).
    Remote { shown: String },
    /// A destination md2pdf cannot turn into an embeddable file.
    Unsupported { shown: String, why: &'static str },
}

/// Resolve one markdown image destination.
///
/// `source_dir` is the directory of the `.md` being converted; `None` when there is no
/// file behind the markdown, in which case every relative path is `Missing`.
pub fn resolve(dest: &str, source_dir: Option<&Path>, probe: &dyn ImageProbe) -> ImageRef {
    let shown = dest.trim().to_string();

    if shown.is_empty() {
        return ImageRef::Unsupported {
            shown,
            why: "empty image destination",
        };
    }
    if is_remote(&shown) {
        return ImageRef::Remote { shown };
    }
    if shown.starts_with("data:") {
        // Solvable — decode to bytes, key the name off their hash — but the manifest
        // is path-based, so it needs a bytes-carrying variant first. See plan §T11(e).
        return ImageRef::Unsupported {
            shown,
            why: "data: URI",
        };
    }

    let cleaned = percent_decode(strip_query_and_fragment(&shown));

    let Some(ext) = extension_of(&cleaned) else {
        return ImageRef::Unsupported {
            shown,
            why: "no file extension, so Typst cannot infer the format",
        };
    };
    if !SUPPORTED.contains(&ext.as_str()) {
        return ImageRef::Unsupported {
            shown,
            why: "not an image format Typst can load",
        };
    }

    let candidate = PathBuf::from(&cleaned);
    let absolute = if candidate.is_absolute() {
        normalise(&candidate)
    } else {
        match source_dir {
            Some(dir) => normalise(&dir.join(&candidate)),
            // No Source on disk: a relative path has nothing to be relative to.
            None => return ImageRef::Missing { shown },
        }
    };

    if probe.exists(&absolute) {
        ImageRef::Resolved {
            virtual_name: virtual_name(&absolute, &ext),
            absolute,
        }
    } else {
        ImageRef::Missing { shown }
    }
}

/// The name the markup references and the manifest is keyed by.
///
/// Derived from the resolved absolute path so it is **unique** (two files sharing a
/// basename differ), **stable** across runs (an unstable name would miss `comemo`'s
/// cache on every recompile — the Override hot path), and **deduplicating** (one file
/// referenced twice is stored once).
fn virtual_name(absolute: &Path, ext: &str) -> String {
    let digest = fnv1a(absolute.to_string_lossy().as_bytes());
    format!("img-{digest:016x}.{ext}")
}

/// `scheme://…`, or protocol-relative `//host/…`.
///
/// A Windows path such as `C:\pics\x.png` contains a colon but never `://`.
fn is_remote(dest: &str) -> bool {
    if dest.starts_with("//") {
        return true;
    }
    match dest.find("://") {
        Some(i) if i > 0 => dest[..i]
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '+'),
        _ => false,
    }
}

/// `img.png?v=2#frag` → `img.png`. Meaningful on the web, never part of a filename.
fn strip_query_and_fragment(dest: &str) -> &str {
    let end = dest.find(['?', '#']).unwrap_or(dest.len());
    &dest[..end]
}

/// `my%20image.png` → `my image.png`. What an editor writes for a filename with a
/// space, and what GitHub renders. Ten lines, so no dependency.
fn percent_decode(s: &str) -> String {
    let bytes = s.as_bytes();
    let mut out: Vec<u8> = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'%' && i + 2 < bytes.len() {
            let hex = std::str::from_utf8(&bytes[i + 1..i + 3]).ok();
            if let Some(byte) = hex.and_then(|h| u8::from_str_radix(h, 16).ok()) {
                out.push(byte);
                i += 3;
                continue;
            }
        }
        out.push(bytes[i]);
        i += 1;
    }
    // Undecodable bytes mean the destination was not really percent-encoded; keeping
    // the original is friendlier than mangling it.
    String::from_utf8(out).unwrap_or_else(|_| s.to_string())
}

fn extension_of(path: &str) -> Option<String> {
    Path::new(path)
        .extension()
        .map(|e| e.to_string_lossy().to_ascii_lowercase())
}

/// Collapse `.` and `..` **lexically**.
///
/// Traversal is deliberately allowed: `![](../assets/logo.png)` is ordinary in a local
/// tool converting the user's own files, and refusing it would break real documents to
/// defend a boundary that does not exist here. Any sandbox policy belongs in
/// `md2pdf-paths`, which actually opens files. See plan §T11(a).
///
/// Lexical only — symlinks and case-insensitive filesystems cannot be collapsed
/// without I/O, so deduplication is best-effort by design.
fn normalise(path: &Path) -> PathBuf {
    let mut out = PathBuf::new();
    for component in path.components() {
        match component {
            Component::CurDir => {}
            Component::ParentDir => {
                // Only pop a real directory name; keep `..` that climbs above the
                // start, since the caller may have given a relative root.
                if out
                    .components()
                    .next_back()
                    .is_some_and(|c| matches!(c, Component::Normal(_)))
                {
                    out.pop();
                } else {
                    out.push("..");
                }
            }
            other => out.push(other.as_os_str()),
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashSet;

    /// Existence without a filesystem.
    struct StubProbe(HashSet<PathBuf>);

    impl StubProbe {
        fn with(paths: &[&str]) -> Self {
            Self(paths.iter().map(PathBuf::from).collect())
        }
    }

    impl ImageProbe for StubProbe {
        fn exists(&self, path: &Path) -> bool {
            self.0.contains(path)
        }
    }

    fn dir() -> PathBuf {
        PathBuf::from("/docs/notes")
    }

    #[test]
    fn a_relative_path_resolves_against_the_source_directory() {
        let probe = StubProbe::with(&["/docs/notes/img/x.png"]);
        match resolve("img/x.png", Some(&dir()), &probe) {
            ImageRef::Resolved {
                absolute,
                virtual_name,
            } => {
                assert_eq!(absolute, PathBuf::from("/docs/notes/img/x.png"));
                assert!(virtual_name.starts_with("img-"));
                assert!(virtual_name.ends_with(".png"));
            }
            other => panic!("expected Resolved, got {other:?}"),
        }
    }

    #[test]
    fn traversal_is_allowed_because_it_is_the_users_own_disk() {
        // `![](../assets/logo.png)` is ordinary; refusing it would break real documents.
        let probe = StubProbe::with(&["/docs/assets/logo.png"]);
        assert!(matches!(
            resolve("../assets/logo.png", Some(&dir()), &probe),
            ImageRef::Resolved { .. }
        ));
    }

    #[test]
    fn a_missing_file_is_missing_not_an_error() {
        let probe = StubProbe::with(&[]);
        assert_eq!(
            resolve("gone.png", Some(&dir()), &probe),
            ImageRef::Missing {
                shown: "gone.png".into()
            }
        );
    }

    #[test]
    fn remote_destinations_are_skipped() {
        let probe = StubProbe::with(&[]);
        for url in [
            "https://x.test/a.png",
            "http://x.test/a.png",
            "//x.test/a.png",
            "ftp://x.test/a.png",
        ] {
            assert!(
                matches!(resolve(url, Some(&dir()), &probe), ImageRef::Remote { .. }),
                "{url} was not treated as remote"
            );
        }
    }

    #[test]
    fn a_windows_path_is_not_mistaken_for_a_url() {
        let probe = StubProbe::with(&[]);
        assert!(
            !matches!(
                resolve(r"C:\pics\x.png", Some(&dir()), &probe),
                ImageRef::Remote { .. }
            ),
            "a drive letter was read as a URL scheme"
        );
    }

    #[test]
    fn unsupported_and_absent_extensions_are_refused() {
        let probe = StubProbe::with(&["/docs/notes/x.bmp", "/docs/notes/noext"]);
        // Reaching Typst with either would fail the WHOLE document.
        assert!(matches!(
            resolve("x.bmp", Some(&dir()), &probe),
            ImageRef::Unsupported { .. }
        ));
        assert!(matches!(
            resolve("noext", Some(&dir()), &probe),
            ImageRef::Unsupported { .. }
        ));
    }

    #[test]
    fn every_format_typst_supports_is_accepted() {
        let paths: Vec<String> = SUPPORTED
            .iter()
            .map(|e| format!("/docs/notes/x.{e}"))
            .collect();
        let probe = StubProbe::with(&paths.iter().map(String::as_str).collect::<Vec<_>>());
        for ext in SUPPORTED {
            assert!(
                matches!(
                    resolve(&format!("x.{ext}"), Some(&dir()), &probe),
                    ImageRef::Resolved { .. }
                ),
                "{ext} was refused"
            );
        }
    }

    #[test]
    fn extensions_are_case_insensitive() {
        let probe = StubProbe::with(&["/docs/notes/X.PNG"]);
        match resolve("X.PNG", Some(&dir()), &probe) {
            ImageRef::Resolved { virtual_name, .. } => assert!(virtual_name.ends_with(".png")),
            other => panic!("expected Resolved, got {other:?}"),
        }
    }

    #[test]
    fn data_uris_are_recorded_as_unsupported() {
        let probe = StubProbe::with(&[]);
        match resolve("data:image/png;base64,iVBOR", Some(&dir()), &probe) {
            ImageRef::Unsupported { why, .. } => assert_eq!(why, "data: URI"),
            other => panic!("expected Unsupported, got {other:?}"),
        }
    }

    #[test]
    fn percent_encoding_is_decoded() {
        let probe = StubProbe::with(&["/docs/notes/my image.png"]);
        assert!(matches!(
            resolve("my%20image.png", Some(&dir()), &probe),
            ImageRef::Resolved { .. }
        ));
    }

    #[test]
    fn query_and_fragment_are_stripped() {
        let probe = StubProbe::with(&["/docs/notes/x.png"]);
        for dest in ["x.png?v=2", "x.png#frag", "x.png?v=2#frag"] {
            assert!(
                matches!(
                    resolve(dest, Some(&dir()), &probe),
                    ImageRef::Resolved { .. }
                ),
                "{dest} did not resolve"
            );
        }
    }

    #[test]
    fn same_basename_in_two_directories_gets_different_names() {
        // The collision the naming scheme exists to prevent.
        let probe = StubProbe::with(&["/docs/notes/a/x.png", "/docs/notes/b/x.png"]);
        let (
            ImageRef::Resolved {
                virtual_name: a, ..
            },
            ImageRef::Resolved {
                virtual_name: b, ..
            },
        ) = (
            resolve("a/x.png", Some(&dir()), &probe),
            resolve("b/x.png", Some(&dir()), &probe),
        )
        else {
            panic!("both should resolve");
        };
        assert_ne!(a, b);
    }

    #[test]
    fn the_same_file_gets_the_same_name_every_time() {
        // Stability is not cosmetic: an unstable name misses `comemo`'s cache on
        // every recompile, which is the Override hot path.
        let probe = StubProbe::with(&["/docs/notes/x.png"]);
        let first = resolve("x.png", Some(&dir()), &probe);
        let again = resolve("./x.png", Some(&dir()), &probe);
        assert_eq!(
            first, again,
            "equivalent spellings produced different names"
        );
    }

    #[test]
    fn without_a_source_directory_relative_paths_are_missing() {
        let probe = StubProbe::with(&["/docs/notes/x.png"]);
        assert!(matches!(
            resolve("x.png", None, &probe),
            ImageRef::Missing { .. }
        ));
    }

    #[test]
    fn an_absolute_path_needs_no_source_directory() {
        let probe = StubProbe::with(&["/pics/x.png"]);
        assert!(matches!(
            resolve("/pics/x.png", None, &probe),
            ImageRef::Resolved { .. }
        ));
    }

    #[test]
    fn an_empty_destination_is_unsupported_not_a_panic() {
        let probe = StubProbe::with(&[]);
        assert!(matches!(
            resolve("   ", Some(&dir()), &probe),
            ImageRef::Unsupported { .. }
        ));
    }

    #[test]
    fn the_no_images_probe_degrades_everything_to_placeholders() {
        assert!(matches!(
            resolve("x.png", Some(&dir()), &NoImages),
            ImageRef::Missing { .. }
        ));
    }
}
