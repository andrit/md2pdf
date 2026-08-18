// P3b — same question for an explicit page() call (the landscape rung).
#set page(width: 220pt, height: 300pt, margin: 12pt)
#set text(size: 10pt, font: "Libertinus Serif")
Before.
#layout(size => {
  page(flipped: true)[landscape content]
})
After.
