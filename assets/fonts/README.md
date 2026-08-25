# FontBook

Vendored, not fetched at build time, so builds are reproducible and offline.

| File | Family | Role | Licence |
|---|---|---|---|
| `SourceSans3.ttf` | Source Sans 3 | body, upright — variable `wght` | SIL OFL 1.1 |
| `SourceSans3-It.ttf` | Source Sans 3 | body, italic (static) | SIL OFL 1.1 |
| `SourceSans3-BoldIt.ttf` | Source Sans 3 | body, bold italic (static) | SIL OFL 1.1 |
| `JetBrainsMono.ttf` | JetBrains Mono | code — variable `wght` | SIL OFL 1.1 |
| `NotoEmoji-Regular.ttf` | Noto Emoji | emoji fallback, **monochrome** | SIL OFL 1.1 |

## Noto Emoji — Regular only, of six weights (T28, 2026-08-24)

**[measured]** 21 characters across a 146-document corpus had no glyph in any shipped
face and rendered as **tofu** — an empty box where the author put a tick, a cross or a
face. `✓ ✗ ⚠ → ▸` and box drawing were always fine; everything with *emoji presentation*
was missing. Adding this one face takes the corpus from 21 uncovered to **0**.

Six weights were available and only Regular is vendored. Emoji here are inline symbols in
body text, not typography that needs a weight axis, and Typst falls back to this face from
bold text without complaint. The other five plus the variable font would add ~4.6 MB to
every binary to make a bold ✅ very slightly bolder.

**Monochrome, not colour.** Same register as the rest of the page, and it needs no
colour-font support to verify. `design/plan-glyphs.md` costs the colour alternatives.

## Italic needs its own files — corrected 2026-08-18

This file previously claimed *"Typst 0.15.1 resolves weight and italic from the axes,
so no static instances are needed."* **That was wrong, and it cost us a live bug.**

`SourceSans3.ttf` exposes exactly one axis — `wght` (200–900). There is no `ital` and
no `slnt`. So `#emph` had no italic face to resolve, silently fell back to upright, and
emphasis rendered identically to plain text. Every test passed the whole time, because
they assert on extracted *text* and text carries no styling. It was found by rendering
a page and looking at it.

The **static** italic faces are vendored rather than Adobe's variable italic, because
the variable file reports its family as `SourceSans3VF` while the roman reports
`Source Sans 3`. Typst groups faces by family name, so the mismatched pair would never
have been recognised as one family — the fix would have looked applied and changed
nothing.

Pinned by `emphasis_renders_in_an_italic_face` in `crates/md2pdf-typeset/tests/contract.rs`,
which asserts on the rendered glyph run's font *style*, not its characters.

Weight still comes from the variable axis; only slant needs separate files.

Typst embeds no sans-serif of its own — this is why md2pdf ships a FontBook at all.
Bundling these is what makes PDF output identical across macOS, Windows and Linux.

OFL permits both bundling and PDF embedding. `OFL.txt` must ship alongside the
binary; see `docs/` for the distribution checklist.

Chosen by rendering an identical specimen through four candidates — see
`design/spike-typst-measure-findings.md`. Source Sans 3 was the most compact
(fewest pages) and is designed for print as well as screen.
