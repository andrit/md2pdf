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
use md2pdf_domain::{fnv1a, ElementClass, Reduction, Template};
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

/// The same, but assert which rung the ladder chose before hashing.
///
/// A golden hash cannot tell you *what* it is covering. `the_escalation_ladder_is_unchanged`
/// was assumed to cover reflow and does not — its table shrinks to 7.5pt and the alternate
/// is never rendered, which is why changing the alternate's column spec left every golden
/// green. Asserting the rung means the fixture cannot silently stop covering it.
fn golden_at(markdown: &str, expected: Reduction) -> String {
    let conversion = convert(markdown, &SourceContext::none());
    let template = Template::default();
    let typesetter = Typesetter::new();

    let (decisions, _) = typesetter
        .probe(&conversion.elements, &template)
        .expect("probe");
    let chosen = conversion
        .elements
        .iter()
        .find(|e| e.class == ElementClass::Table)
        .and_then(|e| decisions.get(&e.id))
        .map(|d| d.reduction)
        .expect("no table in the fixture");
    assert_eq!(
        chosen, expected,
        "this fixture no longer exercises the rung it was written for"
    );

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
        "6ab935bbb504a328/18503"
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
        "4cdaa0d98803c043/13387"
    );
}

#[test]
fn code_and_escaping_are_unchanged() {
    assert_eq!(
        golden(
            "```rust\nfn main() { println!(\"hi\"); }\n```\n\n\
             Hostile text: #hash $dollar [bracket] @at -- ... done.\n"
        ),
        "e1db4680e603a480/11532"
    );
}

#[test]
fn degraded_images_are_unchanged() {
    // The placeholder path — no filesystem, so every image degrades.
    assert_eq!(
        golden("![a diagram](chart.png)\n\n![remote](https://x.test/a.png)\n"),
        "7f0f589676488e39/13174"
    );
}

/// The property the fixtures rest on. If this fails, every hash above is meaningless
/// and the guard in `check-boundaries.sh` has been defeated somehow.
#[test]
fn the_same_document_twice_is_byte_identical() {
    let md = "# Same\n\nEvery time.\n";
    assert_eq!(golden(md), golden(md));
}

/// The ladder renders deterministically — it must not introduce nondeterminism of its
/// own, whichever rung it picks.
///
/// This table shrank to 7.5pt until T26b, and now reflows: below the comfort floor a
/// table wraps rather than being squeezed. The determinism check is the point here, so
/// the fixture is kept as-is rather than re-tuned to hold its old rung.
#[test]
fn the_escalation_ladder_is_unchanged() {
    let md = "| a | b | c | d | e | f | g | h |\n|---|---|---|---|---|---|---|---|\n\
              | verylongvalue1 | verylongvalue2 | verylongvalue3 | verylongvalue4 \
              | verylongvalue5 | verylongvalue6 | verylongvalue7 | verylongvalue8 |\n";
    let first = golden(md);
    assert_eq!(first, golden(md), "the ladder is not deterministic");
    assert_eq!(first, "c4aa35e9a496befd/13643");
}

/// The reflow rung, which no golden covered until 2026-08-22.
///
/// This is the fixture that moves when the alternate's column spec changes — the whole
/// point of T26a2 — so it is the one that would have caught that change silently landing.
#[test]
fn a_reflowed_table_is_unchanged() {
    // Six columns of 60 characters: too wide for landscape even at the 7pt floor, so the
    // ladder falls to its last rung before clipping.
    let cell = "x".repeat(60);
    let row = format!("| {} |\n", [cell.as_str(); 6].join(" | "));
    let md = format!(
        "| Column 0 | Column 1 | Column 2 | Column 3 | Column 4 | Column 5 |\n\
         |---|---|---|---|---|---|\n{row}{row}"
    );
    assert_eq!(golden_at(&md, Reduction::Reflow), "e62d77c673120aae/13252");
}

/// A **lopsided** reflow — one prose column, the rest narrow.
///
/// The other two ladder goldens hold uniform tables, where every column spec that gives
/// equal shares renders identically: `(1fr × 6)` and `(6fr × 6)` are the same layout. So
/// neither of them moved when the alternate changed from equal `1fr`, to `auto` plus
/// `1fr`, to weighted `fr` — three changes to the thing they were assumed to cover.
///
/// This one is proportional by construction, so it moves when the weighting does.
#[test]
fn a_proportional_reflow_is_unchanged() {
    let prose = "a considerably longer sentence that has to wrap somewhere or other";
    let row = format!("| E01 | {prose} | P1 | yes | {prose} |\n");
    let md = format!(
        "| id | detail | pri | ok | notes |\n\
         |---|---|---|---|---|\n{row}{row}{row}"
    );
    assert_eq!(golden_at(&md, Reduction::Reflow), "3dae198a3ce94f95/14925");
}
