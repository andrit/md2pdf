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

use std::collections::BTreeMap;

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
    /// 1-based page it landed on, once the document has been laid out.
    ///
    /// `None` until then, and **`None` rather than a guess** where the layout could not
    /// place it. Only the RenderPass knows this — a sealed Diagnostic is built from
    /// probe decisions, and the probe measures elements outside the page flow.
    pub page: Option<u32>,
}

/// Every Element that made the same kind of concession, as one row.
///
/// **`what` is a representative, not a summary.** Members of a group differ only in
/// detail — two elements shrunk to 9pt and 8pt share a row and the row names one of the
/// sizes. That detail is the least of what the reader needs; *how many* and *where* is
/// the most, and both are exact.
#[derive(Debug, Clone, PartialEq)]
pub struct AttentionGroup {
    /// What made these one row. Not shown — `CompromiseKind::group_key`.
    pub key: String,
    pub what: CompromiseKind,
    pub ids: Vec<ElementId>,
    /// Pages to turn to, ascending and deduplicated. Empty when the document has not
    /// been laid out, or where the layout could not place the Element.
    pub pages: Vec<u32>,
    /// Identical for every member, because `offers_for` reads only the variant — which
    /// is what makes one button for the whole row honest.
    pub offers: Vec<OfferedFix>,
}

impl AttentionGroup {
    pub fn count(&self) -> usize {
        self.ids.len()
    }
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
                    page: c.page,
                })
                .collect(),
        }
    }

    pub fn is_empty(&self) -> bool {
        self.items.is_empty()
    }

    /// The list a person should actually read: one row per kind of concession, worst
    /// first, each saying how many Elements made it and which pages to turn to.
    ///
    /// **Grouping and ranking are read-model work, not display work**, which is why they
    /// are here where they can be tested rather than in the widget that cannot be. The
    /// window draws this in order and adds no judgment of its own.
    ///
    /// Ordering within a severity band is by first appearance, so two runs of the same
    /// document produce the same list.
    pub fn grouped(&self) -> Vec<AttentionGroup> {
        let mut groups: Vec<AttentionGroup> = Vec::new();
        for item in &self.items {
            let key = item.what.group_key();
            match groups.iter_mut().find(|g| g.key == key) {
                Some(group) => {
                    group.ids.push(item.id);
                    group.pages.extend(item.page);
                }
                None => groups.push(AttentionGroup {
                    key,
                    what: item.what.clone(),
                    ids: vec![item.id],
                    pages: item.page.into_iter().collect(),
                    offers: item.offers.clone(),
                }),
            }
        }
        for group in &mut groups {
            group.pages.sort_unstable();
            group.pages.dedup();
        }
        // Stable, so equal severities keep the order the document put them in.
        groups.sort_by_key(|g| g.what.severity());
        groups
    }

    /// Attach the page each Element landed on, keyed by `ElementId::order`.
    ///
    /// **Composed in rather than looked up**, because this crate cannot ask a document
    /// anything — the map comes from `Compilation::element_pages()` and the adapter
    /// carries it across. An Element missing from the map keeps whatever page it had,
    /// which is `None`: a reference that is confidently wrong is worse than none.
    pub fn with_pages(mut self, pages: &BTreeMap<u32, u32>) -> Self {
        for item in &mut self.items {
            item.page = pages.get(&item.id.order).copied().or(item.page);
        }
        self
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

#[cfg(test)]
mod grouping {
    use super::*;
    use crate::diagnostic::{Compromise, Severity};

    fn at(order: u32, kind: CompromiseKind, page: Option<u32>) -> Compromise {
        Compromise {
            id: ElementId::new(order, "body"),
            kind,
            page,
        }
    }

    fn list(compromises: Vec<Compromise>) -> AttentionList {
        AttentionList::from_diagnostic(&Diagnostic { compromises })
    }

    #[test]
    fn the_worst_thing_that_happened_is_first() {
        // The defect this exists for: five reflowed tables and one clipped element, and
        // the clipped one — the only one that *lost* anything — was sixth on the list and
        // off the bottom of the panel.
        let l = list(vec![
            at(0, CompromiseKind::Reflowed, Some(1)),
            at(1, CompromiseKind::Reflowed, Some(2)),
            at(2, CompromiseKind::ShrunkToFloor { size_pt: 9.0 }, Some(2)),
            at(3, CompromiseKind::Clipped, Some(4)),
            at(4, CompromiseKind::Reflowed, Some(5)),
        ]);
        let kinds: Vec<_> = l.grouped().iter().map(|g| g.what.severity()).collect();
        assert_eq!(
            kinds,
            vec![Severity::Lost, Severity::Reduced, Severity::Intact],
            "the list is not ordered worst-first"
        );
        assert_eq!(l.grouped()[0].what, CompromiseKind::Clipped);
    }

    #[test]
    fn one_row_per_kind_however_many_elements_made_it() {
        let l = list(vec![
            at(0, CompromiseKind::Reflowed, Some(1)),
            at(1, CompromiseKind::Reflowed, Some(4)),
            at(2, CompromiseKind::Reflowed, Some(4)),
        ]);
        let groups = l.grouped();
        assert_eq!(groups.len(), 1, "three tables drew three rows");
        assert_eq!(groups[0].count(), 3);
        // Deduplicated and ascending: two elements on page 4 is still one page to turn to.
        assert_eq!(groups[0].pages, vec![1, 4]);
    }

    #[test]
    fn elements_shrunk_to_different_sizes_still_share_a_row() {
        // They made the same kind of concession and carry the same offers. Splitting on
        // the exact point size would rebuild the noise grouping exists to collapse.
        let l = list(vec![
            at(0, CompromiseKind::ShrunkToFloor { size_pt: 9.0 }, Some(1)),
            at(1, CompromiseKind::ShrunkToFloor { size_pt: 8.0 }, Some(3)),
        ]);
        assert_eq!(l.grouped().len(), 1);
        assert_eq!(l.grouped()[0].count(), 2);
    }

    #[test]
    fn each_unrepresentable_construct_keeps_its_own_row() {
        // The exception, and it is deliberate: "2 things md2pdf could not represent"
        // names neither of them, and each is a different thing to go and fix.
        let l = list(vec![
            at(
                0,
                CompromiseKind::UnsupportedConstruct {
                    construct: "footnote".into(),
                },
                Some(1),
            ),
            at(
                1,
                CompromiseKind::UnsupportedConstruct {
                    construct: "task list".into(),
                },
                Some(2),
            ),
        ]);
        assert_eq!(l.grouped().len(), 2);
    }

    #[test]
    fn a_group_offers_what_every_member_was_offered() {
        // What makes one button for a whole row honest — `offers_for` reads only the
        // variant, so every member of a group was offered the same thing.
        let l = list(vec![
            at(0, CompromiseKind::Reflowed, None),
            at(1, CompromiseKind::Reflowed, None),
        ]);
        let group = &l.grouped()[0];
        for id in &group.ids {
            let item = l.items.iter().find(|i| i.id == *id).expect("member");
            assert_eq!(item.offers, group.offers);
        }
        // And with no layout yet, it says nothing about pages rather than guessing.
        assert!(group.pages.is_empty());
    }

    #[test]
    fn pages_are_attached_from_the_laid_out_document() {
        let mut pages = BTreeMap::new();
        pages.insert(0, 3);
        pages.insert(1, 7);
        let l = list(vec![
            at(0, CompromiseKind::Reflowed, None),
            at(1, CompromiseKind::Reflowed, None),
        ])
        .with_pages(&pages);
        assert_eq!(l.grouped()[0].pages, vec![3, 7]);
    }

    #[test]
    fn an_element_the_layout_could_not_place_reports_no_page() {
        // Rather than defaulting to page 1, which would send the reader somewhere
        // confidently wrong.
        let mut pages = BTreeMap::new();
        pages.insert(0, 3);
        let l = list(vec![
            at(0, CompromiseKind::Reflowed, None),
            at(9, CompromiseKind::Reflowed, None),
        ])
        .with_pages(&pages);
        assert_eq!(l.grouped()[0].pages, vec![3], "a missing page was invented");
        assert!(l.items[1].page.is_none());
    }
}
