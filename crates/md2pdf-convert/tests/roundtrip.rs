//! The escaping oracle: text in -> compile -> text out -> compare.
//!
//! These tests do not check md2pdf's arithmetic. They check a property of the real
//! Typst compiler: that text md2pdf emits renders as the text the document contained.
//!
//! The element body is compiled through the **production** render path, not a bespoke
//! harness, so the test exercises exactly what ships.

use md2pdf_convert::escape::escape;
use md2pdf_domain::{Decision, DecisionMap, Element, ElementClass, Markup, Rung, Template};
use md2pdf_typeset::Typesetter;

/// Render one piece of escaped text and return the plain text Typst produced.
///
/// `rung` selects the syntactic position the body lands in — `None` puts it raw at
/// top level, everything else wraps it in a content block `[...]`. Both are
/// production paths; see `design/plan-conversion-crate.md` §1.1.
fn render_text(input: &str, rung: Rung) -> Result<String, String> {
    let el = Element::new(0, ElementClass::Prose, Markup::raw(escape(input)));
    let map = DecisionMap {
        decisions: vec![Decision {
            id: el.id,
            rung,
            natural_pt: 0.0,
            available_pt: 0.0,
        }],
    };
    Typesetter::new()
        .render(&[el], &Template::default(), &map)
        .map(|c| c.text())
        .map_err(|e| e.to_string())
}

/// Typst breaks lines and may render a space as a line break; neither is a
/// correctness failure for escaping. Compare on non-whitespace content.
fn normalise(s: &str) -> String {
    s.split_whitespace().collect::<Vec<_>>().join(" ")
}

/// Every ASCII punctuation character, alone and in the shapes that break parsers.
fn candidates() -> Vec<(String, String)> {
    let mut v: Vec<(String, String)> = Vec::new();
    for c in "!\"#$%&'()*+,-./:;<=>?@[\\]^_`{|}~".chars() {
        v.push((format!("char {c:?}"), format!("a{c}b")));
        v.push((format!("doubled {c:?}"), format!("a{c}{c}b")));
        v.push((format!("leading {c:?}"), format!("{c}b")));
    }
    for (name, s) in [
        ("injection: let binding", "#let x = 1"),
        ("injection: lorem call", "#lorem(50)"),
        ("injection: import", "#import \"evil.typ\""),
        ("unbalanced close bracket", "a ] b"),
        ("unbalanced open bracket", "a [ b"),
        ("emphasis pair", "a *bold* b"),
        ("underscore pair", "snake_case_name"),
        ("math pair", "cost is $5 and $6"),
        ("label ref", "see @figure1"),
        ("backslash run", r"a \\ b \n c"),
        ("html-ish", "a <tag> b"),
        ("smart quotes", "he said \"hello\" and 'bye'"),
        ("typst comment", "a // not a comment"),
        ("typst block comment", "a /* not a comment */ b"),
        ("hash at line start", "#hashtag trending"),
        ("dash sequences", "a -- b --- c"),
        ("ellipsis", "wait ... what"),
    ] {
        v.push((name.to_string(), s.to_string()));
    }
    v
}

/// Reports the danger surface rather than asserting it. Run with `--nocapture` and
/// `--ignored` to regenerate the evidence behind `escape`'s character set.
#[test]
#[ignore = "evidence-gathering, not a gate; run with --ignored --nocapture"]
fn report_roundtrip_failures() {
    let mut broken = Vec::new();
    for (name, input) in candidates() {
        match render_text(&input, Rung::None) {
            Ok(out) if normalise(&out) == normalise(&input) => {}
            Ok(out) => broken.push(format!(
                "MANGLED {name:<28} {input:?} -> {:?}",
                normalise(&out)
            )),
            Err(e) => broken.push(format!("FAILED  {name:<28} {input:?} -> {e}")),
        }
    }
    println!(
        "\n{} of {} candidates broken:",
        broken.len(),
        candidates().len()
    );
    for b in &broken {
        println!("  {b}");
    }
}

#[test]
fn text_survives_round_trip_at_top_level() {
    for (name, input) in candidates() {
        let out = render_text(&input, Rung::None)
            .unwrap_or_else(|e| panic!("{name}: {input:?} failed to compile: {e}"));
        assert_eq!(
            normalise(&out),
            normalise(&input),
            "{name}: {input:?} did not survive round-trip"
        );
    }
}

#[test]
fn escaped_text_cannot_corrupt_the_probe_harness() {
    // The third syntactic position, and the dangerous one: the ProbePass wraps the
    // body in `[...]` *inside* a `#context` block alongside `measure()` calls. A body
    // that escapes its brackets here does not fail loudly — it silently truncates the
    // harness and corrupts the measurement of an unrelated element.
    //
    // The probe discards its own rendering, so there is no text to compare. Compiling
    // clean and yielding a sane decision is the assertion.
    let template = Template::default();
    for (name, input) in candidates() {
        let el = Element::new(0, ElementClass::Prose, Markup::raw(escape(&input)));
        let id = el.id;
        let (map, _diagnostic) = Typesetter::new()
            .probe(&[el], &template)
            .unwrap_or_else(|e| panic!("{name}: {input:?} broke the probe harness: {e}"));

        let decision = map
            .get(&id)
            .unwrap_or_else(|| panic!("{name}: {input:?} produced no decision"));
        assert_eq!(
            decision.rung,
            Rung::None,
            "{name}: {input:?} — Prose is Wrappable and must never escalate"
        );
    }
}

#[test]
fn text_survives_round_trip_inside_a_content_block() {
    // The position every probe uses. An unbalanced bracket here corrupts the
    // harness itself, so this is the case that matters most.
    for (name, input) in candidates() {
        let out = render_text(&input, Rung::Clip)
            .unwrap_or_else(|e| panic!("{name}: {input:?} failed to compile: {e}"));
        let out = normalise(&out);
        let expected = normalise(&input);
        assert!(
            out.contains(&expected),
            "{name}: {input:?} did not survive inside a content block: got {out:?}"
        );
    }
}
