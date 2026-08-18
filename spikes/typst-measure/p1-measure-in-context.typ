// P1 — Does layout(size => measure(el, ...)) yield a usable overflow verdict?
// Also tests the crux of the two-pass design: can metadata emitted from INSIDE
// a layout() callback be pulled back out with `typst query`?

#set page(width: 220pt, height: auto, margin: 12pt)
#set text(size: 10pt, font: "Libertinus Serif")

// Deliberately over-wide: 6 columns of longish cells in a 196pt text block.
#let wide-table = table(
  columns: 6,
  ..range(6).map(i => [*Col #i*]),
  ..range(6).map(i => [longvalue#i]),
)

// Comfortably narrow.
#let narrow-para = [Short line.]

#let probe(id, el) = layout(size => {
  let natural = measure(el)                       // auto = infinite space -> natural width
  let constrained = measure(el, width: size.width) // measured within available width
  [#metadata((
    id: id,
    available: size.width.pt(),
    natural-width: natural.width.pt(),
    natural-height: natural.height.pt(),
    constrained-width: constrained.width.pt(),
    constrained-height: constrained.height.pt(),
    overflows: natural.width > size.width,
  )) <probe>]
})

#probe("wide-table", wide-table)
#probe("narrow-para", narrow-para)

// Render them too, so we can eyeball whether the verdict matches reality.
#wide-table
#narrow-para
