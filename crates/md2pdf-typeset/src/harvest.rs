//! Pull ProbePass decisions back out of a compiled document.
//!
//! This is the library equivalent of `typst query` / `typst eval`, which are CLI-only
//! features md2pdf cannot use. The introspector is the in-process query surface.

use md2pdf_domain::{Decision, DecisionMap, Element, Rung};
use typst::introspection::{Introspector, MetadataElem};
use typst_layout::PagedDocument;

use crate::TypesetError;

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
        let rung = match v["rung"].as_str().ok_or_else(|| bad(v, "rung"))? {
            "none" => Rung::None,
            "shrink" => Rung::Shrink { size_pt },
            "scale" => Rung::Scale { factor },
            "rotate" => Rung::Rotate,
            "clip" => Rung::Clip,
            other => return Err(TypesetError::Harvest(format!("unknown rung {other:?}"))),
        };

        decisions.push(Decision {
            id: element.id,
            rung,
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
