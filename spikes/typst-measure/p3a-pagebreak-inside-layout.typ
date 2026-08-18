// P3a — Can the ladder's rung-3 action (rotate onto its own landscape page)
// be taken INSIDE the layout() callback that discovered it was needed?
// Docs say no. Verify.
#set page(width: 220pt, height: 300pt, margin: 12pt)
#set text(size: 10pt, font: "Libertinus Serif")
Before.
#layout(size => {
  pagebreak()
  [inside layout after pagebreak]
})
After.
