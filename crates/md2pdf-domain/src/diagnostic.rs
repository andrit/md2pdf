//! Compromise and Diagnostic — what the layout pass admits it did.

use serde::{Deserialize, Serialize};

use crate::decision::{DecisionMap, Orientation, Reduction};
use crate::element::ElementId;

/// One recorded concession. The central noun of the design: it is what makes
/// batch preview exception-driven, and what makes Element-scope adjustment
/// offerable at all.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Compromise {
    pub id: ElementId,
    pub kind: CompromiseKind,
    /// 1-based page the element landed on, when known.
    pub page: Option<u32>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CompromiseKind {
    ShrunkToFloor {
        size_pt: f64,
    },
    /// Resized rather than restyled — images. Reported as a percentage, because that
    /// is the fact the reader needs; a point size would be meaningless here.
    Scaled {
        factor: f64,
    },
    Rotated,
    Clipped,
    ImageSkipped,
    ImageMissing,
    /// A markdown construct md2pdf cannot represent, named so the attention gate can
    /// say *which*. Raised at conversion time, before any Typst compilation exists —
    /// unlike the ladder rungs above, which come from the ProbePass.
    UnsupportedConstruct {
        construct: String,
    },
}

/// Every Compromise recorded during one Compilation. Sealed when the ProbePass
/// finishes; discarded whole and rebuilt on recompile, never merged.
///
/// An empty Diagnostic means the document converted cleanly.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct Diagnostic {
    pub compromises: Vec<Compromise>,
}

impl Diagnostic {
    /// A Source is Flagged when its Diagnostic is non-empty. Flagged is not failed —
    /// it converted successfully, and a judgment call was made on the user's behalf.
    pub fn is_flagged(&self) -> bool {
        !self.compromises.is_empty()
    }

    /// Build a Diagnostic from the ProbePass's decisions.
    ///
    /// An Element can concede on **both** axes — rotated *and* shrunk is the ordinary
    /// outcome once landscape re-measurement exists — so each non-default axis
    /// contributes its own Compromise. The attention list then reads "rotated, and
    /// shrunk to 7pt" rather than needing a composite kind per combination.
    pub fn from_decisions(map: &DecisionMap) -> Self {
        let mut compromises = Vec::new();
        for d in map.compromises() {
            if d.orientation == Orientation::Landscape {
                compromises.push(Compromise {
                    id: d.id,
                    kind: CompromiseKind::Rotated,
                    page: None,
                });
            }
            let kind = match d.reduction {
                Reduction::Shrink { size_pt } => Some(CompromiseKind::ShrunkToFloor { size_pt }),
                Reduction::Scale { factor } => Some(CompromiseKind::Scaled { factor }),
                Reduction::Clip => Some(CompromiseKind::Clipped),
                Reduction::None => None,
            };
            if let Some(kind) = kind {
                compromises.push(Compromise {
                    id: d.id,
                    kind,
                    page: None,
                });
            }
        }
        Self { compromises }
    }
}
