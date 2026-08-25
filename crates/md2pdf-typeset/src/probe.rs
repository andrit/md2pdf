//! ProbePass — measure and decide. Never acts, never paginates.
//!
//! Deliberately does NOT call `layout()`. Available width comes from the Template
//! (page minus margins), which is 3.6x cheaper than probing through a full document
//! layout and yields identical decisions. `layout()` was also the only construct
//! that made the two-pass split mandatory; the split is now kept for cost, not
//! legality. See `design/spike-typst-measure-findings.md`.
//!
//! ## Both axes are decided here
//!
//! The RenderPass never measures, so **every** measurement-dependent choice is made in
//! this pass — including what happens *after* a rotation. The GLOSSARY requires it:
//!
//! > RE-MEASURE in landscape; do not inherit the portrait size.
//! > Carrying the Floor size over is a bug.
//!
//! So an Element that will not fit portrait is measured a second time against the
//! landscape width, and reduction is chosen there. Only when even that fails does it
//! clip. The second pass costs nothing in the common case, because it runs only for
//! Elements that already failed the first.

use std::fmt::Write;

use md2pdf_domain::{Element, Override, Permit, Template};

/// Assembles the measure-only harness: elements are measured but never placed in
/// the page flow.
/// Assembles the measure-only harness, optionally under user Overrides.
///
/// **An Override changes the ladder's *constraints*, not its answer** — so the size that
/// comes back is measured under the new permission rather than inherited from the old
/// decision. That is the whole of Option B in `design/plan-review.md`, and the reason
/// `DecisionMap::apply_override` was retired.
///
/// Only the constrained Element differs; everything else is generated exactly as before,
/// which is what keeps this from becoming a second ladder to keep in step with the first.
/// Pass an empty slice for the ordinary case.
pub fn probe_source_with(
    elements: &[Element],
    template: &Template,
    overrides: &[Override],
) -> String {
    let avail = template.available_pt();
    let land = template.available_landscape_pt();
    let base = template.base_size_pt;
    let mut s = String::new();

    // `height: auto` and no body content — nothing here paginates.
    writeln!(
        s,
        "#set page(width: {w}pt, height: auto, margin: {m}pt)\n\
         #set text(font: \"{body}\", size: {base}pt)\n\
         #show raw: set text(font: \"{mono}\")\n\
         #context {{",
        w = template.page_width_pt,
        m = template.margin_pt,
        body = template.font_body,
        mono = template.font_mono,
    )
    .expect("string write");

    for el in elements {
        let order = el.id.order;
        let permit = overrides
            .iter()
            .find(|o| o.id.matches(&el.id))
            .map(|o| o.permit);

        if !el.class.is_atomic() {
            // Wrappable: reflows to any width, so it cannot overflow horizontally and
            // the ladder is skipped. Measured anyway so the Diagnostic carries a real
            // natural width.
            write!(
                s,
                r#"  {{
    let body = [{body}]
    [#metadata((
      order: {order},
      orientation: "portrait",
      reduction: "none",
      size: 0.0,
      factor: 1.0,
      natural: measure(body).width.pt(),
      available: {avail},
    )) <d>]
  }}
"#,
                body = el.body,
            )
            .expect("string write");
        } else if el.class.shrinks_by_scale() {
            // Scaling content — images. The required factor is arithmetic
            // (available / natural); stepping font size cannot move an image at all.
            write!(
                s,
                r#"  {{
    let body = [{body}]
    let natural = measure(body).width
    let needed(space) = if natural > 0pt {{ space / natural }} else {{ 1.0 }}
    let orientation = "portrait"
    let reduction = "none"
    let factor = 1.0
    let available = {avail}
    let p = needed({avail}pt)
    if p >= 1.0 {{
    }} else if p >= {floor} {{
      reduction = "scale"
      factor = p
    }} else {{
      orientation = "landscape"
      available = {land}
      let l = needed({land}pt)
      if l >= 1.0 {{
      }} else if l >= {floor} {{
        reduction = "scale"
        factor = l
      }} else {{
        reduction = "clip"
      }}
    }}
    [#metadata((
      order: {order},
      orientation: orientation,
      reduction: reduction,
      size: 0.0,
      factor: factor,
      natural: natural.pt(),
      available: available,
    )) <d>]
  }}
"#,
                body = el.body,
                floor = template.floors.image_scale,
            )
            .expect("string write");
        } else {
            // Text-bearing atomic content — tables. Linear scan in 0.5pt steps: ~7
            // measurements per element per orientation, and it dominates probe cost.
            // A binary search would cut it to ~3 — noted in the findings, not taken.
            let floor = template.floors.for_class(el.class);
            // A Permit lowers the floor rather than choosing a size. `to_pt` is a bound
            // the user is willing to go down to; the ladder still stops at the first size
            // that actually fits, which is usually well above it.
            let floor = match permit {
                Some(Permit::BelowFloor { to_pt }) => to_pt.min(floor),
                _ => floor,
            };
            // The last rung. An Element with an alternate form reflows instead of
            // losing content; only one with nothing to fall back on clips.
            let last_rung = if matches!(permit, Some(Permit::Clip)) {
                // Asked for explicitly, and the only Permit that destroys content — so
                // it is never reached unless a person chose it.
                "clip"
            } else if el.reflow.is_some() {
                "reflow"
            } else {
                "clip"
            };
            // An Element that can wrap prefers wrapping to being squeezed small, and
            // prefers it to a landscape page of its own (T26b, option O2a).
            //
            // The comfort floor is where that preference turns over: above it the
            // squeeze is imperceptible and the author's column proportions are worth
            // keeping; below it the table reads better at full size with its cells
            // wrapping. Rendered side by side before choosing — `design/evidence/t26a2/`.
            //
            // Rotation becomes an image-only rung as a result: every table in the corpus
            // that rotated could wrap instead, and wrapping costs no page turn.
            //
            // An Element with no alternate keeps the original ladder exactly: a comfort
            // floor of zero makes the test `p >= 0pt` vacuous, so it reduces to
            // "shrink if anything fits".
            let has_alternate = el.reflow.is_some();
            let comfort = if has_alternate {
                template.floors.table_comfort_pt
            } else {
                0.0
            };
            // Permitting a size below the floor also lifts the *comfort* floor out of the
            // way. They answer different questions — "how small before we give up" and
            // "how small before wrapping reads better" — but a user who has asked for
            // smaller text has answered the second one themselves.
            let comfort = match permit {
                Some(Permit::BelowFloor { .. }) => 0.0,
                _ => comfort,
            };
            // Forcing landscape means measuring against the landscape width *from the
            // start*, not turning a portrait decision sideways. Carrying the portrait
            // size over is the bug the GLOSSARY names and T14 fixed; this is where not
            // repeating it is enforced.
            let forced_landscape = matches!(permit, Some(Permit::Landscape));
            let (start_avail, start_orientation) = if forced_landscape {
                (land, "landscape")
            } else {
                (avail, "portrait")
            };
            // With landscape already taken, there is no wider width left to escalate to.
            let escalates = !forced_landscape;
            write!(
                s,
                r#"  {{
    let body = [{body}]
    let natural = measure(body).width
    let choose(space) = {{
      let chosen = none
      let s = {base}pt
      while s >= {floor}pt and chosen == none {{
        if measure(text(size: s, body)).width <= space {{ chosen = s }}
        s = s - 0.5pt
      }}
      chosen
    }}
    let orientation = "{start_orientation}"
    let reduction = "none"
    let size = 0.0
    let available = {start_avail}
    let p = choose({start_avail}pt)
    if p == {base}pt {{
    }} else if p != none and p >= {comfort}pt {{
      reduction = "shrink"
      size = p.pt()
    }} else if {has_alternate} and {escalates} {{
      reduction = "reflow"
    }} else if {escalates} {{
      orientation = "landscape"
      available = {land}
      let l = choose({land}pt)
      if l == {base}pt {{
      }} else if l != none {{
        reduction = "shrink"
        size = l.pt()
      }} else {{
        reduction = "{last_rung}"
      }}
    }} else {{
      reduction = "{last_rung}"
    }}
    [#metadata((
      order: {order},
      orientation: orientation,
      reduction: reduction,
      size: size,
      factor: 1.0,
      natural: natural.pt(),
      available: available,
    )) <d>]
  }}
"#,
                body = el.body,
            )
            .expect("string write");
        }
    }

    s.push_str("}\n");
    s
}
