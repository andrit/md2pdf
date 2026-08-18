// P5 — Full two-pass loop.
//
//   pass 1 (probe):  measure every element, emit a decision as metadata
//   host:            typst eval -> decisions.json
//   pass 2 (render): read decisions.json, apply each decision AT TOP LEVEL
//                    where pagebreak() and page(flipped:true) are legal
//
// Run with:  typst compile p5-two-pass.typ --input pass=probe|render

#let pass = sys.inputs.at("pass", default: "probe")

#set page(width: 220pt, height: 320pt, margin: 12pt)
#set text(size: 10pt, font: "Libertinus Serif")

// --- the document: md2pdf would emit this from pulldown-cmark ------------
// Each element carries the id and class md2pdf assigns at emission time.
#let doc = (
  (id: "e0", class: "prose",  body: lorem(18)),
  (id: "e1", class: "table",  body: table(columns: 6, ..range(6).map(i => [longvalue#i]))),
  (id: "e2", class: "prose",  body: lorem(12)),
  (id: "e3", class: "table",  body: table(columns: 3, [a], [b], [c], [1], [2], [3])),
  (id: "e4", class: "figure", body: rect(width: 400pt, height: 30pt, fill: luma(230))),
)

// Per-class floors, as they would live in template.toml.
#let floors = (prose: 9pt, table: 7pt, code: 7pt, figure: 0pt)
#let base = 10pt

// Only atomic classes can overflow horizontally; prose/code wrap. (P2b/P2c)
#let atomic = ("table", "figure")

// --- pass 1: probe -------------------------------------------------------
#let probe(el) = layout(size => {
  let avail = size.width
  let decision = if el.class not in atomic {
    (rung: "none", size: none)
  } else {
    let floor = floors.at(el.class)
    // step down toward the floor, take the largest that fits
    let chosen = none
    let s = base
    while s >= floor and chosen == none {
      if measure(text(size: s, el.body)).width <= avail { chosen = s }
      s = s - 0.5pt
    }
    if chosen == base { (rung: "none", size: none) }
    else if chosen != none { (rung: "shrink", size: chosen.pt()) }
    else { (rung: "rotate", size: floor.pt()) }
  }
  [#metadata((
    id: el.id,
    class: el.class,
    available: avail.pt(),
    natural: measure(el.body).width.pt(),
    ..decision,
  )) <decision>]
})

// --- pass 2: render ------------------------------------------------------
#let render(el, decisions) = {
  let d = decisions.find(x => x.id == el.id)
  if d == none or d.rung == "none" {
    el.body
  } else if d.rung == "shrink" {
    text(size: d.size * 1pt, el.body)
  } else if d.rung == "rotate" {
    // LEGAL HERE — top level, not inside layout()
    page(flipped: true, margin: 12pt)[
      #set text(size: d.size * 1pt)
      #rotate(0deg, reflow: true, el.body)
    ]
  }
}

// --- drive ---------------------------------------------------------------
#if pass == "probe" {
  for el in doc { probe(el) }
} else {
  let decisions = json("decisions.json")
  for el in doc {
    render(el, decisions)
    v(6pt)
  }
}
