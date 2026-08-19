//! Decision and DecisionMap — the ProbePass -> RenderPass contract.

use serde::{Deserialize, Serialize};

use crate::element::ElementId;

/// Which rung of the Escalation ladder was reached.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(tag = "rung", rename_all = "lowercase")]
pub enum Rung {
    /// Fits as-is, or is Wrappable and therefore cannot overflow.
    None,
    /// Text stepped down to `size_pt`, at or above the class Floor.
    ///
    /// Font size only. Content that scales rather than reflows uses [`Rung::Scale`] —
    /// "8pt" and "40%" are different facts, and the attention gate must not have to
    /// reverse-engineer which one a number is.
    Shrink { size_pt: f64 },
    /// Scaled by `factor` (0..1), at or above the class scale Floor.
    ///
    /// For classes where shrinking means resizing rather than restyling — images.
    /// Applied with `reflow: true`, so the element really does occupy less space;
    /// Typst's default scale is visual only and would leave the overflow in place.
    Scale { factor: f64 },
    /// At the Floor and still over — goes to landscape on its own page.
    ///
    /// The element renders at natural size there. It is **not** re-measured: the
    /// RenderPass never measures, by design. An element too wide even for landscape
    /// still overflows — see T14, "finish the ladder".
    Rotate,
    /// Still over after rotation. Clipped, with a visible marker.
    Clip,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Decision {
    pub id: ElementId,
    pub rung: Rung,
    /// Natural (infinite-space) width in points, as measured. Retained for the
    /// Diagnostic and for contract tests.
    pub natural_pt: f64,
    pub available_pt: f64,
}

impl Decision {
    pub fn is_compromise(&self) -> bool {
        !matches!(self.rung, Rung::None)
    }
}

/// Every Decision for one document, plus any user Overrides layered on top.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct DecisionMap {
    pub decisions: Vec<Decision>,
}

impl DecisionMap {
    pub fn get(&self, id: &ElementId) -> Option<&Decision> {
        self.decisions.iter().find(|d| d.id.matches(id))
    }

    /// Apply a user Override. Returns false and changes nothing if the Override is
    /// stale — the Source was edited and this id no longer names the same element.
    pub fn apply_override(&mut self, id: &ElementId, rung: Rung) -> bool {
        match self.decisions.iter_mut().find(|d| d.id.matches(id)) {
            Some(d) => {
                d.rung = rung;
                true
            }
            None => false,
        }
    }

    pub fn compromises(&self) -> impl Iterator<Item = &Decision> {
        self.decisions.iter().filter(|d| d.is_compromise())
    }
}
