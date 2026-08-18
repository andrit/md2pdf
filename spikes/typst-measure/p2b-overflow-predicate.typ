// P2b — What is the correct overflow predicate?
//
// P2 reported prose as "at floor, still overflowing", which is wrong: prose
// WRAPS. Its natural (infinite-space) width is just "all on one line" and is
// meaningless. So natural-width > available is only a valid test for ATOMIC
// content. For wrappable content the test must be the CONSTRAINED measurement,
// which only exceeds the available width when there is an unbreakable child.

#set page(width: 220pt, height: auto, margin: 12pt)
#set text(size: 10pt, font: "Libertinus Serif")

#let long-url = "https://example.com/a/very/long/unbreakable/path/segment/that/cannot/wrap/anywhere"

#let cases = (
  ("prose-plain",      lorem(20)),
  ("prose-long-token", [Text then #raw(long-url) then more text.]),
  ("table-wide",       table(columns: 6, ..range(6).map(i => [longvalue#i]))),
  ("code-wide",        raw("fn escalate(el: &Element, avail: Length) -> Decision { todo!() }", lang: "rust", block: true)),
  ("image-ish",        rect(width: 400pt, height: 20pt)),
)

#for (id, el) in cases {
  layout(size => {
    let natural = measure(el)
    let constrained = measure(el, width: size.width)
    [#metadata((
      id: id,
      available: size.width.pt(),
      natural-width: natural.width.pt(),
      constrained-width: constrained.width.pt(),
      constrained-height: constrained.height.pt(),
      // naive predicate (what the design assumed)
      naive-overflow: natural.width > size.width,
      // proposed predicate: only the constrained measurement can't be honoured
      true-overflow: constrained.width > size.width + 0.01pt,
    )) <probe>]
  })
}
