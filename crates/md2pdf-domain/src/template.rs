//! Template tokens. The subset the typesetting layer needs.

use serde::{Deserialize, Serialize};

use crate::element::ElementClass;

/// How small the ladder may go before it stops.
///
/// **Two floors, not five (T26c).** `prose_pt` and `code_pt` were removed rather than
/// documented: **[measured]** they were never read. `Floors::for_class` was called from
/// exactly one place — the probe's text-atomic branch — that branch runs only for
/// `is_atomic` classes, which are `Table | Image`, and `Image` takes the scale branch
/// instead. So it only ever received `Table`, and the other two arms were unreachable
/// from the day `Code` became wrappable.
///
/// They were removed while `Floors` is still only a Rust type. Once 3e loads it from
/// `template.toml` the same removal is a migration for fields that never did anything,
/// and a template author will have spent time tuning them first. See
/// `design/plan-floors.md` D4.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct Floors {
    /// The size at which the ladder gives up shrinking a table.
    ///
    /// **Not "how small a table may get" — [`Floors::table_comfort_pt`] decides that**,
    /// and it sits above this one, so nothing is ever rendered at a size between the two.
    /// What this still does, and why it stays: it bounds the probe's downward scan, and
    /// it sets the text size of the *clip marker* drawn when even landscape will not fit
    /// (`render.rs`). Raising it above the comfort floor would make it observable again.
    ///
    /// Recorded as flag **F4**: a number that looks like a policy and is a bound.
    pub table_pt: f64,
    /// The size below which wrapping reads better than shrinking further.
    ///
    /// A **second** floor on the same class, answering a different question from
    /// [`Floors::table_pt`]:
    ///
    /// | Floor | Question |
    /// |---|---|
    /// | `table_pt` | how small before we give up entirely |
    /// | `table_comfort_pt` | how small before wrapping is *nicer* |
    ///
    /// Above it a squeeze is imperceptible and the author's column proportions are worth
    /// keeping; below it the same table reads better at full size with its cells
    /// wrapping. Applies only to Elements that carry an alternate — a table.
    ///
    /// **10.0pt, chosen by eye against real pages (T26c, 2026-08-24)**, not derived.
    ///
    /// Three real corpus tables were rendered both ways at each candidate boundary and
    /// looked at. The turnover is between 9.0 and 10.0:
    ///
    /// | Would shrink to | Which reads better |
    /// |---|---|
    /// | 8.0pt | **reflow** — a third off the base is small, and wrapping costs little |
    /// | 9.0pt | **reflow** — a quarter off is still a visible drop in size |
    /// | 10.0pt | **shrink** — legible, and it keeps one line per row for scanning |
    /// | 11.0pt | **shrink** — the reduction is imperceptible; reflow wraps for nothing |
    ///
    /// It moved from 9.0 because [`Template::base_size_pt`] moved. The floor is absolute
    /// but *comfort* is relative: against a 10pt base, 9.0 was a tenth off; against 12pt
    /// it is a quarter. The number had to follow the base, which is precisely what T30's
    /// doubt D2 predicted and left for this task to settle.
    ///
    /// **[assumed]** 9.5 is defensible on the same evidence — the boundary is one step
    /// wide and this is a judgement about reading, not a measurement. One number to
    /// change if you disagree; `design/plan-floors.md` holds the pairs.
    pub table_comfort_pt: f64,
    /// Smallest scale an image may be reduced to before rotating instead (0..1).
    ///
    /// Not a point size — images shrink by scale factor. Without a floor here shrink
    /// would always succeed and rotation would never fire, however unreadable the
    /// result. A quarter size is the starting point; rotation buys roughly 1.5x the
    /// width, which is the better trade below that.
    ///
    /// **Untuned, and deliberately left so (T26c).** **[measured]** no image in the
    /// corpus reaches the ladder — every compromised element is a table — so there is
    /// nothing real to judge it against. It could be tuned against invented fixtures,
    /// but that would be choosing a number against documents nobody has and calling it
    /// evidence. It waits for a corpus with images in it.
    pub image_scale: f64,
}

impl Default for Floors {
    fn default() -> Self {
        Self {
            table_pt: 7.0,
            table_comfort_pt: 10.0,
            image_scale: 0.25,
        }
    }
}

impl Floors {
    /// The point-size floor for a text-bearing atomic class.
    ///
    /// **Stops pretending to dispatch (T26c).** It used to `match` four ways, which read
    /// as "each class has its own floor" — but only `Table` ever arrived, so three arms
    /// were decoration. It is kept as a named call rather than inlined so the *question*
    /// stays visible at the call site, and so a future atomic class has an obvious place
    /// to be added rather than a bare field access to notice and change.
    ///
    /// Meaningless for `Image`, which shrinks by scale — use [`Floors::image_scale`].
    pub fn for_class(&self, class: ElementClass) -> f64 {
        match class {
            ElementClass::Image => 0.0,
            // Every other class that reaches here is atomic, and `Table` is the only
            // atomic text class there is. Debug-asserted rather than silently defaulted.
            _ => {
                debug_assert_eq!(
                    class,
                    ElementClass::Table,
                    "a new atomic class reached the floor without one of its own"
                );
                self.table_pt
            }
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Template {
    pub name: String,
    pub page_width_pt: f64,
    pub page_height_pt: f64,
    pub margin_pt: f64,
    /// Body text size — **the size this template is trying to achieve**, not a
    /// starting point to be negotiated down from.
    ///
    /// 12pt, matching GitHub's 16px body, because `github-print` exists to reproduce
    /// what GitHub renders and type size is the first thing a reader notices. It also
    /// puts the measure at ~80 characters per line against A4 with these margins; 10pt
    /// gave 96, well outside the 45-75 band that reads comfortably.
    ///
    /// Every reduction the ladder makes is a departure from this number, and the
    /// Diagnostic reports the distance. That only means anything if the number is
    /// chosen: it was 10.0 for no recorded reason until 2026-08-23, so *"fits at 10pt"*
    /// counted as clean while being a silent concession. See `design/plan-base-size.md`.
    pub base_size_pt: f64,
    pub floors: Floors,
    pub font_body: String,
    pub font_mono: String,
}

impl Default for Template {
    fn default() -> Self {
        Self {
            name: "github-print".into(),
            page_width_pt: 595.0, // A4
            page_height_pt: 842.0,
            margin_pt: 56.0,
            base_size_pt: 12.0,
            floors: Floors::default(),
            font_body: "Source Sans 3".into(),
            font_mono: "JetBrains Mono".into(),
        }
    }
}

impl Template {
    /// Available width, computed directly rather than asked of Typst.
    ///
    /// This is what lets the ProbePass skip `layout()` entirely — 3.6x cheaper for
    /// identical decisions. See `design/spike-typst-measure-findings.md`.
    pub fn available_pt(&self) -> f64 {
        self.page_width_pt - 2.0 * self.margin_pt
    }

    /// Roughly how many characters fit in `width_pt` at the base size.
    ///
    /// An estimate, and deliberately a cheap one: it exists so `md2pdf-convert` can size
    /// break opportunities without measuring, which would mean linking Typst. The ratio
    /// is **[measured]** — Source Sans 3 averages about half the point size per
    /// character, so the full 483pt at 10pt held ~96 characters and at 12pt holds ~80.
    ///
    /// Takes a width rather than answering only for the full line, because the caller
    /// that needs it is sizing a **table cell**, which gets a share of the width and
    /// then pays inset out of that share. Answering only `available_pt()` invited the
    /// caller to divide, and dividing is where the inset gets lost.
    ///
    /// Lives here rather than in `convert` because it is a fact about the template's
    /// geometry. It was a hardcoded `96` until 2026-08-23, which silently encoded "A4 at
    /// 10pt" into the conversion layer and was wrong at any other size.
    pub fn chars_in(&self, width_pt: f64) -> usize {
        let per_char = self.base_size_pt * 0.503;
        (width_pt.max(0.0) / per_char) as usize
    }

    /// Available width once the page is flipped — what a rotated Element is
    /// re-measured against.
    ///
    /// Arithmetic, not a second layout: flipping swaps the page dimensions, so the
    /// width becomes the page height less the same margins. Both passes read it from
    /// here rather than each knowing the rule.
    pub fn available_landscape_pt(&self) -> f64 {
        self.page_height_pt - 2.0 * self.margin_pt
    }
}
