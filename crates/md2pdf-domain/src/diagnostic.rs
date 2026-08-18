//! Compromise and Diagnostic — what the layout pass admits it did.

use serde::{Deserialize, Serialize};

use crate::decision::{DecisionMap, Rung};
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

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CompromiseKind {
    ShrunkToFloor { size_pt: f64 },
    Rotated,
    Clipped,
    ImageSkipped,
    ImageMissing,
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

    pub fn from_decisions(map: &DecisionMap) -> Self {
        let compromises = map
            .compromises()
            .filter_map(|d| {
                let kind = match d.rung {
                    Rung::Shrink { size_pt } => CompromiseKind::ShrunkToFloor { size_pt },
                    Rung::Rotate => CompromiseKind::Rotated,
                    Rung::Clip => CompromiseKind::Clipped,
                    Rung::None => return None,
                };
                Some(Compromise {
                    id: d.id,
                    kind,
                    page: None,
                })
            })
            .collect();
        Self { compromises }
    }
}
