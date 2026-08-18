// P2c — ground truth: does ink actually cross the margin?
// Red frame marks the text block. Anything drawn outside it genuinely overflows.
#set page(width: 220pt, height: auto, margin: 12pt, background: place(
  top + left, dx: 12pt, dy: 12pt, rect(width: 196pt, height: 100%, stroke: 0.5pt + red)
))
#set text(size: 10pt, font: "Libertinus Serif")
#let long = "https://example.com/a/very/long/unbreakable/path/segment/that/cannot/wrap"

*prose-plain* #lorem(20)
#v(4pt)
*prose-long-token* Text then #raw(long) then more.
#v(4pt)
*table-wide* #table(columns: 6, ..range(6).map(i => [longvalue#i]))
#v(4pt)
*code-wide* #raw("fn escalate(el: &Element, avail: Length) -> Decision { todo!() }", lang: "rust", block: true)
#v(4pt)
*rect-400* #rect(width: 400pt, height: 12pt)
