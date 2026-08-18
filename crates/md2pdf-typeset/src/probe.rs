//! ProbePass — measure and decide. Never acts, never paginates.
//!
//! Deliberately does NOT call `layout()`. Available width comes from the Template
//! (page minus margins), which is 3.6x cheaper than probing through a full document
//! layout and yields identical decisions. `layout()` was also the only construct
//! that made the two-pass split mandatory; the split is now kept for cost, not
//! legality. See `design/spike-typst-measure-findings.md`.

use std::fmt::Write;

use md2pdf_domain::{Element, Template};

/// Assembles the measure-only harness: elements are measured but never placed in
/// the page flow.
pub fn probe_source(elements: &[Element], template: &Template) -> String {
    let avail = template.available_pt();
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
        let floor = template.floors.for_class(el.class);

        if el.class.is_atomic() {
            // Linear scan in 0.5pt steps: ~7 measurements per atomic element, and it
            // dominates probe cost. A binary search would cut it to ~3 — noted in the
            // findings, not taken yet.
            write!(
                s,
                r#"  {{
    let body = [{body}]
    let natural = measure(body).width
    let chosen = none
    let s = {base}pt
    while s >= {floor}pt and chosen == none {{
      if measure(text(size: s, body)).width <= {avail}pt {{ chosen = s }}
      s = s - 0.5pt
    }}
    let rung = if chosen == {base}pt {{ "none" }} else if chosen != none {{ "shrink" }} else {{ "rotate" }}
    [#metadata((
      order: {order},
      rung: rung,
      size: if chosen == none {{ 0.0 }} else {{ chosen.pt() }},
      natural: natural.pt(),
      available: {avail},
    )) <d>]
  }}
"#,
                body = el.body,
            )
            .expect("string write");
        } else {
            // Wrappable: reflows to any width, so it cannot overflow horizontally and
            // the ladder is skipped. Measured anyway so the Diagnostic carries a real
            // natural width.
            write!(
                s,
                r#"  {{
    let body = [{body}]
    [#metadata((
      order: {order},
      rung: "none",
      size: 0.0,
      natural: measure(body).width.pt(),
      available: {avail},
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
