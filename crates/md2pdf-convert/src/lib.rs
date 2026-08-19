//! Markdown -> Markup. Pure and total: text in, text out.
//!
//! Assigns the `ElementId` and `ElementClass` for every element as it emits.
//! That is what makes ids stable by construction — md2pdf generates the markup
//! rather than inferring structure back out of typst's tree.
//!
//! No I/O. Image paths are *resolved* here but never *read*; reading is
//! `md2pdf-paths`' job.
//!
//! ```
//! let conversion = md2pdf_convert::convert("# Title\n\nSome *prose*.\n");
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
pub mod parse;

use md2pdf_domain::{Compromise, Element};

/// The result of converting one Source: its Elements, plus the Compromises already
/// made — before any Typst compilation exists. See `design/GLOSSARY.md`.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Conversion {
    pub elements: Vec<Element>,
    /// Concessions made during conversion — an unsupported construct, a skipped
    /// image. Every one names an Element in `elements`, so the attention gate can
    /// offer an adjustment for it.
    ///
    /// These are **not** the ProbePass's compromises. Merging the two into one sealed
    /// `Diagnostic` is the engine's job; a Compromise from here carries `page: None`
    /// because pagination has not happened yet.
    pub compromises: Vec<Compromise>,
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
pub fn convert(markdown: &str) -> Conversion {
    let emitted = emit::emit(&parse::parse(markdown));
    Conversion {
        elements: emitted.elements,
        compromises: emitted.compromises,
    }
}
