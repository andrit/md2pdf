//! Pull ProbePass decisions back out of a compiled document.
//!
//! This is the library equivalent of `typst query` / `typst eval`, which are CLI-only
//! features md2pdf cannot use. The introspector is the in-process query surface.

use std::collections::BTreeMap;

use md2pdf_domain::{Decision, DecisionMap, Element, Orientation, Reduction};
use typst::introspection::{Introspector, MetadataElem};
use typst_layout::PagedDocument;

use crate::TypesetError;

/// Which page each Element landed on, from the RenderPass's `<md2pdf-el>` markers.
///
/// **Silent about what it cannot answer.** A marker with no location, or a location the
/// introspector cannot place, is skipped rather than defaulted — `PagedPosition::ORIGIN`
/// would quietly claim page 1 for it, and a page reference that is confidently wrong is
/// worse than none. The read model shows a page only where there is one.
pub fn element_pages(doc: &PagedDocument) -> BTreeMap<u32, u32> {
    let introspector = doc.introspector();
    introspector
        .query_labelled()
        .iter()
        .filter_map(|c| {
            let meta = c.to_packed::<MetadataElem>()?;
            let order = serde_json::to_value(&meta.value).ok()?.as_u64()? as u32;
            let page = introspector.position(c.location()?)?.page.get() as u32;
            Some((order, page))
        })
        .collect()
}

pub fn harvest(doc: &PagedDocument, elements: &[Element]) -> Result<DecisionMap, TypesetError> {
    let raw: Vec<serde_json::Value> = doc
        .introspector()
        .query_labelled()
        .iter()
        .filter_map(|c| c.to_packed::<MetadataElem>())
        .map(|m| serde_json::to_value(&m.value))
        .collect::<Result<_, _>>()
        .map_err(|e| TypesetError::Harvest(e.to_string()))?;

    let mut decisions = Vec::with_capacity(raw.len());
    for v in &raw {
        let order = v["order"].as_u64().ok_or_else(|| bad(v, "order"))? as u32;
        let element = elements
            .iter()
            .find(|e| e.id.order == order)
            .ok_or_else(|| TypesetError::Harvest(format!("no element for order {order}")))?;

        let size_pt = v["size"].as_f64().unwrap_or(0.0);
        let factor = v["factor"].as_f64().unwrap_or(1.0);

        let orientation = match v["orientation"]
            .as_str()
            .ok_or_else(|| bad(v, "orientation"))?
        {
            "portrait" => Orientation::Portrait,
            "landscape" => Orientation::Landscape,
            other => {
                return Err(TypesetError::Harvest(format!(
                    "unknown orientation {other:?}"
                )))
            }
        };
        let reduction = match v["reduction"].as_str().ok_or_else(|| bad(v, "reduction"))? {
            "none" => Reduction::None,
            "shrink" => Reduction::Shrink { size_pt },
            "scale" => Reduction::Scale { factor },
            "reflow" => Reduction::Reflow,
            "clip" => Reduction::Clip,
            other => {
                return Err(TypesetError::Harvest(format!(
                    "unknown reduction {other:?}"
                )))
            }
        };

        decisions.push(Decision {
            id: element.id,
            orientation,
            reduction,
            natural_pt: v["natural"].as_f64().ok_or_else(|| bad(v, "natural"))?,
            available_pt: v["available"].as_f64().ok_or_else(|| bad(v, "available"))?,
        });
    }
    decisions.sort_by_key(|d| d.id.order);
    Ok(DecisionMap { decisions })
}

fn bad(v: &serde_json::Value, field: &str) -> TypesetError {
    TypesetError::Harvest(format!("probe metadata missing {field}: {v}"))
}
