// Font specimen — same content, one sans candidate per render.
// Mono is held constant (JetBrains Mono) so the only variable is the body sans.
#let sans = sys.inputs.at("sans", default: "Inter")

#set page(width: 430pt, height: auto, margin: 26pt)
#set text(font: sans, size: 10pt, lang: "en")
#set par(justify: false, leading: 0.62em, spacing: 0.9em)
#show raw: set text(font: "JetBrains Mono", size: 8.6pt)
#show heading.where(level: 1): set text(size: 17pt, weight: 700)
#show heading.where(level: 2): set text(size: 12.5pt, weight: 600)
#show heading: it => block(above: 1.1em, below: 0.55em, it)

#place(top + right, dx: 0pt, dy: -14pt, text(size: 7.5pt, fill: luma(130))[#sans])

= Converting Markdown to PDF

Body copy at 10pt, which is where this typeface has to earn its keep. The bar is
*however markdown renders on GitHub* — so headings need clear hierarchy, prose needs
to read comfortably at length, and inline code like `measure(el).width` must sit in a
line of text without disrupting it. Note the *italic* and the *#text(weight: 700)[bold]*,
both drawn from the variable weight axis.

== The escalation ladder

Elements divide into two kinds, and only one of them can overflow:

- *Atomic* — tables, images, fixed-width blocks. Natural width is required width.
- *Wrappable* — prose, lists, quotes, code. Reflows to any width, so it never
  overflows horizontally.

#table(
  columns: (auto, 1fr, auto),
  stroke: 0.4pt + luma(180),
  inset: 5pt,
  table.header([*Measured*], [*Response*], [*Rung*]),
  [fits], [render normally], [0],
  [slightly over], [step the size down toward the class floor], [1],
  [at floor, still over], [rotate to landscape on its own page], [2],
  [extreme], [clip, with a visible marker], [3],
)

The probe pass decides but cannot act, because `pagebreak()` is illegal inside
`layout()`. The render pass acts but never measures.

```rust
fn escalate(el: &Element, avail: Abs) -> Decision {
    if !el.class.is_atomic() { return Decision::None; }
    match shrink_to_fit(el, avail) {
        Some(size) => Decision::Shrink(size),
        None => Decision::Rotate,
    }
}
```

#block(
  fill: luma(246), inset: 9pt, radius: 2pt, width: 100%,
  [*47 converted cleanly. 3 need your attention.* \
   #text(size: 9pt)[`api-reference.md` — table on p.4 shrunk to floor, still overflowing]],
)

Numerals in running text: 0123456789 — 1,284 documents, 96.4% clean, £12 / \$18 / €15.
