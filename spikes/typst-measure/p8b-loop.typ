#set page(width: 220pt, height: 320pt, margin: 12pt)
#set text(size: 10pt)
#let avail = 196pt
#let floors = (prose: 9pt, table: 7pt, figure: 0pt)
#let atomic = ("table", "figure")
#let doc = (
  (id: "e0", class: "prose",  body: lorem(18)),
  (id: "e1", class: "table",  body: table(columns: 6, ..range(6).map(i => [longvalue#i]))),
  (id: "e2", class: "table",  body: table(columns: 3, [a],[b],[c])),
  (id: "e3", class: "figure", body: rect(width: 400pt, height: 30pt)),
)
#for el in doc {
  context {
    if el.class not in atomic {
      [#metadata((id: el.id, rung: "none")) <d>]
      el.body; v(6pt)
    } else {
      let floor = floors.at(el.class)
      let chosen = none
      let s = 10pt
      while s >= floor and chosen == none {
        if measure(text(size: s, el.body)).width <= avail { chosen = s }
        s = s - 0.5pt
      }
      if chosen == 10pt {
        [#metadata((id: el.id, rung: "none")) <d>]
        el.body; v(6pt)
      } else if chosen != none {
        [#metadata((id: el.id, rung: "shrink", size: chosen.pt())) <d>]
        text(size: chosen, el.body); v(6pt)
      } else {
        [#metadata((id: el.id, rung: "rotate")) <d>]
        page(flipped: true, margin: 12pt)[#el.body]
      }
    }
  }
}
