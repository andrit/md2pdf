//! Element, ElementId, ElementClass. See `design/GLOSSARY.md`.

use serde::{Deserialize, Serialize};

use crate::hash::fnv1a;
use crate::markup::Markup;

/// Addresses one Element across the ProbePass, the RenderPass, and recompilations.
///
/// `order` is assigned by md2pdf when it emits the Markup — stable by construction,
/// because md2pdf generates the markup rather than inferring structure back out of
/// Typst's tree.
///
/// `content_hash` exists so that a persisted Override cannot silently misapply after
/// the Source is edited externally. Order alone shifts when elements are inserted;
/// the hash catches it and the Override is dropped rather than applied to the wrong
/// element.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct ElementId {
    pub order: u32,
    pub content_hash: u64,
}

impl ElementId {
    pub fn new(order: u32, body: &str) -> Self {
        Self {
            order,
            content_hash: fnv1a(body.as_bytes()),
        }
    }

    /// True when this id refers to the same element as `other` — same position and
    /// same content. A stale Override fails this and is discarded.
    pub fn matches(&self, other: &ElementId) -> bool {
        self.order == other.order && self.content_hash == other.content_hash
    }
}

/// The category an Element belongs to. Carries three jobs: it selects the Floor, it
/// selects the overflow predicate via [`ElementClass::is_atomic`], and it defines what
/// "shrink" means.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ElementClass {
    Prose,
    Heading,
    Quote,
    List,
    Code,
    Table,
    Image,
    Caption,
}

impl ElementClass {
    /// Atomic content's natural width IS its required width, so it can overflow.
    /// Wrappable content reflows to any width and cannot overflow horizontally —
    /// the ladder skips it entirely.
    ///
    /// Verified against Typst 0.15.1: `raw` blocks wrap, so `Code` is Wrappable.
    /// See `design/spike-typst-measure-findings.md`.
    pub fn is_atomic(self) -> bool {
        matches!(self, Self::Table | Self::Image)
    }

    /// Shrinking means different things per class: a font size for text-bearing
    /// content, a scale factor for images. A rect's width does not care what size
    /// the text is.
    pub fn shrinks_by_scale(self) -> bool {
        matches!(self, Self::Image)
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Self::Prose => "prose",
            Self::Heading => "heading",
            Self::Quote => "quote",
            Self::List => "list",
            Self::Code => "code",
            Self::Table => "table",
            Self::Image => "image",
            Self::Caption => "caption",
        }
    }
}

/// One measurable unit of content, carrying its own Typst markup fragment.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Element {
    pub id: ElementId,
    pub class: ElementClass,
    /// Typst markup for this element's body, emitted by md2pdf-convert.
    ///
    /// Typed as [`Markup`] rather than `String` so that unescaped document text
    /// cannot arrive here without an explicit `Markup::raw` at the call site.
    pub body: Markup,
}

impl Element {
    pub fn new(order: u32, class: ElementClass, body: Markup) -> Self {
        Self {
            id: ElementId::new(order, body.as_str()),
            class,
            body,
        }
    }
}
