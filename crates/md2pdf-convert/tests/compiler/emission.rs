//! The §1.1 body invariant, enforced against the real compiler.
//!
//! Every emitted body is interpolated into Typst source in three positions:
//! raw at top level (`Reduction::None`), inside a content block (`[...]`, used by every
//! shrink/rotate/clip), and inside the ProbePass harness alongside `measure()`.
//!
//! A body that is valid in one and not another does not fail loudly — in the probe
//! case it silently truncates the harness and corrupts an *unrelated* element's
//! measurement. So all three are tested, on a corpus covering every construct md2pdf
//! emits.

use md2pdf_convert::{emit::emit, parse::parse, SourceContext};
use md2pdf_domain::{Decision, DecisionMap, Element, Orientation, Reduction, Template};
use md2pdf_typeset::Typesetter;

/// One document exercising every construct the emitter can produce, including the
/// adversarial text that motivated the escaping module.
const CORPUS: &str = r##"---
title: Front matter that must not render
---

# Heading One

Prose with *emphasis*, **strong**, ~~struck~~, `inline code`, and a [link](https://x.test).

Text with Typst-hostile characters: #hash $dollar [bracket] @at ` backtick \ backslash
and shorthands -- like --- these ... too.

## Heading Two

- bullet one
- bullet two with `code`
  - nested bullet

1. numbered
2. numbered again

- [x] done task
- [ ] pending task

> A blockquote with *emphasis*.

> [!WARNING]
> An alert blockquote.

```rust
fn main() { println!("hi #not_a_hash"); }
```

```
fenced with no language
```

    indented code block

| Column A | Column B |
|----------|----------|
| cell 1   | cell 2   |
| `code`   | *em*     |

![a diagram](missing-image.png)

A claim needing support[^1] and one that is undefined[^nope].

<div>raw html block</div>

---

Final paragraph.

[^1]: The footnote body, with *emphasis*.
"##;

fn template() -> Template {
    Template::default()
}

fn elements() -> Vec<Element> {
    let out = emit(&parse(CORPUS), &SourceContext::none());
    assert!(!out.elements.is_empty(), "corpus produced no elements");
    out.elements
}

#[test]
fn the_whole_corpus_compiles_in_the_probe_pass() {
    // The dangerous position: bodies sit inside `[...]` next to `measure()` calls.
    let elements = elements();
    let (map, _) = Typesetter::new()
        .probe(&elements, &template())
        .expect("corpus must survive the ProbePass");

    // Every element must come back with a decision, or harvest lost one.
    for el in &elements {
        assert!(
            map.get(&el.id).is_some(),
            "no decision harvested for element {} ({:?})",
            el.id.order,
            el.class
        );
    }
}

#[test]
fn the_whole_corpus_compiles_in_every_render_position() {
    let elements = elements();
    let template = template();

    // Both axes, in the combinations the ladder can actually produce.
    for (orientation, reduction) in [
        (Orientation::Portrait, Reduction::None),
        (Orientation::Portrait, Reduction::Shrink { size_pt: 8.0 }),
        (Orientation::Landscape, Reduction::None),
        (Orientation::Landscape, Reduction::Shrink { size_pt: 8.0 }),
        (Orientation::Portrait, Reduction::Clip),
    ] {
        let map = DecisionMap {
            decisions: elements
                .iter()
                .map(|el| Decision {
                    id: el.id,
                    orientation,
                    reduction,
                    natural_pt: 0.0,
                    available_pt: 0.0,
                })
                .collect(),
        };
        let compilation = Typesetter::new()
            .render(&elements, &template, &map)
            .unwrap_or_else(|e| panic!("corpus failed at {orientation:?}/{reduction:?}: {e}"));
        assert!(
            compilation.page_count() > 0,
            "no pages at {orientation:?}/{reduction:?}"
        );
    }
}

#[test]
fn the_corpus_produces_a_pdf() {
    // End to end: the point of the crate.
    let elements = elements();
    let (map, _) = Typesetter::new().probe(&elements, &template()).unwrap();
    let pdf = Typesetter::new()
        .render(&elements, &template(), &map)
        .unwrap()
        .pdf()
        .expect("PDF export");
    assert!(pdf.starts_with(b"%PDF"), "not a PDF");
    assert!(
        pdf.len() > 1000,
        "suspiciously small PDF: {} bytes",
        pdf.len()
    );
}

#[test]
fn front_matter_and_hostile_text_survive_correctly() {
    let elements = elements();
    let (map, _) = Typesetter::new().probe(&elements, &template()).unwrap();
    let text = Typesetter::new()
        .render(&elements, &template(), &map)
        .unwrap()
        .text();

    // D3: front matter is metadata, not content.
    assert!(
        !text.contains("Front matter that must not render"),
        "front matter leaked into the PDF"
    );
    // T5: hostile characters render literally rather than executing or vanishing.
    for needle in ["#hash", "$dollar", "[bracket]", "@at"] {
        assert!(
            text.contains(needle),
            "{needle:?} did not survive: {text:?}"
        );
    }
    // Shorthands must not have been transformed into en/em dashes or an ellipsis.
    assert!(text.contains("--"), "en dash substitution leaked in");
    assert!(text.contains("..."), "ellipsis substitution leaked in");
    // Code content must not be markup-escaped.
    assert!(
        text.contains("#not_a_hash"),
        "code block content was mangled: {text:?}"
    );
}

#[test]
fn every_concession_is_recorded_and_addressable() {
    let out = emit(&parse(CORPUS), &SourceContext::none());
    assert!(
        !out.compromises.is_empty(),
        "the corpus has an image and raw html; both are concessions"
    );
    // A Compromise is only useful if it points at an Element the user can act on.
    for c in &out.compromises {
        assert!(
            out.elements.iter().any(|e| e.id.matches(&c.id)),
            "compromise {:?} points at no element",
            c.kind
        );
    }
}
