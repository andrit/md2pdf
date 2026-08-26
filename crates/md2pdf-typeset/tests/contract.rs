//! Behavioural contract tests for the Typst boundary.
//!
//! These do NOT test our arithmetic. They pin **Typst's observable behaviour**, so
//! that a version bump which still compiles but changes semantics fails loudly.
//!
//! Going to 0.15.1 produced eight breaking API changes — every one a compile error,
//! which is the safe kind. The dangerous kind is silent behavioural drift: if a future
//! `measure()` stopped clamping, or `raw` blocks stopped wrapping, nothing would fail
//! to compile and the escalation ladder would quietly start making wrong decisions.
//! That is what these catch.
//!
//! **Tolerance is 0.5pt** — exactly the shrink step. Any drift large enough to change
//! a rung fails; anything smaller cannot. That makes epsilon principled rather than
//! guessed.

use md2pdf_domain::{
    Decision, DecisionMap, Element, ElementClass, ElementId, Floors, Markup, Orientation, Override,
    Permit, Reduction, Template,
};
use md2pdf_typeset::Typesetter;

/// The shrink step. Drift below this cannot change a decision.
const EPSILON_PT: f64 = 0.5;

/// Deliberately narrow so overflow is easy to provoke, and fixed so the pinned
/// numbers below mean something.
fn template() -> Template {
    Template {
        name: "contract".into(),
        page_width_pt: 220.0,
        page_height_pt: 320.0,
        margin_pt: 12.0,
        base_size_pt: 10.0,
        floors: Floors {
            table_pt: 7.0,
            // Kept at 9.0 rather than following the default to 10.0: these tests pin the
            // *mechanism* against a small synthetic page, and several choose their sizes
            // to sit either side of this boundary. T26c moved the shipped default by eye
            // against real pages, which is a different question from this one.
            table_comfort_pt: 9.0,
            image_scale: 0.25,
        },
        font_body: "Source Sans 3".into(),
        font_mono: "JetBrains Mono".into(),
    }
}

fn wide_table() -> Element {
    Element::new(
        1,
        ElementClass::Table,
        Markup::raw(r#"#table(columns: 6, ..range(6).map(i => [longvalue#i]))"#),
    )
}
fn narrow_table() -> Element {
    Element::new(
        2,
        ElementClass::Table,
        Markup::raw(r#"#table(columns: 3, [a], [b], [c])"#),
    )
}
fn prose() -> Element {
    Element::new(0, ElementClass::Prose, Markup::raw("#lorem(18)"))
}
fn code() -> Element {
    Element::new(
        3,
        ElementClass::Code,
        Markup::raw(
            r#"#raw("fn escalate(el: &Element, avail: Abs) -> Decision { todo!() }", lang: "rust", block: true)"#,
        ),
    )
}
fn figure() -> Element {
    Element::new(
        4,
        ElementClass::Image,
        Markup::raw("#rect(width: 400pt, height: 30pt)"),
    )
}

fn probe(elements: &[Element]) -> DecisionMap {
    Typesetter::new()
        .probe(elements, &template())
        .expect("probe compiles")
        .0
}

// ---------------------------------------------------------------------------
// The available width the Template computes must match what Typst would report.
// ---------------------------------------------------------------------------

#[test]
fn available_width_is_page_minus_margins() {
    // 220 - 2*12. Verified against `layout()` in the spike, which reported 196.0.
    // The ProbePass relies on this being computable without asking Typst.
    assert!((template().available_pt() - 196.0).abs() < f64::EPSILON);
}

// ---------------------------------------------------------------------------
// The escalation ladder's decisions. These are the contract that matters.
// ---------------------------------------------------------------------------

#[test]
fn wide_table_escalates_to_rotate() {
    let els = vec![wide_table()];
    let map = probe(&els);
    let d = map.get(&els[0].id).expect("decision for the table");
    assert_eq!(
        d.orientation,
        Orientation::Landscape,
        "a 6-column table at 196pt must reach the floor and still not fit; got {:?} \
         (natural {:.2}pt)",
        d.orientation,
        d.natural_pt
    );
}

#[test]
fn narrow_table_is_left_alone() {
    let els = vec![narrow_table()];
    let map = probe(&els);
    assert_eq!(map.get(&els[0].id).unwrap().reduction, Reduction::None);
}

#[test]
fn prose_never_overflows_because_it_wraps() {
    // The naive predicate `natural > available` is TRUE here — prose laid out on one
    // infinite line is far wider than the page. If this test ever reports a rung
    // other than None, the Wrappable/Atomic split has been broken.
    let els = vec![prose()];
    let map = probe(&els);
    let d = map.get(&els[0].id).unwrap();
    assert_eq!(d.reduction, Reduction::None);
    assert!(
        d.natural_pt > d.available_pt,
        "precondition: prose natural width should exceed available, else this test \
         proves nothing (natural {:.2}, available {:.2})",
        d.natural_pt,
        d.available_pt
    );
}

#[test]
fn an_oversized_figure_scales_before_it_rotates() {
    // 400pt into 196pt needs ~49%, comfortably above the 0.25 scale floor.
    //
    // This replaces `oversized_figure_escalates_to_rotate`, which asserted the rung
    // reached *before* T13 — when the probe stepped font size to shrink an image and
    // therefore could never move one, sending every oversized image straight to
    // Rotate. Scaling is lossless for an image; a whole landscape page is not.
    let els = vec![figure()];
    let map = probe(&els);
    match map.get(&els[0].id).unwrap().reduction {
        Reduction::Scale { factor } => {
            assert!(
                (0.45..0.55).contains(&factor),
                "expected ~0.49, got {factor}"
            );
        }
        other => panic!("expected Scale, got {other:?}"),
    }
}

#[test]
fn a_figure_below_the_scale_floor_rotates_instead() {
    // 1000pt into 196pt needs ~20%, under the floor — unreadable, so rotate for the
    // extra width rather than shrink into uselessness.
    let els = vec![Element::new(
        5,
        ElementClass::Image,
        Markup::raw("#rect(width: 1000pt, height: 30pt)"),
    )];
    let map = probe(&els);
    let d = map.get(&els[0].id).unwrap();
    assert_eq!(d.orientation, Orientation::Landscape);
}

#[test]
fn a_scaled_element_really_occupies_less_space() {
    // `reflow: true` is the difference between fixing an overflow and only appearing
    // to: Typst's scale is a visual transform by default and leaves the original
    // footprint behind.
    let ts = Typesetter::new();
    let el = figure();
    let scaled = ts
        .render(
            std::slice::from_ref(&el),
            &template(),
            &DecisionMap {
                decisions: vec![Decision {
                    id: el.id,
                    orientation: Orientation::Portrait,
                    reduction: Reduction::Scale { factor: 0.25 },
                    natural_pt: 400.0,
                    available_pt: 196.0,
                }],
            },
        )
        .expect("renders");
    let plain = ts
        .render(
            std::slice::from_ref(&el),
            &template(),
            &DecisionMap { decisions: vec![] },
        )
        .expect("renders");
    // The unscaled 400pt figure overflows a 220pt page and spills onto more of it;
    // the scaled one fits. Page count is the coarse, honest signal available here.
    assert!(
        scaled.page_count() <= plain.page_count(),
        "scaling did not reduce the space used"
    );
}

// ---------------------------------------------------------------------------
// Typst behaviours we depend on. If these change, our model is wrong.
// ---------------------------------------------------------------------------

#[test]
fn raw_blocks_wrap_so_code_is_wrappable() {
    // Verified in 0.15.1: a long `raw(block: true)` line breaks rather than
    // overflowing. `ElementClass::Code::is_atomic()` returns false BECAUSE of this.
    // If Typst ever stops wrapping raw blocks, Code must become Atomic.
    assert!(!ElementClass::Code.is_atomic());
    let els = vec![code()];
    let map = probe(&els);
    assert_eq!(map.get(&els[0].id).unwrap().reduction, Reduction::None);
}

#[test]
fn measured_natural_width_is_stable() {
    // Pinned measurements, tolerance = one shrink step. These are the numbers the
    // ladder branches on; drift beyond half a point can change a rung.
    let els = vec![wide_table(), narrow_table(), figure()];
    let map = probe(&els);

    let expect = [
        (&els[0], 335.22, "wide 6-column table"),
        (&els[1], 45.13, "narrow 3-column table"),
        (&els[2], 400.00, "400pt figure"),
    ];

    // Collect every mismatch rather than panicking on the first: after a Typst
    // upgrade you want the whole picture in one run, not one number at a time.
    let mut drift = Vec::new();
    for (el, want, what) in expect {
        let got = map.get(&el.id).unwrap().natural_pt;
        if (got - want).abs() > EPSILON_PT {
            drift.push(format!("{what}: expected {want:.2}pt, got {got:.2}pt"));
        }
    }
    assert!(
        drift.is_empty(),
        "measured natural widths drifted beyond {EPSILON_PT}pt:\n  {}\n\
         If this is a deliberate Typst upgrade, re-verify the ladder against real \
         output and update the pins in the same commit. See docs/typst-upgrade.md.",
        drift.join("\n  ")
    );
}

// ---------------------------------------------------------------------------
// The RenderPass acts where the ProbePass only decided.
// ---------------------------------------------------------------------------

#[test]
fn rotate_produces_a_landscape_page() {
    let els = vec![prose(), wide_table(), prose()];
    let ts = Typesetter::new();
    let tpl = template();
    let (map, diagnostic) = ts.probe(&els, &tpl).expect("probe");
    let out = ts.render(&els, &tpl, &map).expect("render");

    let geo = out.geometry();
    assert!(
        geo.iter().any(|p| p.is_landscape()),
        "the rotated table must land on a landscape page; got {geo:?}"
    );
    assert!(
        diagnostic.is_flagged(),
        "a rotation is a Compromise and must be recorded"
    );
}

#[test]
fn preview_and_export_come_from_one_compilation() {
    let els = vec![prose(), wide_table()];
    let ts = Typesetter::new();
    let tpl = template();
    let (map, _) = ts.probe(&els, &tpl).unwrap();
    let out = ts.render(&els, &tpl, &map).unwrap();

    let pdf = out.pdf().expect("pdf export");
    assert!(pdf.starts_with(b"%PDF"), "expected a PDF header");

    let (w, h, px) = out.raster(0, 2.0).expect("raster page 1");
    assert_eq!(px.len(), (w * h * 4) as usize, "RGBA8");
    assert!(w > 0 && h > 0);
}

// ---------------------------------------------------------------------------
// The FontBook. Typst ships no sans-serif; ours must be present.
// ---------------------------------------------------------------------------

#[test]
fn shipped_fontbook_contains_our_faces() {
    let families = Typesetter::new().font_families();
    for want in ["Source Sans 3", "JetBrains Mono"] {
        assert!(
            families.iter().any(|f| f == want),
            "{want} missing from the shipped FontBook; have {families:?}"
        );
    }
}

// ---------------------------------------------------------------------------
// Override staleness — the hole content-hashing closes.
// ---------------------------------------------------------------------------

#[test]
fn a_stale_decision_is_rejected_not_misapplied() {
    // The hole content-hashing closes: the Source was edited, and this id no longer names
    // the same Element. Splicing a decision in anyway would rotate or shrink whatever now
    // occupies that position.
    //
    // **Rewritten in 3f, not renumbered.** This used to exercise
    // `DecisionMap::apply_override`, which set the two axes directly — and its other half
    // asserted that "force portrait" left a shrink alone, a property that only made sense
    // while an Override *was* a decision. It is now a permission the ProbePass measures
    // under, so there is no way to name an orientation without a size measured for it,
    // and that half of the test was asserting the bug was still reachable. See
    // `design/plan-review.md`.
    let els = vec![wide_table()];
    let mut map = probe(&els);
    let live = map.get(&els[0].id).cloned().expect("decided");

    let edited = Decision {
        id: ElementId::new(els[0].id.order, "#table(columns: 2, [a], [b])"),
        orientation: Orientation::Portrait,
        ..live.clone()
    };
    assert!(
        !map.replace(edited),
        "a Decision whose content hash no longer matches must be dropped, not applied \
         to whatever now occupies that position"
    );
    assert_eq!(
        map.get(&els[0].id).unwrap().orientation,
        Orientation::Landscape,
        "unchanged"
    );

    // The live id still splices.
    assert!(map.replace(Decision {
        orientation: Orientation::Portrait,
        ..live
    }));
    assert_eq!(
        map.get(&els[0].id).unwrap().orientation,
        Orientation::Portrait
    );
}

#[test]
fn emphasis_renders_in_an_italic_face() {
    let el = Element::new(
        0,
        ElementClass::Prose,
        Markup::raw("plain #emph[slanted] plain"),
    );
    let runs = Typesetter::new()
        .render(&[el], &template(), &DecisionMap { decisions: vec![] })
        .expect("compiles")
        .text_runs();

    let italic: Vec<_> = runs.iter().filter(|(_, _, s)| *s == "italic").collect();
    assert!(
        !italic.is_empty(),
        "nothing rendered in an italic face; runs were {runs:?}"
    );
    assert!(
        italic.iter().any(|(t, _, _)| t.contains("slanted")),
        "the emphasised word is not the italic one: {italic:?}"
    );
    // And it must be OUR family, not a fallback serif.
    assert!(
        italic.iter().any(|(_, f, _)| f == "Source Sans 3"),
        "italic came from a fallback family: {italic:?}"
    );
}

// ---- The World file map (T10) ----------------------------------------------
//
// Typst cannot load a file the World will not serve, and a missing file is a
// compilation error for the WHOLE document rather than a skipped element. These pin
// the behaviour the image pipeline is built on.

const SQUARE_10X10: &[u8] = include_bytes!("fixtures/square-10x10.png");
const WIDE_200X20: &[u8] = include_bytes!("fixtures/wide-200x20.png");

fn image_element(name: &str) -> Element {
    Element::new(
        0,
        ElementClass::Image,
        Markup::raw(format!(r#"#image("{name}")"#)),
    )
}

fn natural_width_of(ts: &Typesetter, el: &Element) -> f64 {
    ts.probe(std::slice::from_ref(el), &template())
        .expect("probe compiles")
        .0
        .get(&el.id)
        .expect("decision")
        .natural_pt
}

#[test]
fn a_registered_image_renders() {
    let ts = Typesetter::new();
    assert!(ts.add_file("img.png", SQUARE_10X10.to_vec()));

    let el = image_element("img.png");
    let compilation = ts
        .render(
            std::slice::from_ref(&el),
            &template(),
            &DecisionMap { decisions: vec![] },
        )
        .expect("a registered image must compile");

    assert_eq!(compilation.page_count(), 1);
    let pdf = compilation.pdf().expect("pdf");
    assert!(pdf.starts_with(b"%PDF"));
}

#[test]
fn an_unregistered_image_fails_the_whole_compilation() {
    // Not a defect — the reason Stage 1 emits placeholders instead of `#image`.
    // Pinned so that if Typst ever softens this, we find out and can revisit.
    let ts = Typesetter::new();
    let el = image_element("never-registered.png");
    let err = match ts.render(
        std::slice::from_ref(&el),
        &template(),
        &DecisionMap { decisions: vec![] },
    ) {
        Ok(_) => panic!("an unresolvable file must fail the compilation"),
        Err(e) => e,
    };
    assert!(
        err.to_string().contains("file not found"),
        "unexpected error: {err}"
    );
}

#[test]
fn replacing_bytes_is_not_served_stale() {
    // The risk worth pinning: `Typesetter` holds a long-lived `World` so `comemo`
    // memoisation survives between compilations. If a replaced image were served from
    // that cache the document would silently show the OLD picture — no error, no
    // diagnostic. It is not, and this keeps it that way.
    let ts = Typesetter::new();
    let el = image_element("img.png");

    ts.add_file("img.png", SQUARE_10X10.to_vec());
    let square = natural_width_of(&ts, &el);

    ts.add_file("img.png", WIDE_200X20.to_vec());
    let wide = natural_width_of(&ts, &el);

    assert!(
        wide > square * 5.0,
        "replaced image was served stale: {square}pt then {wide}pt"
    );
}

#[test]
fn clearing_files_unregisters_them() {
    // Between Jobs, so one document's images can never leak into another's.
    let ts = Typesetter::new();
    ts.add_file("img.png", SQUARE_10X10.to_vec());
    let el = image_element("img.png");
    assert!(ts
        .render(
            std::slice::from_ref(&el),
            &template(),
            &DecisionMap { decisions: vec![] }
        )
        .is_ok());

    ts.clear_files();
    assert!(
        ts.render(
            std::slice::from_ref(&el),
            &template(),
            &DecisionMap { decisions: vec![] }
        )
        .is_err(),
        "cleared file was still served"
    );
}

#[test]
fn an_unusable_virtual_name_is_rejected_rather_than_panicking() {
    // Names ultimately derive from filesystem paths this crate never sees, so a
    // library must not panic on one. Typst rejects exactly two shapes
    // (`typst-syntax/src/path.rs`, `PathError`):
    //
    //   Escapes   — the path would climb above the root (`..`)
    //   Backslash — cross-platform hazard, since Windows uses it as a separator
    //
    // The first is free defence-in-depth for image path traversal: even if a
    // malformed name reached here, `../../etc/passwd` cannot be registered.
    let ts = Typesetter::new();

    assert!(
        !ts.add_file("../escape.png", vec![1, 2, 3]),
        "a path escaping the root was accepted"
    );
    assert!(
        !ts.add_file("back\\slash.png", vec![1, 2, 3]),
        "a backslash name was accepted"
    );
    assert!(
        ts.add_file("img-9f3a1c77b2e40d51.png", vec![1, 2, 3]),
        "the virtual naming scheme from plan-images.md was rejected"
    );
    assert!(
        ts.add_file("nested/dir/img.png", vec![1, 2, 3]),
        "a nested virtual path was rejected"
    );
}

const HUGE: &[u8] = include_bytes!("fixtures/huge-2000x300.png");

#[test]
fn a_real_image_enters_the_ladder_by_scale_not_by_font_size() {
    // The defect T13 fixed, pinned against a real raster rather than a `#rect`.
    // Before: the probe stepped font size, which cannot move an image, so a 2000pt
    // image measured identically ~20 times and fell straight through to Rotate.
    let ts = Typesetter::new();
    ts.add_file("huge.png", HUGE.to_vec());

    // 2000pt into 196pt needs ~10%: under the 0.25 floor, so rotation is right.
    let huge = Element::new(
        0,
        ElementClass::Image,
        Markup::raw(r#"#box(image("huge.png"))"#),
    );
    let (map, _) = ts
        .probe(std::slice::from_ref(&huge), &template())
        .expect("probe");
    assert_eq!(
        map.get(&huge.id).unwrap().orientation,
        Orientation::Landscape
    );

    // The same image on a page wide enough that ~30% suffices must SCALE instead.
    let wide_page = Template {
        page_width_pt: 640.0,
        ..template()
    };
    let (map, _) = ts
        .probe(std::slice::from_ref(&huge), &wide_page)
        .expect("probe");
    match map.get(&huge.id).unwrap().reduction {
        Reduction::Scale { factor } => assert!((0.25..0.40).contains(&factor), "got {factor}"),
        other => panic!("expected Scale on the wider page, got {other:?}"),
    }
}

#[test]
fn a_small_image_is_left_alone() {
    let ts = Typesetter::new();
    ts.add_file("sq.png", SQUARE_10X10.to_vec());
    let el = Element::new(
        0,
        ElementClass::Image,
        Markup::raw(r#"#box(image("sq.png"))"#),
    );
    let (map, diag) = ts
        .probe(std::slice::from_ref(&el), &template())
        .expect("probe");
    assert_eq!(map.get(&el.id).unwrap().reduction, Reduction::None);
    assert!(!diag.is_flagged(), "a fitting image is not a compromise");
}

// ---- The ladder's tail (T14) -----------------------------------------------

/// A table too wide for portrait must be **re-measured in landscape**, not rotated at
/// its portrait floor.
///
/// The GLOSSARY is explicit — "RE-MEASURE in landscape; do not inherit the portrait
/// size… Carrying the Floor size over is a bug" — and until T14 the RenderPass simply
/// rendered a rotated element at natural size, because it never measures.
#[test]
fn a_rotated_element_is_remeasured_in_landscape() {
    let els = vec![wide_table()];
    let map = probe(&els);
    let d = map.get(&els[0].id).expect("decision");

    assert_eq!(d.orientation, Orientation::Landscape);
    // Decided against the landscape width, not the portrait one.
    assert_eq!(
        d.available_pt,
        template().available_landscape_pt(),
        "reduction was measured against the wrong width"
    );
    // And it fits there at a size above the floor rather than being driven to it.
    match d.reduction {
        Reduction::Shrink { size_pt } => assert!(
            size_pt > template().floors.table_pt,
            "inherited the portrait floor instead of re-measuring: {size_pt}pt"
        ),
        Reduction::None => {}
        other => panic!("expected to fit in landscape, got {other:?}"),
    }
}

/// The fourth rung, reachable for the first time.
///
/// `probe.rs` could previously emit only none/shrink/rotate, so `Reduction::Clip` was
/// unreachable code even though `harvest` parsed it and `render` implemented it. An
/// element too wide even for landscape at its floor overflowed silently.
#[test]
fn an_element_too_wide_even_for_landscape_is_clipped() {
    let els = vec![Element::new(
        9,
        ElementClass::Table,
        // 40 columns of long values: no font size within the floor fits 308pt.
        Markup::raw(r#"#table(columns: 40, ..range(40).map(i => [verylongcell#i]))"#),
    )];
    let map = probe(&els);
    let d = map.get(&els[0].id).expect("decision");

    assert_eq!(
        d.orientation,
        Orientation::Landscape,
        "should have tried landscape"
    );
    assert_eq!(
        d.reduction,
        Reduction::Clip,
        "the last rung is still unreachable"
    );
}

/// Clipping must be visible. Silently losing content is the one outcome worse than an
/// ugly page.
#[test]
fn a_clipped_element_carries_a_visible_marker() {
    let el = Element::new(
        0,
        ElementClass::Table,
        Markup::raw("#table(columns: 1, [x])"),
    );
    let map = DecisionMap {
        decisions: vec![Decision {
            id: el.id,
            orientation: Orientation::Portrait,
            reduction: Reduction::Clip,
            natural_pt: 999.0,
            available_pt: 196.0,
        }],
    };
    let text = Typesetter::new()
        .render(std::slice::from_ref(&el), &template(), &map)
        .expect("renders")
        .text();
    assert!(
        text.contains("clipped"),
        "no marker in the output: {text:?}"
    );
}

/// Both axes reach the page: a rotated element lands on a landscape page, and its
/// reduction is applied there.
#[test]
fn rotation_and_reduction_compose_in_the_output() {
    let el = wide_table();
    let map = DecisionMap {
        decisions: vec![Decision {
            id: el.id,
            orientation: Orientation::Landscape,
            reduction: Reduction::Shrink { size_pt: 8.0 },
            natural_pt: 400.0,
            available_pt: 296.0,
        }],
    };
    let compilation = Typesetter::new()
        .render(std::slice::from_ref(&el), &template(), &map)
        .expect("renders");
    let landscape = compilation.geometry().iter().any(|p| p.is_landscape());
    assert!(
        landscape,
        "no landscape page produced: {:?}",
        compilation.geometry()
    );
}

// ---- Reflow: the rung that replaced clipping for tables (T26a) ---------------

#[test]
fn a_table_too_wide_to_shrink_reflows_instead_of_clipping() {
    // Before this rung existed, 25 elements across 9 real documents lost their
    // right-hand columns. Every one was a table.
    let cells: String = (0..6)
        .map(|i| format!("[a reasonably long cell value number {i}]"))
        .collect::<Vec<_>>()
        .join(", ");
    let el = Element::with_reflow(
        0,
        ElementClass::Table,
        Markup::raw(format!("#table(columns: 6, {cells})")),
        Markup::raw(format!(
            "#table(columns: (1fr, 1fr, 1fr, 1fr, 1fr, 1fr), {cells})"
        )),
    );

    let map = probe(std::slice::from_ref(&el));
    let d = map.get(&el.id).expect("decision");
    assert_eq!(
        d.reduction,
        Reduction::Reflow,
        "a table with an alternate form must never clip"
    );
}

#[test]
fn an_element_with_no_alternate_still_clips() {
    // Images cannot reflow, so the last rung remains reachable for them.
    let el = Element::new(
        9,
        ElementClass::Table,
        Markup::raw(r#"#table(columns: 40, ..range(40).map(i => [verylongcell#i]))"#),
    );
    let map = probe(std::slice::from_ref(&el));
    assert_eq!(map.get(&el.id).unwrap().reduction, Reduction::Clip);
}

#[test]
fn reflow_renders_the_alternate_body_not_the_original() {
    let el = Element::with_reflow(
        0,
        ElementClass::Table,
        Markup::raw("#table(columns: 1, [ORIGINAL])"),
        Markup::raw("#table(columns: (1fr), [ALTERNATE])"),
    );
    let map = DecisionMap {
        decisions: vec![Decision {
            id: el.id,
            orientation: Orientation::Portrait,
            reduction: Reduction::Reflow,
            natural_pt: 999.0,
            available_pt: 196.0,
        }],
    };
    let text = Typesetter::new()
        .render(std::slice::from_ref(&el), &template(), &map)
        .expect("renders")
        .text();

    assert!(
        text.contains("ALTERNATE"),
        "the alternate was not used: {text:?}"
    );
    assert!(
        !text.contains("ORIGINAL"),
        "both bodies were rendered: {text:?}"
    );
}

#[test]
fn a_reflowed_table_keeps_all_of_its_content() {
    // The point of the rung: nothing is lost.
    let cells: String = (0..6)
        .map(|i| format!("[CELL{i}]"))
        .collect::<Vec<_>>()
        .join(", ");
    let el = Element::with_reflow(
        0,
        ElementClass::Table,
        Markup::raw(format!("#table(columns: 6, {cells})")),
        Markup::raw(format!(
            "#table(columns: (1fr, 1fr, 1fr, 1fr, 1fr, 1fr), {cells})"
        )),
    );
    let map = probe(std::slice::from_ref(&el));
    let text = Typesetter::new()
        .render(std::slice::from_ref(&el), &template(), &map)
        .expect("renders")
        .text();

    for i in 0..6 {
        assert!(text.contains(&format!("CELL{i}")), "CELL{i} was lost");
    }
}

// ---------------------------------------------------------------------------------
// T28 — glyph coverage.
//
// A character with no glyph renders as **tofu**, an empty box, and nothing in the text
// layer says so: extracted text carries the character either way. That is why ✅ and ❌
// sat unnoticed in roughly a fifth of the corpus until a page was looked at during T26b.
//
// These live in this file rather than their own because each integration binary
// statically links ~250 crates of the typst graph, and linking two at once has run the
// machine out of memory three times. One binary per crate.
// ---------------------------------------------------------------------------------

/// Characters the corpus actually contains, with why each one matters.
///
/// The ones that work are as load-bearing as the ones that do not: box drawing inside
/// code fences appears in 46 documents, and losing it would be worse than the tofu.
const CORPUS_CHARACTERS: &[(char, &str)] = &[
    ('✓', "check mark U+2713"),
    ('✗', "ballot X U+2717"),
    ('⚠', "warning sign U+26A0"),
    ('→', "rightwards arrow U+2192"),
    ('▸', "small right triangle U+25B8"),
    (
        '─',
        "box drawing horizontal U+2500 — 46 documents, in code fences",
    ),
    ('│', "box drawing vertical U+2502"),
    ('└', "box drawing up-and-right U+2514"),
    ('☐', "ballot box U+2610 — emitted by convert for task lists"),
    (
        '☑',
        "ballot box checked U+2611 — emitted by convert for task lists",
    ),
    ('✅', "white heavy check mark U+2705 — 28 of 146 documents"),
    ('❌', "cross mark U+274C — 8 of 146 documents"),
    ('❓', "question mark ornament U+2753"),
    ('❗', "exclamation mark ornament U+2757"),
    ('🔴', "red circle U+1F534 — 7 of 146 documents"),
    ('📋', "clipboard U+1F4CB"),
    ('🤔', "thinking face U+1F914"),
    ('😊', "smiling face U+1F60A"),
];

#[test]
fn every_character_the_corpus_uses_has_a_glyph() {
    let ts = Typesetter::new();
    let missing = ts.uncovered(CORPUS_CHARACTERS.iter().map(|(c, _)| *c));
    let described: Vec<String> = missing
        .iter()
        .map(|c| {
            let why = CORPUS_CHARACTERS
                .iter()
                .find(|(m, _)| m == c)
                .map(|(_, d)| *d)
                .unwrap_or("");
            format!("  {c}  {why}")
        })
        .collect();
    assert!(
        missing.is_empty(),
        "the shipped FontBook has no glyph for:\n{}\n\
         These render as an empty box where the author put a character. \
         See `design/plan-glyphs.md`.",
        described.join("\n")
    );
}

// ---------------------------------------------------------------------------------
// 3f — Overrides are permissions the ladder measures under, not decisions.
// ---------------------------------------------------------------------------------

/// A converted table too wide for the page — with a reflow alternate, so the ladder's
/// default is to wrap and an Override is a real change rather than a no-op.
fn reflowing_table() -> Vec<Element> {
    let md = "| a | b | c | d |\n|---|---|---|---|\n| xxxxxxxxxxxxxxxxxxxx | xxxxxxxxxxxxxxxxxxxx \
              | xxxxxxxxxxxxxxxxxxxx | xxxxxxxxxxxxxxxxxxxx |";
    md2pdf_convert::convert(md, &md2pdf_convert::SourceContext::none()).elements
}

#[test]
fn forcing_landscape_measures_against_the_landscape_width() {
    // **The defect this phase started from.** `apply_override` used to set the axis
    // directly, leaving a size chosen against the *portrait* width on an element now
    // rendered sideways — the bug the GLOSSARY names and T14 fixed:
    //
    //   "RE-MEASURE in landscape; do not inherit the portrait size."
    //
    // A permission cannot express that mistake: the ladder runs again and reports the
    // width it actually measured against.
    let template = template();
    let elements = reflowing_table();
    let ts = Typesetter::new();

    let table = elements
        .iter()
        .find(|e| e.class == ElementClass::Table)
        .expect("a table");

    let (before, _) = ts.probe(&elements, &template).expect("probe");
    let portrait_width = before.get(&table.id).expect("decided").available_pt;

    let (after, _) = ts
        .probe_with(
            &elements,
            &template,
            &[Override {
                id: table.id,
                permit: Permit::Landscape,
            }],
        )
        .expect("probe");
    let d = after.get(&table.id).expect("decided");

    assert_eq!(d.orientation, Orientation::Landscape);
    assert!(
        d.available_pt > portrait_width,
        "the landscape decision was measured against {}pt, the portrait width — the size \
         was inherited rather than re-measured",
        d.available_pt
    );
    assert_eq!(
        d.available_pt,
        template.available_landscape_pt(),
        "measured against something that is neither width"
    );
}

#[test]
fn permitting_below_the_floor_yields_a_size_below_it() {
    // The other half: a permission is a *bound the user allows*, and the ladder still
    // stops at the first size that fits. What must not happen is the floor silently
    // winning anyway.
    let template = template();
    let elements = reflowing_table();
    let ts = Typesetter::new();
    let table = elements
        .iter()
        .find(|e| e.class == ElementClass::Table)
        .expect("a table");

    let (after, _) = ts
        .probe_with(
            &elements,
            &template,
            &[Override {
                id: table.id,
                permit: Permit::BelowFloor { to_pt: 4.0 },
            }],
        )
        .expect("probe");

    match after.get(&table.id).expect("decided").reduction {
        Reduction::Shrink { size_pt } => assert!(
            size_pt < template.floors.table_comfort_pt,
            "permitting 4pt still returned {size_pt}pt, at or above the comfort floor"
        ),
        other => panic!("expected a shrink under a below-floor permit, got {other:?}"),
    }
}

#[test]
fn an_override_for_a_stale_element_changes_nothing() {
    // The Source was edited and the id no longer names the same content. Misapplying it
    // would silently rotate or shrink whatever now sits at that position.
    let template = template();
    let elements = reflowing_table();
    let ts = Typesetter::new();

    let (before, _) = ts.probe(&elements, &template).expect("probe");
    let (after, _) = ts
        .probe_with(
            &elements,
            &template,
            &[Override {
                id: ElementId::new(0, "content that is no longer there"),
                permit: Permit::Landscape,
            }],
        )
        .expect("probe");

    assert_eq!(before, after, "a stale Override was applied anyway");
}

#[test]
fn probing_one_element_agrees_with_probing_the_document() {
    // **D1 in `design/plan-review.md`** — the risk that would pass every other test.
    //
    // The recompile loop re-probes to apply an Override. If an Element's decision
    // depended on the Elements around it, a re-probe would answer differently and every
    // override would be subtly inconsistent with its page — and rendering one page would
    // look fine, so nothing else here would catch it.
    let template = template();
    let elements = reflowing_table();
    let ts = Typesetter::new();

    let (whole, _) = ts.probe(&elements, &template).expect("probe");
    for el in &elements {
        let (alone, _) = ts
            .probe(std::slice::from_ref(el), &template)
            .expect("probe one");
        assert_eq!(
            whole.get(&el.id),
            alone.get(&el.id),
            "element {} decides differently on its own than in its document",
            el.id.order
        );
    }
}

// ---------------------------------------------------------------------------
// Which page an Element landed on. Only the final layout knows: the ProbePass
// measures outside the page flow precisely so that it can measure at all.
// ---------------------------------------------------------------------------

/// A block of a known height, so page breaks fall where the arithmetic says.
fn tall(order: u32, height_pt: u32) -> Element {
    Element::new(
        order,
        ElementClass::Prose,
        Markup::raw(format!("#block(width: 100%, height: {height_pt}pt)[.]")),
    )
}

#[test]
fn each_element_reports_the_page_it_landed_on() {
    // 320pt page, 12pt margins => 296pt of usable height. Three 150pt blocks cannot
    // share a page: one and two go to page 1 and 2 respectively once spacing is counted,
    // which is the point — the pages must *differ*, and rise.
    let template = template();
    let elements = vec![tall(0, 150), tall(1, 150), tall(2, 150), tall(3, 150)];
    let ts = Typesetter::new();
    let (map, _) = ts.probe(&elements, &template).expect("probe");
    let pages = ts
        .render(&elements, &template, &map)
        .expect("render")
        .element_pages();

    assert_eq!(pages.len(), elements.len(), "an Element reported no page");
    assert_eq!(pages[&0], 1, "the first Element is not on page 1");
    let mut last = 0;
    for order in 0..elements.len() as u32 {
        let page = pages[&order];
        assert!(
            page >= last,
            "element {order} reports page {page} after element {} reported {last}",
            order.saturating_sub(1)
        );
        last = page;
    }
    assert!(
        last > 1,
        "four 150pt blocks fitted on one 296pt page — the fixture stopped testing anything"
    );
}

#[test]
fn a_landscape_element_reports_its_own_page_not_the_one_before_it() {
    // The trap in the marker's placement. `#page(flipped: true)` *starts* a page, so a
    // marker emitted before it lands on the preceding one and every rotated table would
    // send the reader to the wrong page. It goes inside the flipped block instead.
    let template = template();
    let elements = vec![tall(0, 150), wide_table()];
    let ts = Typesetter::new();
    let (map, _) = ts.probe(&elements, &template).expect("probe");
    assert_eq!(
        map.get(&elements[1].id).unwrap().orientation,
        Orientation::Landscape,
        "the fixture stopped rotating, so this no longer tests placement"
    );

    let compilation = ts.render(&elements, &template, &map).expect("render");
    let pages = compilation.element_pages();
    let geometry = compilation.geometry();
    let table_page = pages[&elements[1].id.order];

    assert!(
        geometry[table_page as usize - 1].is_landscape(),
        "the table reports page {table_page}, which is portrait — the marker landed on \
         the page before the flipped one"
    );
}
