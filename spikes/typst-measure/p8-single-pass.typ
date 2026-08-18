// P8 — If the probe does NOT use layout(), is the two-pass split still needed?
//
// P3 proved pagebreak()/page() are illegal inside layout() because layout()
// forces a block-level container. But Q1 just showed layout() is unnecessary:
// md2pdf computes available width from the template itself.
//
// So: can a plain `context` block measure AND rotate, in ONE pass?
#set page(width: 220pt, height: 320pt, margin: 12pt)
#set text(size: 10pt)
#let avail = 196pt
#let wide = table(columns: 6, ..range(6).map(i => [longvalue#i]))

Before.

#context {
  if measure(wide).width > avail {
    page(flipped: true, margin: 12pt)[#wide]
  } else {
    wide
  }
}

After.
