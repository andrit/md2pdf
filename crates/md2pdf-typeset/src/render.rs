//! RenderPass — act, never measure.
//!
//! Decisions are baked into the markup md2pdf generates. They are deliberately NOT
//! passed to the Template: a Template is a print stylesheet, and decisions are
//! structure, not style. Making template authors handle decision logic would break
//! that separation. See `design/GLOSSARY.md`, "Template".

use md2pdf_domain::{Decision, DecisionMap, Element, Orientation, Reduction, Template};

pub fn render_source(elements: &[Element], template: &Template, map: &DecisionMap) -> String {
    let mut s = String::new();
    s.push_str(&format!(
        "#set page(width: {}pt, height: {}pt, margin: {}pt)\n\
         #set text(font: \"{}\", size: {}pt)\n\
         #show raw: set text(font: \"{}\")\n",
        template.page_width_pt,
        template.page_height_pt,
        template.margin_pt,
        template.font_body,
        template.base_size_pt,
        template.font_mono
    ));

    for el in elements {
        let default = Decision::fits(el.id, 0.0, 0.0);
        let decision = map.get(&el.id).unwrap_or(&default);

        // The two axes compose: reduce the body, then place it. Neither step needs to
        // know what the other chose, which is the point of splitting them.
        // Reflow swaps the body for the Element's always-fitting alternate rather than
        // transforming what is there — the difference between a table that wraps and a
        // table that has lost its right-hand columns.
        let source = match (decision.reduction, &el.reflow) {
            (Reduction::Reflow, Some(alternate)) => alternate.to_string(),
            _ => el.body.to_string(),
        };
        let reduced = reduce(&source, decision.reduction, template);
        // A zero-size marker carrying the Element's order, so the compiled document can
        // be asked which page each Element landed on (`Compilation::element_pages`).
        // `metadata` draws nothing and occupies nothing; it exists to be queried.
        //
        // **A sibling, never a wrapper.** The ProbePass wraps elements in containers and
        // that is exactly why `#set page` is illegal inside it — the RenderPass is where
        // page-level markup has to keep working, so nothing here may enclose the body.
        let marker = format!("#metadata({})<md2pdf-el>", el.id.order);
        match decision.orientation {
            Orientation::Portrait => {
                // Before the body: the page an Element *starts* on is the one worth
                // being sent to. An Element that breaks across pages reports the first.
                s.push_str(&format!("{marker}\n{reduced}\n#v(0.65em)\n"));
            }
            Orientation::Landscape => {
                // Legal at top level. Illegal inside `layout()`, which is one reason
                // the ProbePass does not use it. The size was already chosen against
                // the landscape width by the probe — nothing is re-measured here,
                // because the RenderPass never measures.
                // *Inside* the flipped page, not before it: `#page()` starts a new page,
                // so a marker placed before this one would report the page before the
                // element rather than the element's own.
                s.push_str(&format!(
                    "#page(flipped: true, margin: {}pt)[{marker}{reduced}]\n",
                    template.margin_pt
                ));
            }
        }
    }
    s
}

fn reduce(body: &str, reduction: Reduction, template: &Template) -> String {
    match reduction {
        Reduction::None => body.to_string(),
        Reduction::Shrink { size_pt } => format!("#text(size: {size_pt}pt)[{body}]"),
        Reduction::Scale { factor } => {
            // `reflow: true` is load-bearing. Typst's scale is a visual transform by
            // default — "scales content without affecting layout" — so without it the
            // element would draw smaller while still reserving its original width,
            // leaving the overflow exactly where it was.
            let factor = factor.clamp(0.05, 1.0);
            format!("#scale({:.2}%, reflow: true)[{body}]", factor * 100.0)
        }
        // The alternate body already fits; nothing further is applied to it.
        Reduction::Reflow => body.to_string(),
        Reduction::Clip => format!(
            "#block(width: 100%, clip: true)[{body}]\n\
             #text(size: {}pt, fill: red)[[clipped]]",
            template.floors.table_pt
        ),
    }
}
