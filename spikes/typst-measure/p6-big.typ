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
  (id: "e0", class: "prose",  body: lorem(60)),
  (id: "e1", class: "table",  body: table(columns: 6, ..range(12).map(j => [longvalue#j]))),
  (id: "e2", class: "prose",  body: lorem(60)),
  (id: "e3", class: "code",   body: raw("fn escalate(el: &Element) -> Decision { todo!() }", lang: "rust", block: true)),
  (id: "e4", class: "figure", body: rect(width: 400pt, height: 40pt, fill: luma(230))),
  (id: "e5", class: "prose",  body: lorem(60)),
  (id: "e6", class: "table",  body: table(columns: 6, ..range(12).map(j => [longvalue#j]))),
  (id: "e7", class: "prose",  body: lorem(60)),
  (id: "e8", class: "code",   body: raw("fn escalate(el: &Element) -> Decision { todo!() }", lang: "rust", block: true)),
  (id: "e9", class: "figure", body: rect(width: 400pt, height: 40pt, fill: luma(230))),
  (id: "e10", class: "prose",  body: lorem(60)),
  (id: "e11", class: "table",  body: table(columns: 6, ..range(12).map(j => [longvalue#j]))),
  (id: "e12", class: "prose",  body: lorem(60)),
  (id: "e13", class: "code",   body: raw("fn escalate(el: &Element) -> Decision { todo!() }", lang: "rust", block: true)),
  (id: "e14", class: "figure", body: rect(width: 400pt, height: 40pt, fill: luma(230))),
  (id: "e15", class: "prose",  body: lorem(60)),
  (id: "e16", class: "table",  body: table(columns: 6, ..range(12).map(j => [longvalue#j]))),
  (id: "e17", class: "prose",  body: lorem(60)),
  (id: "e18", class: "code",   body: raw("fn escalate(el: &Element) -> Decision { todo!() }", lang: "rust", block: true)),
  (id: "e19", class: "figure", body: rect(width: 400pt, height: 40pt, fill: luma(230))),
  (id: "e20", class: "prose",  body: lorem(60)),
  (id: "e21", class: "table",  body: table(columns: 6, ..range(12).map(j => [longvalue#j]))),
  (id: "e22", class: "prose",  body: lorem(60)),
  (id: "e23", class: "code",   body: raw("fn escalate(el: &Element) -> Decision { todo!() }", lang: "rust", block: true)),
  (id: "e24", class: "figure", body: rect(width: 400pt, height: 40pt, fill: luma(230))),
  (id: "e25", class: "prose",  body: lorem(60)),
  (id: "e26", class: "table",  body: table(columns: 6, ..range(12).map(j => [longvalue#j]))),
  (id: "e27", class: "prose",  body: lorem(60)),
  (id: "e28", class: "code",   body: raw("fn escalate(el: &Element) -> Decision { todo!() }", lang: "rust", block: true)),
  (id: "e29", class: "figure", body: rect(width: 400pt, height: 40pt, fill: luma(230))),
  (id: "e30", class: "prose",  body: lorem(60)),
  (id: "e31", class: "table",  body: table(columns: 6, ..range(12).map(j => [longvalue#j]))),
  (id: "e32", class: "prose",  body: lorem(60)),
  (id: "e33", class: "code",   body: raw("fn escalate(el: &Element) -> Decision { todo!() }", lang: "rust", block: true)),
  (id: "e34", class: "figure", body: rect(width: 400pt, height: 40pt, fill: luma(230))),
  (id: "e35", class: "prose",  body: lorem(60)),
  (id: "e36", class: "table",  body: table(columns: 6, ..range(12).map(j => [longvalue#j]))),
  (id: "e37", class: "prose",  body: lorem(60)),
  (id: "e38", class: "code",   body: raw("fn escalate(el: &Element) -> Decision { todo!() }", lang: "rust", block: true)),
  (id: "e39", class: "figure", body: rect(width: 400pt, height: 40pt, fill: luma(230))),
  (id: "e40", class: "prose",  body: lorem(60)),
  (id: "e41", class: "table",  body: table(columns: 6, ..range(12).map(j => [longvalue#j]))),
  (id: "e42", class: "prose",  body: lorem(60)),
  (id: "e43", class: "code",   body: raw("fn escalate(el: &Element) -> Decision { todo!() }", lang: "rust", block: true)),
  (id: "e44", class: "figure", body: rect(width: 400pt, height: 40pt, fill: luma(230))),
  (id: "e45", class: "prose",  body: lorem(60)),
  (id: "e46", class: "table",  body: table(columns: 6, ..range(12).map(j => [longvalue#j]))),
  (id: "e47", class: "prose",  body: lorem(60)),
  (id: "e48", class: "code",   body: raw("fn escalate(el: &Element) -> Decision { todo!() }", lang: "rust", block: true)),
  (id: "e49", class: "figure", body: rect(width: 400pt, height: 40pt, fill: luma(230))),
  (id: "e50", class: "prose",  body: lorem(60)),
  (id: "e51", class: "table",  body: table(columns: 6, ..range(12).map(j => [longvalue#j]))),
  (id: "e52", class: "prose",  body: lorem(60)),
  (id: "e53", class: "code",   body: raw("fn escalate(el: &Element) -> Decision { todo!() }", lang: "rust", block: true)),
  (id: "e54", class: "figure", body: rect(width: 400pt, height: 40pt, fill: luma(230))),
  (id: "e55", class: "prose",  body: lorem(60)),
  (id: "e56", class: "table",  body: table(columns: 6, ..range(12).map(j => [longvalue#j]))),
  (id: "e57", class: "prose",  body: lorem(60)),
  (id: "e58", class: "code",   body: raw("fn escalate(el: &Element) -> Decision { todo!() }", lang: "rust", block: true)),
  (id: "e59", class: "figure", body: rect(width: 400pt, height: 40pt, fill: luma(230))),
  (id: "e60", class: "prose",  body: lorem(60)),
  (id: "e61", class: "table",  body: table(columns: 6, ..range(12).map(j => [longvalue#j]))),
  (id: "e62", class: "prose",  body: lorem(60)),
  (id: "e63", class: "code",   body: raw("fn escalate(el: &Element) -> Decision { todo!() }", lang: "rust", block: true)),
  (id: "e64", class: "figure", body: rect(width: 400pt, height: 40pt, fill: luma(230))),
  (id: "e65", class: "prose",  body: lorem(60)),
  (id: "e66", class: "table",  body: table(columns: 6, ..range(12).map(j => [longvalue#j]))),
  (id: "e67", class: "prose",  body: lorem(60)),
  (id: "e68", class: "code",   body: raw("fn escalate(el: &Element) -> Decision { todo!() }", lang: "rust", block: true)),
  (id: "e69", class: "figure", body: rect(width: 400pt, height: 40pt, fill: luma(230))),
  (id: "e70", class: "prose",  body: lorem(60)),
  (id: "e71", class: "table",  body: table(columns: 6, ..range(12).map(j => [longvalue#j]))),
  (id: "e72", class: "prose",  body: lorem(60)),
  (id: "e73", class: "code",   body: raw("fn escalate(el: &Element) -> Decision { todo!() }", lang: "rust", block: true)),
  (id: "e74", class: "figure", body: rect(width: 400pt, height: 40pt, fill: luma(230))),
  (id: "e75", class: "prose",  body: lorem(60)),
  (id: "e76", class: "table",  body: table(columns: 6, ..range(12).map(j => [longvalue#j]))),
  (id: "e77", class: "prose",  body: lorem(60)),
  (id: "e78", class: "code",   body: raw("fn escalate(el: &Element) -> Decision { todo!() }", lang: "rust", block: true)),
  (id: "e79", class: "figure", body: rect(width: 400pt, height: 40pt, fill: luma(230))),
  (id: "e80", class: "prose",  body: lorem(60)),
  (id: "e81", class: "table",  body: table(columns: 6, ..range(12).map(j => [longvalue#j]))),
  (id: "e82", class: "prose",  body: lorem(60)),
  (id: "e83", class: "code",   body: raw("fn escalate(el: &Element) -> Decision { todo!() }", lang: "rust", block: true)),
  (id: "e84", class: "figure", body: rect(width: 400pt, height: 40pt, fill: luma(230))),
  (id: "e85", class: "prose",  body: lorem(60)),
  (id: "e86", class: "table",  body: table(columns: 6, ..range(12).map(j => [longvalue#j]))),
  (id: "e87", class: "prose",  body: lorem(60)),
  (id: "e88", class: "code",   body: raw("fn escalate(el: &Element) -> Decision { todo!() }", lang: "rust", block: true)),
  (id: "e89", class: "figure", body: rect(width: 400pt, height: 40pt, fill: luma(230))),
  (id: "e90", class: "prose",  body: lorem(60)),
  (id: "e91", class: "table",  body: table(columns: 6, ..range(12).map(j => [longvalue#j]))),
  (id: "e92", class: "prose",  body: lorem(60)),
  (id: "e93", class: "code",   body: raw("fn escalate(el: &Element) -> Decision { todo!() }", lang: "rust", block: true)),
  (id: "e94", class: "figure", body: rect(width: 400pt, height: 40pt, fill: luma(230))),
  (id: "e95", class: "prose",  body: lorem(60)),
  (id: "e96", class: "table",  body: table(columns: 6, ..range(12).map(j => [longvalue#j]))),
  (id: "e97", class: "prose",  body: lorem(60)),
  (id: "e98", class: "code",   body: raw("fn escalate(el: &Element) -> Decision { todo!() }", lang: "rust", block: true)),
  (id: "e99", class: "figure", body: rect(width: 400pt, height: 40pt, fill: luma(230))),
  (id: "e100", class: "prose",  body: lorem(60)),
  (id: "e101", class: "table",  body: table(columns: 6, ..range(12).map(j => [longvalue#j]))),
  (id: "e102", class: "prose",  body: lorem(60)),
  (id: "e103", class: "code",   body: raw("fn escalate(el: &Element) -> Decision { todo!() }", lang: "rust", block: true)),
  (id: "e104", class: "figure", body: rect(width: 400pt, height: 40pt, fill: luma(230))),
  (id: "e105", class: "prose",  body: lorem(60)),
  (id: "e106", class: "table",  body: table(columns: 6, ..range(12).map(j => [longvalue#j]))),
  (id: "e107", class: "prose",  body: lorem(60)),
  (id: "e108", class: "code",   body: raw("fn escalate(el: &Element) -> Decision { todo!() }", lang: "rust", block: true)),
  (id: "e109", class: "figure", body: rect(width: 400pt, height: 40pt, fill: luma(230))),
  (id: "e110", class: "prose",  body: lorem(60)),
  (id: "e111", class: "table",  body: table(columns: 6, ..range(12).map(j => [longvalue#j]))),
  (id: "e112", class: "prose",  body: lorem(60)),
  (id: "e113", class: "code",   body: raw("fn escalate(el: &Element) -> Decision { todo!() }", lang: "rust", block: true)),
  (id: "e114", class: "figure", body: rect(width: 400pt, height: 40pt, fill: luma(230))),
  (id: "e115", class: "prose",  body: lorem(60)),
  (id: "e116", class: "table",  body: table(columns: 6, ..range(12).map(j => [longvalue#j]))),
  (id: "e117", class: "prose",  body: lorem(60)),
  (id: "e118", class: "code",   body: raw("fn escalate(el: &Element) -> Decision { todo!() }", lang: "rust", block: true)),
  (id: "e119", class: "figure", body: rect(width: 400pt, height: 40pt, fill: luma(230))),
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
