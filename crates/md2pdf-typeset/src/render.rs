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
        let reduced = reduce(&el.body.to_string(), decision.reduction, template);
        match decision.orientation {
            Orientation::Portrait => {
                s.push_str(&format!("{reduced}\n#v(0.65em)\n"));
            }
            Orientation::Landscape => {
                // Legal at top level. Illegal inside `layout()`, which is one reason
                // the ProbePass does not use it. The size was already chosen against
                // the landscape width by the probe — nothing is re-measured here,
                // because the RenderPass never measures.
                s.push_str(&format!(
                    "#page(flipped: true, margin: {}pt)[{reduced}]\n",
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
        Reduction::Clip => format!(
            "#block(width: 100%, clip: true)[{body}]\n\
             #text(size: {}pt, fill: red)[[clipped]]",
            template.floors.table_pt
        ),
    }
}
