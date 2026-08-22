//! The overflow oracle — *did this element run off the page?*
//!
//! Typst does not report overflow. It lays content out, and anything too wide simply
//! extends past the margin: no error, no warning, nothing in the document tree to
//! inspect. So the only way to answer the question is to **render the page and look at
//! the pixels**.
//!
//! That had been written as throwaway code four times before this file existed — the
//! T26a clip triage, the T26a2 column-spec experiment, the T26b exposure measurement,
//! and T29's feasibility check — and thrown away each time. Recorded as flag **F1**.
//!
//! ## Why it earns its place
//!
//! After T26b, **152 of 198 compromised elements sit on the reflow rung**, and reflow is
//! the rung that can overflow *silently*: the ladder records `Reflowed`, which every
//! consumer reads as handled, while the table runs off the page and its cells overprint
//! each other. `CompromiseKind` cannot tell those apart, and no golden hash can either —
//! a hash says the bytes changed, not that the output is wrong.
//!
//! This is the only check in the project that judges the *result* rather than the
//! decision.

use md2pdf_domain::{Element, Template};
use md2pdf_typeset::{Compilation, Typesetter};

/// Ink found beyond the text area, and where.
#[derive(Debug, PartialEq)]
pub struct Overflow {
    pub page: usize,
    /// How far past the right margin, in points.
    pub past_margin_pt: f64,
}

/// Anything darker than this counts as ink. Antialiased edges are much lighter.
const INK: u8 = 200;

/// Look for ink to the right of the text area, on **every** page.
///
/// Every page, deliberately: an earlier throwaway checked page 1 only, which made its
/// counts a floor rather than a measurement. A table overflowing solely on a
/// continuation page would have been missed.
pub fn overflow(compilation: &Compilation, template: &Template) -> Option<Overflow> {
    // Rendering at 1 pixel per point keeps the pixel/point arithmetic honest.
    let ppp = 1.0;
    for page in 0..compilation.page_count() {
        let Some((w, h, bytes)) = compilation.raster(page, ppp) else {
            continue;
        };
        // A landscape page is wider; take the limit from the raster rather than assuming
        // portrait, or every rotated element reads as an overflow.
        let page_width_pt = f64::from(w) / f64::from(ppp);
        let limit_pt = page_width_pt - template.margin_pt;
        // One point of tolerance: a glyph may be antialiased across the boundary.
        let limit_px = ((limit_pt + 1.0) * f64::from(ppp)) as usize;
        if limit_px >= w as usize {
            continue;
        }

        let mut furthest = 0usize;
        for y in 0..h as usize {
            for x in limit_px..w as usize {
                let i = (y * w as usize + x) * 4;
                if bytes[i] < INK && bytes[i + 1] < INK && bytes[i + 2] < INK {
                    furthest = furthest.max(x);
                }
            }
        }
        if furthest > 0 {
            return Some(Overflow {
                page,
                past_margin_pt: (f64::from(furthest as u32) / f64::from(ppp)) - limit_pt,
            });
        }
    }
    None
}

/// Render these elements as the ladder decided, and ask whether anything spilled.
pub fn overflow_of(elements: &[Element], template: &Template) -> Option<Overflow> {
    let typesetter = Typesetter::new();
    let (decisions, _) = typesetter.probe(elements, template).expect("probe");
    let compilation = typesetter
        .render(elements, template, &decisions)
        .expect("render");
    overflow(&compilation, template)
}

#[cfg(test)]
mod tests {
    use super::*;
    use md2pdf_convert::{convert, SourceContext};
    use md2pdf_domain::Markup;

    fn table_of(cell: &str) -> String {
        let row = format!("| {} |\n", [cell; 5].join(" | "));
        format!("| a | b | c | d | e |\n|---|---|---|---|---|\n{row}{row}")
    }

    /// The negative control. An oracle that never fires is not an oracle, and this is
    /// the exact shape T29 fixes — five columns of an unbreakable identifier, rendered
    /// with the break opportunities stripped back out.
    #[test]
    fn the_oracle_detects_an_overflow_that_is_really_there() {
        let template = Template::default();
        let conversion = convert(
            &table_of("`completeSubmissionWithAVeryLongIdentifier`"),
            &SourceContext::none(),
        );
        // Strip the breaks from the alternate: this is the pre-T29 rendering.
        let elements: Vec<Element> = conversion
            .elements
            .iter()
            .map(|el| match &el.reflow {
                Some(alt) => Element::with_reflow(
                    el.id.order,
                    el.class,
                    el.body.clone(),
                    Markup::raw(alt.as_str().replace('\u{200b}', "")),
                ),
                None => el.clone(),
            })
            .collect();
        assert!(
            overflow_of(&elements, &template).is_some(),
            "the oracle missed an overflow it was written to catch"
        );
    }

    /// And the fix. Same table, breaks left in.
    #[test]
    fn an_unbreakable_run_no_longer_runs_off_the_page() {
        let template = Template::default();
        let conversion = convert(
            &table_of("`completeSubmissionWithAVeryLongIdentifier`"),
            &SourceContext::none(),
        );
        assert_eq!(
            overflow_of(&conversion.elements, &template),
            None,
            "a table with long unbreakable content still overflows"
        );
    }

    /// Long paths in plain text, the other measured source of unbreakable runs.
    #[test]
    fn a_long_path_no_longer_runs_off_the_page() {
        let template = Template::default();
        let conversion = convert(
            &table_of("/Users/someone/Documents/Projects/deeply/nested/path/file.md"),
            &SourceContext::none(),
        );
        assert_eq!(overflow_of(&conversion.elements, &template), None);
    }

    /// Every fixture in the committed corpus fits. The census says which rung each one
    /// took; this says the result is actually on the page.
    #[test]
    fn no_corpus_fixture_overflows() {
        let broker = md2pdf_paths::PathBroker::new();
        let template = Template::default();
        let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/corpus");
        for source in broker.walk(&dir).expect("corpus").sources {
            let markdown = broker.read_to_string(&source).expect("fixture");
            let images = super::super::census::CorpusImages;
            let conversion = convert(&markdown, &SourceContext::new(&dir, &images));
            let typesetter = Typesetter::new();
            for (name, path) in &conversion.images {
                if let Ok(bytes) = broker.read_bytes(path) {
                    typesetter.add_file(name, bytes);
                }
            }
            let (decisions, _) = typesetter
                .probe(&conversion.elements, &template)
                .expect("probe");
            let compilation = typesetter
                .render(&conversion.elements, &template, &decisions)
                .expect("render");
            assert_eq!(
                overflow(&compilation, &template),
                None,
                "{} spills off the page",
                source.file_name().unwrap().to_string_lossy()
            );
        }
    }
}

/// The oracle pointed at a real directory rather than the fixtures.
///
/// Ignored: it needs `documents/`, which is untracked (**F5**), and takes ~3 minutes.
/// This is the half of **F1** that answers "how bad is it out there", as opposed to the
/// fixture tests above which answer "did we regress".
#[cfg(test)]
mod corpus_check {
    use super::*;
    use md2pdf_convert::{convert, SourceContext};

    #[test]
    #[ignore = "needs documents/; ~3 minutes"]
    fn how_many_real_elements_still_overflow() {
        let broker = md2pdf_paths::PathBroker::new();
        let template = Template::default();
        let (mut capable, mut reflowed, mut over) = (0usize, 0usize, 0usize);
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
                if el.reflow.is_none() {
                    continue;
                }
                capable += 1;
                let one = vec![el.clone()];
                let typesetter = Typesetter::new();
                // The ladder's own decision, not a forced one: this is what ships.
                let Ok((d, _)) = typesetter.probe(&one, &template) else {
                    continue;
                };
                if !matches!(
                    d.get(&el.id).map(|d| d.reduction),
                    Some(md2pdf_domain::Reduction::Reflow)
                ) {
                    continue;
                }
                reflowed += 1;
                let Ok(c) = typesetter.render(&one, &template, &d) else {
                    continue;
                };
                if let Some(o) = overflow(&c, &template) {
                    over += 1;
                    println!(
                        "  OVERFLOW {:>6.1}pt  {}",
                        o.past_margin_pt,
                        source.file_name().unwrap().to_string_lossy()
                    );
                }
            }
        }
        println!("\n{capable} tables can reflow, {reflowed} do, {over} of those overflow");
    }
}
