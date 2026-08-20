//! `OutputPath` = Destination + (Source relative to `SourceRoot`).
//!
//! Batch output mirrors the source subfolder structure rather than flattening it
//! (`INV-12`). Deciding *where* output goes is path arithmetic, so it lives with the
//! crate that owns paths rather than in the engine (`INV-9`).

use std::path::{Path, PathBuf};

/// Where the PDF for `source` should be written.
///
/// `source_root` is the directory a batch was walked from. Pass `None` for a single
/// Source with no tree behind it, and the output lands directly in `destination`.
///
/// With a root, the Source's path *relative to that root* is preserved beneath the
/// Destination — so `docs/api/ref.md` walked from `docs/` becomes
/// `out/api/ref.pdf`, never `out/ref.pdf`. Flattening would silently collide two
/// files with the same name in different folders, which is the whole reason the
/// design forbids it.
pub fn output_path(destination: &Path, source: &Path, source_root: Option<&Path>) -> PathBuf {
    let relative = source_root
        .and_then(|root| source.strip_prefix(root).ok())
        .unwrap_or_else(|| Path::new(source.file_name().unwrap_or(source.as_os_str())));

    destination.join(relative).with_extension("pdf")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_single_source_lands_directly_in_the_destination() {
        assert_eq!(
            output_path(Path::new("/out"), Path::new("/docs/notes.md"), None),
            PathBuf::from("/out/notes.pdf")
        );
    }

    #[test]
    fn the_source_tree_is_mirrored_not_flattened() {
        // INV-12. Flattening would put both of these at /out/ref.pdf.
        assert_eq!(
            output_path(
                Path::new("/out"),
                Path::new("/docs/api/ref.md"),
                Some(Path::new("/docs"))
            ),
            PathBuf::from("/out/api/ref.pdf")
        );
        assert_eq!(
            output_path(
                Path::new("/out"),
                Path::new("/docs/guide/ref.md"),
                Some(Path::new("/docs"))
            ),
            PathBuf::from("/out/guide/ref.pdf")
        );
    }

    #[test]
    fn a_source_outside_the_root_falls_back_to_its_filename() {
        // Rather than producing a path with `..` in it, or panicking.
        assert_eq!(
            output_path(
                Path::new("/out"),
                Path::new("/elsewhere/stray.md"),
                Some(Path::new("/docs"))
            ),
            PathBuf::from("/out/stray.pdf")
        );
    }

    #[test]
    fn only_the_final_extension_is_replaced() {
        assert_eq!(
            output_path(Path::new("/out"), Path::new("/docs/notes.tar.md"), None),
            PathBuf::from("/out/notes.tar.pdf")
        );
    }

    #[test]
    fn a_source_without_an_extension_still_gets_one() {
        assert_eq!(
            output_path(Path::new("/out"), Path::new("/docs/README"), None),
            PathBuf::from("/out/README.pdf")
        );
    }
}
