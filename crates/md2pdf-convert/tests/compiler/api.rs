//! `convert()` — the crate's whole public surface (T8).
//!
//! The properties here are the ones callers are entitled to rely on. Construct-level
//! behaviour is unit-tested next to the code; these are the contract.

use md2pdf_convert::{convert, Conversion, SourceContext};
use md2pdf_domain::{ElementClass, Template};
use md2pdf_typeset::Typesetter;

/// Conversion is **total**: no input produces an error, because there is no error to
/// produce. If this ever needs a `Result`, the design changed.
#[test]
fn conversion_is_total_over_hostile_input() {
    let hostile = [
        ("empty", ""),
        ("whitespace", "   \n\n\t  \n"),
        ("nul and control bytes", "a\u{0}b\u{7}c\u{1b}[0m"),
        ("lone combining marks", "\u{0301}\u{0301}\u{0301}"),
        ("rtl and bidi overrides", "\u{202e}reversed\u{202c} text"),
        ("zero-width joiners", "a\u{200d}\u{200b}\u{feff}b"),
        ("unterminated fence", "```rust\nfn main() {"),
        ("unterminated table", "| a | b |\n|---|"),
        ("unclosed emphasis", "*never closed"),
        ("deeply nested lists", &"- a\n".repeat(200)),
        ("deep blockquote", &">".repeat(200)),
        ("very long line", &"x".repeat(50_000)),
        ("only front matter", "---\ntitle: t\n---\n"),
        ("html soup", "<div><span><p>unclosed"),
        ("null footnote ref", "text[^]"),
        ("astral plane", "𝕳𝖊𝖑𝖑𝖔 🎉🇬🇧 👨‍👩‍👧‍👦"),
    ];

    for (name, input) in hostile {
        // The assertion is that this returns at all — no panic, no Result.
        let c: Conversion = convert(input, &SourceContext::none());
        // And that whatever came back is internally consistent.
        assert_compromises_are_addressable(&c, name);
    }
}

/// A Compromise that names no Element cannot be acted on at the attention gate, which
/// makes it worse than useless — it reports a problem the user cannot reach.
fn assert_compromises_are_addressable(c: &Conversion, ctx: &str) {
    for compromise in &c.compromises {
        assert!(
            c.elements.iter().any(|e| e.id.matches(&compromise.id)),
            "{ctx}: compromise {:?} points at no element",
            compromise.kind
        );
    }
}

/// `harvest.rs` resolves probe metadata back to elements by `order` alone, so a
/// duplicate silently binds a decision to the wrong element.
#[test]
fn orders_are_unique_across_a_whole_document() {
    let md = "# A\n\np1\n\n- l\n\n> q\n\n```\nc\n```\n\n| h |\n|---|\n| v |\n\np2\n\n## B\n\np3\n";
    let orders: Vec<u32> = convert(md, &SourceContext::none())
        .elements
        .iter()
        .map(|e| e.id.order)
        .collect();

    let mut unique = orders.clone();
    unique.sort_unstable();
    unique.dedup();
    assert_eq!(orders.len(), unique.len(), "duplicate orders: {orders:?}");
    assert!(
        orders.windows(2).all(|w| w[0] < w[1]),
        "orders not ascending: {orders:?}"
    );
}

#[test]
fn elements_keep_source_order_and_class() {
    let c = convert(
        "# H\n\nprose\n\n| a |\n|---|\n| 1 |\n\n```\ncode\n```\n",
        &SourceContext::none(),
    );
    let classes: Vec<ElementClass> = c.elements.iter().map(|e| e.class).collect();
    assert_eq!(
        classes,
        vec![
            ElementClass::Heading,
            ElementClass::Prose,
            ElementClass::Table,
            ElementClass::Code,
        ]
    );
}

#[test]
fn a_clean_document_is_not_flagged() {
    let c = convert(
        "# Title\n\nOrdinary *prose* with `code`.\n",
        &SourceContext::none(),
    );
    assert!(
        !c.is_flagged(),
        "unexpected compromises: {:?}",
        c.compromises
    );
}

#[test]
fn a_document_with_an_image_is_flagged() {
    // Stage 1 cannot embed images, and says so rather than failing silently.
    let c = convert("![alt](d.png)\n", &SourceContext::none());
    assert!(c.is_flagged());
}

/// The end-to-end promise: markdown in, PDF out, through the real compiler.
#[test]
fn a_converted_document_compiles_to_a_pdf() {
    let c = convert(
        "# Report\n\nProse with *emphasis* and a [link](https://x.test).\n\n\
         | Col | Val |\n|---|---|\n| a | 1 |\n\n```rust\nfn main() {}\n```\n",
        &SourceContext::none(),
    );
    let template = Template::default();
    let ts = Typesetter::new();

    let (map, diagnostic) = ts.probe(&c.elements, &template).expect("probe");
    let compilation = ts.render(&c.elements, &template, &map).expect("render");
    let pdf = compilation.pdf().expect("pdf");

    assert!(pdf.starts_with(b"%PDF"));
    assert!(compilation.page_count() >= 1);
    // Nothing in this document should have needed the escalation ladder.
    assert!(
        !diagnostic.is_flagged(),
        "unexpected ladder compromises: {diagnostic:?}"
    );
}

/// Emphasis must survive all the way from markdown to an italic glyph run.
///
/// The unit tests check that `#emph[..]` is emitted; only the compiler can confirm a
/// face was actually resolved. Italic silently not rendering is exactly the bug this
/// crate's text-based tests could not see.
#[test]
fn emphasis_reaches_the_page_as_italic() {
    let c = convert("Plain *slanted* plain.\n", &SourceContext::none());
    let template = Template::default();
    let ts = Typesetter::new();
    let (map, _) = ts.probe(&c.elements, &template).expect("probe");
    let runs = ts
        .render(&c.elements, &template, &map)
        .expect("render")
        .text_runs();

    assert!(
        runs.iter()
            .any(|(text, _, style)| *style == "italic" && text.contains("slanted")),
        "emphasis did not reach the page in italic: {runs:?}"
    );
}
