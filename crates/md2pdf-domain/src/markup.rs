//! Markup — Typst markup text emitted from a parsed Source.
//!
//! Note the boundary this sits on. Typst *markup syntax* is a stable surface and
//! md2pdf-convert emits it freely. The typst *Rust crate* is an unstable surface
//! and only md2pdf-typeset may link it. Those are different things, and conflating
//! them would put an anti-corruption layer in the wrong place.

use serde::{Deserialize, Serialize};

/// Typst markup, ready to be interpolated into a document.
///
/// ## What this type does and does not guarantee
///
/// It does **not** prove its contents are escaped — `raw` accepts any string, and no
/// value type could check "is this correctly escaped Typst" without a parser.
///
/// What it does is make every unescaped construction **say so at the call site**. The
/// only constructor is [`Markup::raw`], so text that came from a user document cannot
/// reach an [`Element`](crate::element::Element) body without someone writing the word
/// `raw` — which is the review signal. `md2pdf-convert` escapes with its `escape`
/// module and wraps the result here; nothing else should call `raw` on user text.
///
/// Why it matters: an `Element` body is interpolated into Typst source both raw at top
/// level and inside a content block `[...]`. Unescaped `#` is arbitrary code execution
/// from document text, and an unbalanced `]` silently terminates the probe harness —
/// corrupting the measurement of an unrelated element rather than raising an error.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Markup(String);

impl Markup {
    /// Wrap text that is **already** valid Typst markup.
    ///
    /// Correct for markup md2pdf generates itself (`= Heading`, `#table(...)`) and for
    /// document text that has been through `md2pdf-convert`'s `escape`. Passing raw
    /// user text is the bug this type exists to make visible.
    pub fn raw(text: impl Into<String>) -> Self {
        Self(text.into())
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }

    pub fn into_string(self) -> String {
        self.0
    }

    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }
}

impl std::fmt::Display for Markup {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}
