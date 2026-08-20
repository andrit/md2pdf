//! Golden hashes — the whole PDF, pinned.
//!
//! These exist because md2pdf's output is **deterministic**: the same markdown produces
//! byte-identical PDFs, verified across three separate processes. That property is
//! guarded by `check-boundaries.sh` (INV-7), and this is what it buys.
//!
//! Every other test in this project asserts something *specific* — this text survived,
//! that rung was chosen, this face was italic. A golden hash asserts the opposite: that
//! **nothing at all changed**. It is the only test here that can catch a change nobody
//! thought to look for, which is precisely how the italic bug survived five green
//! commits.
//!
//! ## When one of these fails
//!
//! A red golden test means the rendered output changed. That is not automatically bad —
//! it is *unexplained*. Two honest responses:
//!
//! 1. **The change was intended.** Update the hash in the same commit as the change,
//!    and say in the commit message what moved and why.
//! 2. **The change was not intended.** You have found a real defect, in something you
//!    were not touching. Do not update the hash.
//!
//! Refreshing a hash reflexively because it went red is worse than deleting the test:
//! it looks like coverage and provides none.
//!
//! A Typst upgrade will turn all of these red at once. That is correct — an upgrade is a
//! deliberate, scheduled event with its own runbook (`docs/typst-upgrade.md`), and these
//! going red is the signal that it landed.

use md2pdf_convert::{convert, SourceContext};
use md2pdf_domain::{fnv1a, Template};
use md2pdf_typeset::Typesetter;

/// Convert, compile, and digest the resulting PDF.
///
/// `fnv1a` rather than a cryptographic hash: this detects change, it does not resist an
/// adversary — the same reasoning as `ElementId`'s content hash, and it means these
/// tests add no dependency.
fn golden(markdown: &str) -> String {
    let conversion = convert(markdown, &SourceContext::none());
    let template = Template::default();
    let typesetter = Typesetter::new();

    let (decisions, _) = typesetter
        .probe(&conversion.elements, &template)
        .expect("probe");
    let pdf = typesetter
        .render(&conversion.elements, &template, &decisions)
        .expect("render")
        .pdf()
        .expect("pdf");

    format!("{:016x}/{}", fnv1a(&pdf), pdf.len())
}

/// Deliberately small. Each fixture is a *class* of document, not a variation — the
/// point is broad coverage of the rendering path, and every extra fixture is another
/// line to update on a legitimate Typst upgrade.
#[test]
fn prose_is_unchanged() {
    assert_eq!(
        golden("# Title\n\nOrdinary prose with *emphasis*, **bold**, and `inline code`.\n"),
        "195bdb4cc2c49230/18501"
    );
}

#[test]
fn structure_is_unchanged() {
    assert_eq!(
        golden(
            "# Heading\n\n\
             - a list\n- with items\n\n\
             1. and\n2. numbers\n\n\
             > a quote\n\n\
             | col | val |\n|---|---|\n| a | 1 |\n"
        ),
        "b5079fca3099cde9/13378"
    );
}

#[test]
fn code_and_escaping_are_unchanged() {
    assert_eq!(
        golden(
            "```rust\nfn main() { println!(\"hi\"); }\n```\n\n\
             Hostile text: #hash $dollar [bracket] @at -- ... done.\n"
        ),
        "079a036da2b7c47b/11522"
    );
}

#[test]
fn degraded_images_are_unchanged() {
    // The placeholder path — no filesystem, so every image degrades.
    assert_eq!(
        golden("![a diagram](chart.png)\n\n![remote](https://x.test/a.png)\n"),
        "9db69bf5ba61b742/13170"
    );
}

/// The property the fixtures rest on. If this fails, every hash above is meaningless
/// and the guard in `check-boundaries.sh` has been defeated somehow.
#[test]
fn the_same_document_twice_is_byte_identical() {
    let md = "# Same\n\nEvery time.\n";
    assert_eq!(golden(md), golden(md));
}

/// A rotated, scaled and clipped document also renders deterministically — the
/// escalation ladder must not introduce nondeterminism of its own.
#[test]
fn the_escalation_ladder_is_unchanged() {
    let md = "| a | b | c | d | e | f | g | h |\n|---|---|---|---|---|---|---|---|\n\
              | verylongvalue1 | verylongvalue2 | verylongvalue3 | verylongvalue4 \
              | verylongvalue5 | verylongvalue6 | verylongvalue7 | verylongvalue8 |\n";
    let first = golden(md);
    assert_eq!(first, golden(md), "the ladder is not deterministic");
    assert_eq!(first, "910368d04bc4d1b0/13525");
}
