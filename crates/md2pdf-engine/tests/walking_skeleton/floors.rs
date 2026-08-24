//! T26c — the evidence for choosing the comfort floor, from real documents.
//!
//! `table_comfort_pt` answers one question: **at what size does wrapping read better than
//! shrinking further?** It cannot be derived. `design/plan-floors.md` says it is chosen
//! by eye against real pages, and this produces the pages.
//!
//! ```text
//! # which real tables sit at each candidate boundary
//! cargo test --release -p md2pdf-engine --test walking_skeleton \
//!     candidates_for_the_comfort_floor -- --ignored --nocapture
//!
//! # render one of them both ways
//! PAIR_AT=8.5 PAIR_OUT=/tmp/pair cargo test --release -p md2pdf-engine \
//!     --test walking_skeleton render_the_pair -- --ignored --nocapture
//! ```
//!
//! **The floor is dropped to 7.0 while scanning**, so a table reports the size it *would*
//! shrink to rather than the reflow the current floor turns it into. That is the number
//! the choice is about.

use md2pdf_domain::{Element, Reduction, Template};
use md2pdf_typeset::Typesetter;

/// A template that lets the ladder shrink as far as it likes, so the natural landing
/// size is observable.
pub fn unfloored() -> Template {
    let mut t = Template::default();
    t.floors.table_comfort_pt = 0.0;
    t
}

/// The size this element would shrink to if nothing stopped it.
pub fn shrink_target(el: &Element, template: &Template) -> Option<f64> {
    let ts = Typesetter::new();
    let (d, _) = ts.probe(std::slice::from_ref(el), template).ok()?;
    match d.get(&el.id)?.reduction {
        Reduction::Shrink { size_pt } => Some(size_pt),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use md2pdf_convert::{convert, SourceContext};
    use md2pdf_domain::{ElementClass, Markup};
    use std::collections::BTreeMap;

    /// Every reflow-capable table in the corpus, by the size it would shrink to.
    #[test]
    #[ignore = "needs documents/; evidence for T26c"]
    fn candidates_for_the_comfort_floor() {
        let broker = md2pdf_paths::PathBroker::new();
        let template = unfloored();
        let mut by_size: BTreeMap<String, Vec<(String, usize, u32)>> = BTreeMap::new();

        for source in broker
            .walk(&std::path::PathBuf::from("/workspace/documents"))
            .expect("corpus")
            .sources
        {
            let Ok(markdown) = broker.read_to_string(&source) else {
                continue;
            };
            let parent = source.parent().unwrap().to_path_buf();
            let images = super::super::census::CorpusImages;
            let conversion = convert(&markdown, &SourceContext::new(&parent, &images));
            for el in &conversion.elements {
                if el.class != ElementClass::Table || el.reflow.is_none() {
                    continue;
                }
                let Some(size) = shrink_target(el, &template) else {
                    continue;
                };
                // Column count, so a shape can be named rather than guessed at.
                let cols = el
                    .reflow
                    .as_ref()
                    .and_then(|m| {
                        let s = m.as_str();
                        let i = s.find("let n = ")? + 8;
                        s[i..].split_whitespace().next()?.parse::<usize>().ok()
                    })
                    .unwrap_or(0);
                by_size.entry(format!("{size:.1}")).or_default().push((
                    source.file_name().unwrap().to_string_lossy().to_string(),
                    cols,
                    el.id.order,
                ));
            }
        }

        for (size, tables) in &by_size {
            println!("\n{size}pt — {} tables", tables.len());
            let mut shapes: BTreeMap<usize, usize> = BTreeMap::new();
            for (_, c, _) in tables {
                *shapes.entry(*c).or_default() += 1;
            }
            println!("  shapes (columns: count): {shapes:?}");
            for (f, c, o) in tables.iter().take(4) {
                println!("    {c} cols  order {o:>3}  {f}");
            }
        }
    }

    /// Render one real table both ways it could go: shrunk, and reflowed at full size.
    ///
    /// The whole question, made lookable-at. Two files, same table, same page geometry.
    #[test]
    #[ignore = "needs documents/; evidence for T26c"]
    fn render_the_pair() {
        let want: f64 = std::env::var("PAIR_AT")
            .expect("PAIR_AT")
            .parse()
            .expect("a size");
        let out = std::env::var("PAIR_OUT").expect("PAIR_OUT");
        let only: Option<String> = std::env::var("PAIR_FILE").ok();

        let broker = md2pdf_paths::PathBroker::new();
        let template = unfloored();

        for source in broker
            .walk(&std::path::PathBuf::from("/workspace/documents"))
            .expect("corpus")
            .sources
        {
            if let Some(f) = &only {
                if !source.to_string_lossy().contains(f.as_str()) {
                    continue;
                }
            }
            let Ok(markdown) = broker.read_to_string(&source) else {
                continue;
            };
            let parent = source.parent().unwrap().to_path_buf();
            let images = super::super::census::CorpusImages;
            let conversion = convert(&markdown, &SourceContext::new(&parent, &images));
            for el in &conversion.elements {
                if el.class != ElementClass::Table || el.reflow.is_none() {
                    continue;
                }
                if shrink_target(el, &template) != Some(want) {
                    continue;
                }

                // A: shrunk to the size the ladder would choose.
                write_one(
                    el,
                    &template,
                    &format!("{out}-{want}-A-shrunk.png"),
                    &broker,
                );
                // B: the same table reflowed instead — its alternate at full size.
                let reflowed = Element::new(
                    el.id.order,
                    el.class,
                    Markup::raw(el.reflow.as_ref().unwrap().as_str().to_string()),
                );
                write_one(
                    &reflowed,
                    &template,
                    &format!("{out}-{want}-B-reflowed.png"),
                    &broker,
                );
                println!(
                    "pair at {want}pt from {}",
                    source.file_name().unwrap().to_string_lossy()
                );
                return;
            }
        }
        println!("no table found at {want}pt");
    }

    fn write_one(el: &Element, template: &Template, path: &str, broker: &md2pdf_paths::PathBroker) {
        let ts = Typesetter::new();
        let (d, _) = ts.probe(std::slice::from_ref(el), template).expect("probe");
        let c = ts
            .render(std::slice::from_ref(el), template, &d)
            .expect("render");
        let (w, h, rgba) = c.raster(0, 2.0).expect("raster");
        broker
            .overwrite(
                std::path::Path::new(path),
                &super::super::look::png(w, h, &rgba),
            )
            .expect("write");
        println!("  wrote {path}");
    }

    /// §6's baseline, counted without the batch.
    ///
    /// `compromise-mechanism.md` §9 requires every ladder change to re-measure §6, and
    /// its recipe is the release CLI over `documents/`. That batch is currently
    /// OOM-killed at ~141 of 146 on this machine — **F3**, and it kills an unmodified
    /// binary too. This walks the same corpus a document at a time with a fresh
    /// `Typesetter` each, so the memory never accumulates, and counts the same events.
    #[test]
    #[ignore = "needs documents/; the §6 baseline"]
    fn recount_the_baseline() {
        let broker = md2pdf_paths::PathBroker::new();
        let template = Template::default();
        let mut kinds: BTreeMap<String, usize> = BTreeMap::new();
        let mut sizes: BTreeMap<String, usize> = BTreeMap::new();
        let (mut docs, mut flagged, mut elements) = (0usize, 0usize, 0usize);

        for source in broker
            .walk(&std::path::PathBuf::from("/workspace/documents"))
            .expect("corpus")
            .sources
        {
            let Ok(markdown) = broker.read_to_string(&source) else {
                continue;
            };
            let parent = source.parent().unwrap().to_path_buf();
            let images = super::super::census::CorpusImages;
            let conversion = convert(&markdown, &SourceContext::new(&parent, &images));
            docs += 1;
            elements += conversion.elements.len();

            let ts = Typesetter::new();
            let Ok((decisions, _)) = ts.probe(&conversion.elements, &template) else {
                continue;
            };
            let mut here = 0usize;
            for el in &conversion.elements {
                let Some(d) = decisions.get(&el.id) else {
                    continue;
                };
                let name = match d.reduction {
                    md2pdf_domain::Reduction::None => continue,
                    md2pdf_domain::Reduction::Shrink { size_pt } => {
                        *sizes.entry(format!("{size_pt:.1}")).or_default() += 1;
                        "shrunk"
                    }
                    md2pdf_domain::Reduction::Reflow => "reflowed",
                    md2pdf_domain::Reduction::Scale { .. } => "scaled",
                    md2pdf_domain::Reduction::Clip => "CLIPPED",
                };
                *kinds.entry(name.to_string()).or_default() += 1;
                here += 1;
            }
            // Conversion-time compromises (unsupported, images) count too.
            here += conversion.compromises.len();
            for c in &conversion.compromises {
                *kinds
                    .entry(
                        format!("{:?}", c.kind)
                            .split(' ')
                            .next()
                            .unwrap()
                            .to_lowercase(),
                    )
                    .or_default() += 1;
            }
            if here > 0 {
                flagged += 1;
            }
        }
        println!("\n{docs} documents, {elements} elements");
        println!("{flagged} flagged ({}%)", 100 * flagged / docs.max(1));
        println!("kinds: {kinds:?}");
        println!("shrink targets: {sizes:?}");
    }
}
