//! How wrong is the column-width **estimate**? — the measurement Option 2 rests on.
//!
//! `md2pdf-convert` sizes a reflowed table's columns from a proxy it can compute without
//! Typst: the character length of the longest cell in each column. `md2pdf-typeset` could
//! instead *measure* each cell. `design/plan-typeset-move.md` Option 2 is the change that
//! would do that, and its whole value is the gap between those two numbers.
//!
//! That gap was `[assumed]` in the plan. This measures it, so the option can be costed
//! against evidence rather than against my impression of it.
//!
//! ```text
//! cargo test -p md2pdf-engine --test walking_skeleton \
//!     how_wrong_is_the_column_estimate -- --ignored --nocapture
//! ```
//!
//! **What it compares.** The emitted alternate carries the estimator's answer directly in
//! its column spec — `#table(columns: (1fr, 6fr), …)`. The cells are in the same string.
//! So both halves are recoverable without touching `convert`'s internals: parse the spec
//! for the estimate, measure the cells for the truth, and compare the two as proportions
//! of the table's width.
//!
//! Splitting cells on `], [` is sound because `escape` escapes every bracket in document
//! text — that is what `brackets_that_would_break_the_probe_harness_are_escaped` pins.

use md2pdf_domain::{Element, ElementClass, Markup, Template};
use md2pdf_typeset::Typesetter;

/// The estimator's weights, read back out of the column spec it produced.
fn spec_weights(alternate: &str) -> Option<Vec<f64>> {
    let open = alternate.find("columns: (")? + "columns: (".len();
    let close = alternate[open..].find(')')? + open;
    alternate[open..close]
        .split(',')
        .map(|w| w.trim().trim_end_matches("fr").parse::<f64>().ok())
        .collect()
}

/// The cells, in row-major order.
fn cells(alternate: &str) -> Vec<String> {
    let Some(open) = alternate.find("columns: (") else {
        return Vec::new();
    };
    let Some(close) = alternate[open..].find(')').map(|i| i + open) else {
        return Vec::new();
    };
    let rest = alternate[close + 1..].trim_start_matches(", ");
    let rest = rest.trim_end().trim_end_matches(')').trim_end();
    let rest = rest.trim_end_matches(',');
    rest.trim_start_matches('[')
        .trim_end_matches(']')
        .split("], [")
        .map(|c| c.to_string())
        .collect()
}

/// Natural width of each cell, measured by Typst rather than counted.
fn measured(cells: &[String], template: &Template) -> Vec<f64> {
    // Prose is non-atomic, so the probe reports a natural width and skips the ladder —
    // exactly the measurement wanted, with no new Typst written.
    let elements: Vec<Element> = cells
        .iter()
        .enumerate()
        .map(|(i, c)| {
            Element::new(
                i as u32,
                ElementClass::Prose,
                Markup::raw(format!("#box[{c}]")),
            )
        })
        .collect();
    let ts = Typesetter::new();
    let Ok((decisions, _)) = ts.probe(&elements, template) else {
        return Vec::new();
    };
    elements
        .iter()
        .map(|e| decisions.get(&e.id).map(|d| d.natural_pt).unwrap_or(0.0))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use md2pdf_convert::{convert, SourceContext};

    #[test]
    #[ignore = "needs documents/; a planning measurement, not a gate"]
    fn how_wrong_is_the_column_estimate() {
        let broker = md2pdf_paths::PathBroker::new();
        let template = Template::default();
        let mut errors: Vec<f64> = Vec::new();
        let mut worst: Vec<(f64, String, usize)> = Vec::new();
        let mut tables = 0usize;
        let mut probe_ms = 0u128;
        let (mut differing, mut compared) = (0usize, 0usize);
        let t_all = std::time::Instant::now();

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
                let Some(alt) = &el.reflow else { continue };
                let alt = alt.as_str();
                let Some(weights) = spec_weights(alt) else {
                    continue;
                };
                let cs = cells(alt);
                let columns = weights.len();
                if columns == 0 || cs.len() < columns {
                    continue;
                }
                let t = std::time::Instant::now();
                let widths = measured(&cs, &template);
                probe_ms += t.elapsed().as_millis();
                if widths.len() != cs.len() {
                    continue;
                }
                tables += 1;

                // A column's real demand is its widest cell. Compare *proportions*: the
                // spec sets relative shares, so absolute points are not the question.
                let mut demand = vec![0.0f64; columns];
                for (i, w) in widths.iter().enumerate() {
                    demand[i % columns] = demand[i % columns].max(*w);
                }
                let dsum: f64 = demand.iter().sum();
                let wsum: f64 = weights.iter().sum();
                if dsum <= 0.0 || wsum <= 0.0 {
                    continue;
                }

                // Does the difference survive the *policy*? `column_spec` quantises every
                // weight into six buckets, so an estimate must be off by roughly a sixth
                // of the widest column before the emitted spec changes at all.
                let widest = demand.iter().cloned().fold(0.0f64, f64::max).max(1.0);
                let policy: Vec<f64> = demand
                    .iter()
                    .map(|d| ((d / widest * 6.0).floor()).max(1.0))
                    .collect();
                if policy != weights {
                    differing += 1;
                }
                compared += 1;

                for i in 0..columns {
                    let want = demand[i] / dsum;
                    let got = weights[i] / wsum;
                    // Error as a fraction of the width the column actually gets.
                    let err = (got - want).abs();
                    errors.push(err);
                    worst.push((
                        err,
                        source.file_name().unwrap().to_string_lossy().to_string(),
                        columns,
                    ));
                }
            }
        }

        errors.sort_by(|a, b| a.partial_cmp(b).unwrap());
        worst.sort_by(|a, b| b.0.partial_cmp(&a.0).unwrap());
        let pct = |q: f64| errors[((errors.len() - 1) as f64 * q) as usize] * 100.0;

        println!(
            "\n{tables} reflowed tables, {} columns measured in {probe_ms}ms of probing\n\
             (whole pass, convert included: {}ms)",
            errors.len(),
            t_all.elapsed().as_millis()
        );
        println!(
            "\nspec that MEASURING would change: {differing} of {compared} tables ({:.0}%)",
            100.0 * differing as f64 / compared.max(1) as f64
        );
        println!("estimated share vs measured share, absolute error in percentage points:");
        println!(
            "  p50 {:.1}   p90 {:.1}   p99 {:.1}   max {:.1}",
            pct(0.5),
            pct(0.9),
            pct(0.99),
            pct(1.0)
        );
        let over10 = errors.iter().filter(|e| **e > 0.10).count();
        println!(
            "  columns off by more than 10 points: {over10} of {} ({:.0}%)",
            errors.len(),
            100.0 * over10 as f64 / errors.len() as f64
        );
        println!("\nworst:");
        for (e, f, c) in worst.iter().take(8) {
            println!("  {:.1} points  {c} columns  {f}", e * 100.0);
        }
    }

    /// Can Typst size the columns **itself**, at layout time?
    ///
    /// If it can, the estimate is not merely wrong — it is unnecessary, and no width ever
    /// has to cross the crate boundary. Spike for `plan-typeset-move.md`.
    ///
    /// **[measured] 2026-08-23: it compiles and renders — and the probe reports its
    /// natural width as `0.0`.** `#context` defers evaluation until layout, so there is
    /// nothing for `measure()` to see at probe time. That is disqualifying for an
    /// Element *body*, because the ladder picks a rung from exactly that number and would
    /// read every self-sizing table as fitting.
    ///
    /// It is **not** obviously disqualifying for the *alternate*, which is chosen only
    /// after the body has failed and is never itself measured. Whether that holds is the
    /// one thing standing between this and being a fourth option, and the overflow oracle
    /// is what would settle it. Not spiked further — the choice is the operator's.
    #[test]
    #[ignore = "spike"]
    fn can_typst_size_its_own_columns() {
        let template = Template::default();
        let src = r#"#context {
  let cs = ([short], [a considerably longer cell that wants far more of the width than the others do], [mid sized])
  let ws = cs.map(c => measure(c).width)
  let total = ws.fold(0pt, (a, b) => a + b)
  table(columns: ws.map(w => (w / total) * 1fr), ..cs)
}"#;
        let el = Element::new(0, ElementClass::Table, Markup::raw(src.to_string()));
        let ts = Typesetter::new();
        match ts.probe(std::slice::from_ref(&el), &template) {
            Ok((d, _)) => {
                println!("PROBE OK: {:?}", d.get(&el.id).map(|x| x.natural_pt));
                match ts.render(std::slice::from_ref(&el), &template, &d) {
                    Ok(c) => println!("RENDER OK, {} page(s)", c.page_count()),
                    Err(e) => println!("RENDER FAILED: {e:?}"),
                }
            }
            Err(e) => println!("PROBE FAILED: {e:?}"),
        }
    }

    /// Who breaks ordinary words — us, or Typst? And how often?
    #[test]
    #[ignore = "needs documents/; a planning measurement"]
    fn how_often_do_we_break_an_ordinary_word() {
        const BREAK: char = '\u{200b}';
        let broker = md2pdf_paths::PathBroker::new();
        let (mut ordinary, mut pathological, mut shown) = (0usize, 0usize, 0usize);

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
                let Some(alt) = &el.reflow else { continue };
                for tok in alt.as_str().split_whitespace() {
                    if !tok.contains(BREAK) {
                        continue;
                    }
                    let bare: String = tok.chars().filter(|c| *c != BREAK).collect();
                    // "Ordinary" = a run of letters only: no separator a reader would
                    // break at, and nothing that reads as an identifier or a path.
                    let alpha = bare.trim_matches(|c: char| !c.is_alphanumeric());
                    let is_word = alpha.chars().all(|c| c.is_alphabetic()) && alpha.len() >= 3;
                    if is_word {
                        ordinary += 1;
                        if shown < 12 {
                            println!("  ORDINARY WORD BROKEN: {}", tok.replace(BREAK, "|"));
                            shown += 1;
                        }
                    } else {
                        pathological += 1;
                    }
                }
            }
        }
        println!(
            "\ntokens given break opportunities: {} ordinary words, {} identifiers/paths/hashes",
            ordinary, pathological
        );
    }
}
