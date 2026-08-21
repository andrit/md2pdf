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
    /// A table was rendered with wrapping columns instead of content-sized ones.
    ///
    /// Recorded even though the result is usually *better* than the alternative: it is
    /// still a departure from the authored shape, and `INV-4` says every judgment call
    /// md2pdf makes on the user's behalf is reported.
    Reflowed,
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

    /// Seal a Diagnostic from **both** halves of the pipeline.
    ///
    /// Compromises arise in two places that never meet: conversion produces
    /// `ImageMissing`, `ImageSkipped` and `UnsupportedConstruct` before any compilation
    /// exists, and the ProbePass produces the ladder's concessions afterwards. Until
    /// this existed, the convert-time half had **no route into a Diagnostic at all** —
    /// so `INV-4` held in the code and broke at the join.
    ///
    /// Ordered by `ElementId.order`, so a Diagnostic reads in document order rather
    /// than grouped by which pass happened to produce it. The user does not care which
    /// half of the pipeline conceded; they care where in their document it happened.
    ///
    /// This is the only way to build a complete Diagnostic. [`Diagnostic::from_decisions`]
    /// remains for the ladder-only case that contract tests use, and is *not* complete
    /// by construction — which is why sealing is a separate, named act.
    pub fn seal(convert_compromises: Vec<Compromise>, map: &DecisionMap) -> Self {
        let mut compromises = convert_compromises;
        compromises.extend(Self::from_decisions(map).compromises);
        compromises.sort_by_key(|c| c.id.order);
        Self { compromises }
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
                Reduction::Reflow => Some(CompromiseKind::Reflowed),
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::decision::{Decision, Orientation, Reduction};
    use crate::element::ElementId;

    fn id(order: u32) -> ElementId {
        ElementId::new(order, "body")
    }

    #[test]
    fn sealing_carries_both_halves_in_document_order() {
        // The §4 seam: conversion concedes before compilation exists, the ProbePass
        // concedes afterwards, and until `seal` they had no common destination.
        let convert = vec![
            Compromise {
                id: id(3),
                kind: CompromiseKind::ImageMissing,
                page: None,
            },
            Compromise {
                id: id(0),
                kind: CompromiseKind::ImageSkipped,
                page: None,
            },
        ];
        let map = DecisionMap {
            decisions: vec![Decision {
                id: id(1),
                orientation: Orientation::Landscape,
                reduction: Reduction::Shrink { size_pt: 7.0 },
                natural_pt: 400.0,
                available_pt: 200.0,
            }],
        };

        let sealed = Diagnostic::seal(convert, &map);

        let orders: Vec<u32> = sealed.compromises.iter().map(|c| c.id.order).collect();
        assert_eq!(
            orders,
            vec![0, 1, 1, 3],
            "a Diagnostic must read in document order, not grouped by which pass conceded"
        );
        // The rotated-and-shrunk element contributes both of its concessions.
        assert_eq!(
            sealed
                .compromises
                .iter()
                .filter(|c| c.id.order == 1)
                .count(),
            2
        );
    }

    #[test]
    fn a_clean_document_seals_to_an_empty_diagnostic() {
        let sealed = Diagnostic::seal(vec![], &DecisionMap::default());
        assert!(!sealed.is_flagged(), "an empty Diagnostic means clean");
    }

    #[test]
    fn convert_time_compromises_survive_sealing() {
        // The regression this exists to prevent: before `seal`, these had no route in
        // at all, so INV-4 held in the code and broke at the join.
        let sealed = Diagnostic::seal(
            vec![Compromise {
                id: id(0),
                kind: CompromiseKind::UnsupportedConstruct {
                    construct: "html: <div>".into(),
                },
                page: None,
            }],
            &DecisionMap::default(),
        );
        assert_eq!(sealed.compromises.len(), 1);
        assert!(sealed.is_flagged());
    }
}
