# FontBook

Vendored, not fetched at build time, so builds are reproducible and offline.

| File | Family | Role | Licence |
|---|---|---|---|
| `SourceSans3.ttf` | Source Sans 3 | body | SIL OFL 1.1 |
| `JetBrainsMono.ttf` | JetBrains Mono | code | SIL OFL 1.1 |

Both are variable fonts; Typst 0.15.1 resolves weight and italic from the axes, so
no static instances are needed.

Typst embeds no sans-serif of its own — this is why md2pdf ships a FontBook at all.
Bundling these is what makes PDF output identical across macOS, Windows and Linux.

OFL permits both bundling and PDF embedding. `OFL.txt` must ship alongside the
binary; see `docs/` for the distribution checklist.

Chosen by rendering an identical specimen through four candidates — see
`design/spike-typst-measure-findings.md`. Source Sans 3 was the most compact
(fewest pages) and is designed for print as well as screen.
