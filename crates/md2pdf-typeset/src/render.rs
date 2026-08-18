//! RenderPass — act, never measure.
//!
//! Decisions are baked into the markup md2pdf generates. They are deliberately NOT
//! passed to the Template: a Template is a print stylesheet, and decisions are
//! structure, not style. Making template authors handle decision logic would break
//! that separation. See `design/GLOSSARY.md`, "Template".

use md2pdf_domain::{DecisionMap, Element, Rung, Template};

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
        let rung = map.get(&el.id).map(|d| d.rung).unwrap_or(Rung::None);
        match rung {
            Rung::None => {
                s.push_str(&format!("{}\n#v(0.65em)\n", el.body));
            }
            Rung::Shrink { size_pt } => {
                if el.class.shrinks_by_scale() {
                    let factor = (size_pt / template.base_size_pt).clamp(0.05, 1.0);
                    s.push_str(&format!(
                        "#scale({:.1}%)[{}]\n#v(0.65em)\n",
                        factor * 100.0,
                        el.body
                    ));
                } else {
                    s.push_str(&format!(
                        "#text(size: {size_pt}pt)[{}]\n#v(0.65em)\n",
                        el.body
                    ));
                }
            }
            Rung::Rotate => {
                // Legal at top level. Illegal inside `layout()`, which is one reason
                // the ProbePass does not use it.
                //
                // No size is carried over from the probe: landscape offers far more
                // width, so the element is re-measured here at base size.
                s.push_str(&format!(
                    "#page(flipped: true, margin: {}pt)[{}]\n",
                    template.margin_pt, el.body
                ));
            }
            Rung::Clip => {
                s.push_str(&format!(
                    "#block(width: 100%, clip: true)[{}]\n\
                     #text(size: 7pt, fill: red)[[clipped]]\n#v(0.65em)\n",
                    el.body
                ));
            }
        }
    }
    s
}
