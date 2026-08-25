//! Review — what md2pdf conceded, and what it can do instead.
//!
//! The payoff for treating layout as something that *emits*. The ladder has recorded every
//! judgment call since T13; this is the first thing that offers to revisit one.
//!
//! ## An Override is a permission, not an outcome
//!
//! The user says what they will *allow*; the ladder still decides what that allows. That
//! separation is the whole engine, and getting it backwards is a real defect rather than a
//! stylistic preference:
//!
//! > RE-MEASURE in landscape; do not inherit the portrait size. Carrying the Floor size
//! > over is a bug. — `GLOSSARY`, and the reason T14 exists.
//!
//! `DecisionMap::apply_override` used to set the two axes directly, which let a caller
//! turn an Element landscape while keeping a size chosen against the *portrait* width. It
//! had no callers, which is the only reason it never shipped that bug. An [`Override`]
//! carries a [`Permit`] instead, the ProbePass measures under it, and there is no longer a
//! way to name an orientation without a size to go with it. See `design/plan-review.md`.

use serde::{Deserialize, Serialize};

use crate::diagnostic::{Compromise, CompromiseKind, Diagnostic};
use crate::element::ElementId;

/// What the user permits for one Element, for this Job only.
///
/// Not persistent: Job and Element scope last one Job (`GLOSSARY`, Scope). A permanent
/// change is Template scope — `template.toml`, which belongs to the template author and
/// which 3e made editable. "Why did my override vanish" therefore has an answer, and it is
/// a decision rather than an oversight.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Permit {
    /// A page of its own, in landscape — roughly 1.5x the width.
    Landscape,
    /// Text smaller than the Floor would allow, down to this size.
    BelowFloor { to_pt: f64 },
    /// Losing what does not fit. **The only Permit that destroys content**, which is why
    /// it is offered last and never chosen by the ladder while anything else remains.
    Clip,
}

/// A user decision at Element scope.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct Override {
    pub id: ElementId,
    pub permit: Permit,
}

/// A fix md2pdf can offer for one Compromise.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct OfferedFix {
    pub permit: Permit,
    /// What it would do, in the user's terms rather than the ladder's.
    pub label: &'static str,
}

/// One entry in the attention list: what was done, and what could be done instead.
#[derive(Debug, Clone, PartialEq)]
pub struct Attention {
    pub id: ElementId,
    pub what: CompromiseKind,
    pub offers: Vec<OfferedFix>,
}

/// Everything in one document that needed a judgment call.
///
/// Drawn by phase 4 as `attention_list()` — the read model and the widget share a name on
/// purpose (`GLOSSARY`, naming).
#[derive(Debug, Clone, Default, PartialEq)]
pub struct AttentionList {
    pub items: Vec<Attention>,
}

impl AttentionList {
    /// Read a sealed Diagnostic as a list of things a person could act on.
    pub fn from_diagnostic(diagnostic: &Diagnostic) -> Self {
        Self {
            items: diagnostic
                .compromises
                .iter()
                .map(|c| Attention {
                    id: c.id,
                    what: c.kind.clone(),
                    offers: offers_for(c),
                })
                .collect(),
        }
    }

    pub fn is_empty(&self) -> bool {
        self.items.is_empty()
    }

    /// Only the entries md2pdf can actually do something about.
    ///
    /// A missing image file is a Compromise and is *not* actionable here — no Override
    /// puts a file on disk. Offering a fix that cannot work is worse than offering none,
    /// because it teaches the user the list is noise.
    pub fn actionable(&self) -> impl Iterator<Item = &Attention> {
        self.items.iter().filter(|a| !a.offers.is_empty())
    }
}

/// What can be offered for a given Compromise.
///
/// **Element scope is offered only where the Diagnostic named an Element** (`GLOSSARY`):
/// md2pdf is not a layout editor, and it offers a fix exactly where the engine already
/// admitted it compromised — no more.
fn offers_for(c: &Compromise) -> Vec<OfferedFix> {
    match &c.kind {
        // Wrapped cells at full size. A landscape page keeps the author's column
        // proportions instead; clipping is not offered, because reflow already lost
        // nothing and clip would.
        CompromiseKind::Reflowed => vec![OfferedFix {
            permit: Permit::Landscape,
            label: "give it a landscape page instead of wrapping",
        }],
        // Smaller text. More width would mean less shrinking; going below the floor is the
        // other direction and is the user's to allow.
        CompromiseKind::ShrunkToFloor { .. } => vec![
            OfferedFix {
                permit: Permit::Landscape,
                label: "give it a landscape page so it need not shrink so far",
            },
            OfferedFix {
                permit: Permit::BelowFloor { to_pt: 6.0 },
                label: "allow smaller text than the floor permits",
            },
        ],
        // Already on its own page and still reduced.
        CompromiseKind::Scaled { .. } | CompromiseKind::Rotated => vec![OfferedFix {
            permit: Permit::BelowFloor { to_pt: 6.0 },
            label: "allow more reduction than the floor permits",
        }],
        // The only kind that already lost content. Everything is offered, because
        // anything is better than what happened.
        CompromiseKind::Clipped => vec![
            OfferedFix {
                permit: Permit::Landscape,
                label: "give it a landscape page",
            },
            OfferedFix {
                permit: Permit::BelowFloor { to_pt: 6.0 },
                label: "allow smaller text than the floor permits",
            },
        ],
        // Nothing an Override can do: no permission puts a file on disk or teaches the
        // converter a construct.
        CompromiseKind::ImageMissing
        | CompromiseKind::ImageSkipped
        | CompromiseKind::UnsupportedConstruct { .. } => Vec::new(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::diagnostic::Compromise;

    fn compromise(order: u32, kind: CompromiseKind) -> Compromise {
        Compromise {
            id: ElementId::new(order, "body"),
            kind,
            page: None,
        }
    }

    #[test]
    fn a_clean_document_needs_no_attention() {
        let list = AttentionList::from_diagnostic(&Diagnostic::default());
        assert!(list.is_empty());
    }

    #[test]
    fn a_reflowed_table_is_offered_a_landscape_page() {
        let d = Diagnostic {
            compromises: vec![compromise(1, CompromiseKind::Reflowed)],
        };
        let list = AttentionList::from_diagnostic(&d);
        assert_eq!(list.items.len(), 1);
        assert_eq!(list.items[0].offers[0].permit, Permit::Landscape);
    }

    #[test]
    fn reflow_is_never_offered_a_clip() {
        // Reflow lost nothing; clipping would. Offering it would be offering a downgrade.
        let d = Diagnostic {
            compromises: vec![compromise(1, CompromiseKind::Reflowed)],
        };
        let list = AttentionList::from_diagnostic(&d);
        assert!(!list.items[0]
            .offers
            .iter()
            .any(|o| o.permit == Permit::Clip));
    }

    #[test]
    fn a_missing_image_is_listed_but_offers_nothing() {
        // It is still a Compromise the user should see — INV-4 — but no permission puts a
        // file on disk, and offering a fix that cannot work teaches them the list is noise.
        let d = Diagnostic {
            compromises: vec![compromise(1, CompromiseKind::ImageMissing)],
        };
        let list = AttentionList::from_diagnostic(&d);
        assert_eq!(list.items.len(), 1, "it must still be listed");
        assert!(list.items[0].offers.is_empty());
        assert_eq!(list.actionable().count(), 0);
    }

    #[test]
    fn every_kind_that_can_be_fixed_offers_something() {
        // A guard against a new CompromiseKind silently arriving with no offers: if the
        // ladder learns a new rung, this says so rather than the list quietly shrinking.
        for kind in [
            CompromiseKind::Reflowed,
            CompromiseKind::ShrunkToFloor { size_pt: 8.0 },
            CompromiseKind::Scaled { factor: 0.5 },
            CompromiseKind::Rotated,
            CompromiseKind::Clipped,
        ] {
            let d = Diagnostic {
                compromises: vec![compromise(1, kind.clone())],
            };
            let list = AttentionList::from_diagnostic(&d);
            assert!(
                !list.items[0].offers.is_empty(),
                "{kind:?} is fixable but offers nothing"
            );
        }
    }
}
