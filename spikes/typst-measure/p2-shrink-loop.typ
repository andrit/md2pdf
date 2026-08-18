// P2 — Can we step font size down toward a per-class floor and pick the
// largest size that fits? This is rung 1 of the escalation ladder.

#set page(width: 220pt, height: auto, margin: 12pt)
#set text(size: 10pt, font: "Libertinus Serif")

#let wide-table = table(
  columns: 6,
  ..range(6).map(i => [*Col #i*]),
  ..range(6).map(i => [longvalue#i]),
)

#let code-block = raw(
  "fn escalate(el: &Element, avail: Length) -> Decision { todo!() }",
  lang: "rust",
  block: true,
)

// Candidate sizes, largest first, stopping at the class floor.
#let candidates(base, floor, step) = {
  let out = ()
  let s = base
  while s >= floor {
    out.push(s)
    s = s - step
  }
  out
}

// Rung 1: largest candidate whose measured width fits. `none` => at floor, still over.
#let fit-size(el, avail, sizes) = {
  let chosen = none
  for s in sizes {
    if chosen == none {
      if measure(text(size: s, el)).width <= avail { chosen = s }
    }
  }
  chosen
}

#let probe(id, el, base, floor) = layout(size => {
  let sizes = candidates(base, floor, 0.5pt)
  let chosen = fit-size(el, size.width, sizes)
  [#metadata((
    id: id,
    available: size.width.pt(),
    base: base.pt(),
    floor: floor.pt(),
    tried: sizes.len(),
    natural-at-base: measure(text(size: base, el)).width.pt(),
    width-at-floor: measure(text(size: floor, el)).width.pt(),
    chosen: if chosen == none { none } else { chosen.pt() },
    at-floor-still-over: chosen == none,
  )) <probe>]
})

// Table class: floor 7pt. Prose class: floor 9pt.
#probe("wide-table", wide-table, 10pt, 7pt)
#probe("code-block", code-block, 10pt, 7pt)
#probe("prose", lorem(20), 10pt, 9pt)
