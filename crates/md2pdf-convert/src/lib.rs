//! Markdown -> Markup. Pure and total: text in, text out.
//!
//! Assigns the `ElementId` and `ElementClass` for every element as it emits.
//! That is what makes ids stable by construction — md2pdf generates the markup
//! rather than inferring structure back out of typst's tree.
//!
//! No I/O. Image paths are *resolved* here but never *read*; reading is
//! `md2pdf-paths`' job, and existence arrives through an injected [`ImageProbe`].
//!
//! ```
//! use md2pdf_convert::{convert, SourceContext};
//!
//! let conversion = convert("# Title\n\nSome *prose*.\n", &SourceContext::none());
//! assert_eq!(conversion.elements.len(), 2);
//! assert!(!conversion.is_flagged());
//! ```

// markdown construct -> ElementClass (Atomic vs Wrappable)
pub mod classify;
// internal events -> Typst Markup
pub mod emit;
// arbitrary document text -> text safe inside Typst markup
pub mod escape;
// relative-path resolution; remote-image policy
pub mod images;
// pulldown-cmark -> internal event stream
pub mod glyphs;
pub mod parse;

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use md2pdf_domain::{Compromise, Element, Template};

pub use images::{ImageProbe, NoImages};

/// Every image the document referenced and md2pdf resolved: **virtual name → the file
/// on disk**.
///
/// The seam between three crates that each may do only their own part — convert cannot
/// read bytes, typeset cannot touch the filesystem, and only `md2pdf-paths` may. The
/// engine walks this map, reads each file through the broker, and hands the bytes to
/// `Typesetter::add_file` under the same name the markup references.
///
/// A `BTreeMap` for two reasons: one file referenced twice collapses to one entry, and
/// iteration order is deterministic, so a batch produces the same work in the same
/// order every run.
pub type ImageManifest = BTreeMap<String, PathBuf>;

/// What the world looks like from the Source's point of view.
///
/// Conversion stays pure by taking a *description* of its surroundings rather than
/// reaching for them: the directory to resolve relative paths against, and a probe that
/// answers whether a file exists.
pub struct SourceContext<'a> {
    /// Directory of the Source. `None` when no file backs the markdown — stdin, a
    /// scratch buffer, a test.
    pub source_dir: Option<&'a Path>,
    pub images: &'a dyn ImageProbe,
    /// The Template the result will be rendered against.
    ///
    /// Conversion does not lay anything out, but it does decide *proportions* — which
    /// table columns share the leftover width, and where a long run may wrap — because
    /// that is the only place the cell text exists. Those decisions depend on how much
    /// room the page has, so the alternative to carrying the template is hardcoding a
    /// page geometry, which is what `CHARS_ACROSS = 96` silently did until 2026-08-23.
    ///
    /// Owned rather than borrowed so `none()` can exist without a static. Cloned once
    /// per Source, against a probe that costs ~150ms.
    pub template: Template,
}

impl SourceContext<'_> {
    /// No filesystem at all: every image degrades to a visible placeholder.
    ///
    /// Deliberately explicit. There is no `convert(markdown)` shortcut, because a
    /// caller that meant to pass a real context and silently got this one would see
    /// every image in the document quietly become a placeholder — a failure that looks
    /// like success.
    pub fn none() -> Self {
        Self {
            source_dir: None,
            images: &NoImages,
            template: Template::default(),
        }
    }
}

impl<'a> SourceContext<'a> {
    pub fn new(source_dir: &'a Path, images: &'a dyn ImageProbe) -> Self {
        Self::with_template(source_dir, images, Template::default())
    }

    /// The same, against a template that is not the default — which is every template
    /// once 3e loads them from disk.
    pub fn with_template(
        source_dir: &'a Path,
        images: &'a dyn ImageProbe,
        template: Template,
    ) -> Self {
        Self {
            source_dir: Some(source_dir),
            images,
            template,
        }
    }
}

/// The result of converting one Source: its Elements, plus the Compromises already
/// made — before any Typst compilation exists. See `design/GLOSSARY.md`.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Conversion {
    pub elements: Vec<Element>,
    /// Concessions made during conversion — an unsupported construct, a missing or
    /// remote image. Every one names an Element in `elements`, so the attention gate
    /// can offer an adjustment for it.
    ///
    /// These are **not** the ProbePass's compromises. Merging the two into one sealed
    /// `Diagnostic` is the engine's job; a Compromise from here carries `page: None`
    /// because pagination has not happened yet.
    pub compromises: Vec<Compromise>,
    /// Files the engine must read and register before compiling.
    pub images: ImageManifest,
}

impl Conversion {
    /// True when conversion made a judgment call the user should see.
    ///
    /// Mirrors `Diagnostic::is_flagged`. Flagged is not failed — the document
    /// converted, and something in it could not be represented faithfully.
    pub fn is_flagged(&self) -> bool {
        !self.compromises.is_empty()
    }
}

/// Convert markdown into Elements ready for typesetting.
///
/// **Total — there is no `Result`.** Parsing cannot fail (`pulldown-cmark` accepts any
/// `&str`), and emission cannot fail either: a construct md2pdf cannot represent
/// becomes a `Compromise`, not an error. A `ConversionError` would be a type with no
/// inhabitants, so the signature does not pretend otherwise.
///
/// Templates are not involved. Conversion decides *what* the elements are; the
/// Template decides what they look like, and the ProbePass decides what fits.
pub fn convert(markdown: &str, ctx: &SourceContext) -> Conversion {
    let emitted = emit::emit(&parse::parse(markdown), ctx);
    Conversion {
        elements: emitted.elements,
        compromises: emitted.compromises,
        images: emitted.images,
    }
}
