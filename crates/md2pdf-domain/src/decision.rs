//! Decision and DecisionMap — the ProbePass -> RenderPass contract.

use serde::{Deserialize, Serialize};

use crate::element::ElementId;

/// Which way the page runs for this Element.
///
/// Rotation exists **only** to change the width a reduction is measured against —
/// landscape offers roughly 1.5x more. It is not a step that replaces reduction, which
/// is why orientation and reduction are separate axes rather than rungs of one enum.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Orientation {
    #[default]
    Portrait,
    /// On its own page, flipped. The Element was re-measured against the landscape
    /// width before this was chosen — it does **not** inherit its portrait size.
    Landscape,
}

/// How much the Element had to give up to fit the width available to it.
///
/// Separate from [`Orientation`] because an Element commonly does both: too wide for
/// portrait, rotated, and *still* needing to shrink in landscape. A single enum could
/// not express that, and Element-scope Overrides — "force landscape", "allow clip",
/// "permit below-floor" — are three independent toggles that each set one axis.
#[derive(Debug, Clone, Copy, PartialEq, Default, Serialize, Deserialize)]
#[serde(tag = "reduction", rename_all = "lowercase")]
pub enum Reduction {
    /// Fits as-is, or is Wrappable and therefore cannot overflow.
    #[default]
    None,
    /// Text stepped down to `size_pt`, at or above the class Floor.
    ///
    /// Font size only. Content that resizes rather than restyles uses [`Reduction::Scale`]
    /// — "8pt" and "40%" are different facts, and the attention gate must not have to
    /// reverse-engineer which one a number is.
    Shrink { size_pt: f64 },
    /// Scaled by `factor` (0..1), at or above the class scale Floor. For images.
    ///
    /// Applied with `reflow: true`: Typst's default scale is a visual transform and
    /// would leave the overflow exactly where it was.
    Scale { factor: f64 },
    /// Rendered in its alternate, always-fitting form — a table with fractional
    /// columns, wrapping inside cells rather than sizing them to content.
    ///
    /// Sits immediately before [`Reduction::Clip`], and takes precedence: reflowing
    /// changes how a table *looks*, clipping destroys what it *says*. Only Elements
    /// carrying an `Element::reflow` alternate can reach this rung.
    Reflow,
    /// Over even at the Floor in landscape, with no alternate form available. Clipped,
    /// with a visible marker — the last rung, and the only outcome that loses content.
    /// Reachable now only for Elements that cannot reflow, such as images.
    Clip,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Decision {
    pub id: ElementId,
    pub orientation: Orientation,
    pub reduction: Reduction,
    /// Natural (infinite-space) width in points, as measured. Retained for the
    /// Diagnostic and for contract tests.
    pub natural_pt: f64,
    /// The width the reduction was decided against — the landscape width when
    /// `orientation` is `Landscape`.
    pub available_pt: f64,
}

impl Decision {
    /// True when md2pdf made a judgment call on the user's behalf.
    pub fn is_compromise(&self) -> bool {
        self.orientation != Orientation::Portrait || self.reduction != Reduction::None
    }

    /// A Decision that changes nothing — the common case.
    pub fn fits(id: ElementId, natural_pt: f64, available_pt: f64) -> Self {
        Self {
            id,
            orientation: Orientation::Portrait,
            reduction: Reduction::None,
            natural_pt,
            available_pt,
        }
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

    /// Apply a user Override to one or both axes.
    ///
    /// Each axis is set independently — "force landscape" must not disturb a shrink the
    /// probe chose, and "permit below-floor" must not undo a rotation. Passing `None`
    /// for an axis leaves it as decided.
    ///
    /// Returns false and changes nothing if the Override is stale: the Source was
    /// edited and this id no longer names the same element.
    pub fn apply_override(
        &mut self,
        id: &ElementId,
        orientation: Option<Orientation>,
        reduction: Option<Reduction>,
    ) -> bool {
        match self.decisions.iter_mut().find(|d| d.id.matches(id)) {
            Some(d) => {
                if let Some(o) = orientation {
                    d.orientation = o;
                }
                if let Some(r) = reduction {
                    d.reduction = r;
                }
                true
            }
            None => false,
        }
    }

    pub fn compromises(&self) -> impl Iterator<Item = &Decision> {
        self.decisions.iter().filter(|d| d.is_compromise())
    }
}
