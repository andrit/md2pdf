//! Markdown -> Markup. Pure and total: text in, text out.
//!
//! Assigns the `ElementId` and `ElementClass` for every element as it emits.
//! That is what makes ids stable by construction — md2pdf generates the markup
//! rather than inferring structure back out of typst's tree.
//!
//! No I/O. Image paths are *resolved* here but never *read*; reading is
//! `md2pdf-paths`' job.

// markdown construct -> ElementClass (Atomic vs Wrappable)
pub mod classify;
// internal events -> Typst Markup
pub mod emit;
// relative-path resolution; remote-image policy
pub mod images;
// pulldown-cmark -> internal event stream
pub mod parse;
