# Plan — characters we cannot draw (T28)

**Written:** 2026-08-24 · **Raised by:** T26b, which noticed tofu boxes on a rendered comparison
sheet. **Scoped by:** an exhaustive scan, which found the noticing had undercounted by 10×.

Claims marked **[measured]** or **[assumed]**, per *Planning discipline* in `design/invariants.md`.
Options laid out in full and scored against the goal, per *Cost is not a reason to decline an
option*.

---

## The scope was wrong, and measuring fixed it

T28 was scheduled as *"✅/❌ render as tofu"* — the two characters someone saw as boxes. Asking the
FontBook about **every** non-ASCII character in the corpus instead:

```
81 distinct non-ASCII characters, 21 with no glyph

  ✅ U+2705  28 docs      🔴 U+1F534   7 docs      😊 U+1F60A  2 docs
  ❌ U+274C   8 docs      📋 U+1F4CB   2 docs      😐 U+1F610  2 docs
  ❓ U+2753   3 docs      🔄 U+1F504   2 docs      😤 U+1F624  2 docs
  ❗ U+2757   2 docs      🚪 U+1F6AA   2 docs      🤔 U+1F914  2 docs
  ️ U+FE0F  21 docs      🔍 🔵 🔶 😌 😒 😟 😮 😰  1 doc each
```

**Ten times the problem that was scheduled**, and the difference is entirely that one was noticed
and the other was measured. Everything else the corpus uses is covered — `✓ ✗ ⚠ → ▸`, box drawing
(46 documents, inside code fences), and the ballot boxes `convert` emits for task lists.

**This kills substitution as *the* answer.** `✅ → ✓` is a good trade. There is no plain-text
equivalent of 😊, and inventing one would be worse than admitting we cannot draw it.

## What shipped

**1 · The font, which makes all 21 draw.** See below — it landed the same day and deleted the
substitution that had been written first.

**2 · Report anything still uncovered.** The engine asks the Typesetter which characters have no glyph and
raises a Compromise for each. **This is the part that generalises**: it needs no table, no font, and
covers a character nobody has seen yet. `INV-4`'s promise is that an empty Diagnostic means the
document converted cleanly — 19 characters rendering as boxes while the Diagnostic stayed empty was
a straightforward breach of it.

**3 · A gate on coverage.** `every_character_the_corpus_uses_has_a_glyph` names every character the
corpus contains, the working ones included — box drawing inside code fences appears in 46 documents
and losing it would be worse than the tofu was. A FontBook change that drops any of them goes red.

## The bundled font — **added 2026-08-24, and it retired the substitution**

The operator supplied Noto Emoji. `NotoEmoji-Regular.ttf` is vendored, and **[measured]** the corpus
goes from **21 uncovered characters to 0**. Every emoji now draws its own glyph.

**The substitution is deleted, not kept as a fallback.** `the_substituted_characters_are_still_the_
ones_that_need_it` went red the moment the font landed — which is exactly what it was written to do.
Rewriting an author's `✅` into `✓` was defensible only while we could not draw the original; it is
not defensible now, and leaving it would have been a silent rewrite nobody needed.

**The reporting stays.** It is the general answer: it needs no table and no font, and it covers the
next character this font also lacks.

**Regular only, of the six weights supplied.** Emoji here are inline symbols, not typography needing
a weight axis, and Typst falls back from bold without complaint — the other five plus the variable
font would add ~4.6 MB per binary for a slightly bolder ✅.

### The original analysis, kept

**This was the real fix, and it was blocked rather than declined.** **[measured]** there is no
emoji font anywhere on this machine, and `INV-1` means no build-time fetching — the FontBook is
vendored precisely so builds are reproducible and offline. The file has to be added deliberately.

| Option | Covers | Size | Licence | Notes |
|---|---|---|---|---|
| **Noto Emoji** (monochrome) | all 21, and essentially any future emoji | ~1.1 MB, **subsettable to a few KB** | OFL 1.1, same as the current four | Black-and-white glyphs — matches a print target, and matches the rest of the FontBook |
| Noto Color Emoji | same | ~10 MB (CBDT) / ~1.5 MB (COLRv1) | OFL 1.1 | **[assumed]** colour font support in Typst 0.15 is unverified — cheap to test once a file exists, and a hard blocker if absent |
| Twemoji | same | ~2 MB as a COLR build | **CC-BY 4.0**, not OFL | Attribution obligation the other four do not carry; the graphics are Twitter's |

**Recommendation: Noto Emoji, monochrome, subset to the characters the corpus uses.** It is the same
licence as everything already vendored, it is the smallest, it needs no colour-font support to be
verified, and monochrome is the right register for a print stylesheet — the rest of the page is
black on white.

**What it changes.** Substitution becomes unnecessary and should be retired (the test in step 3 will
say so). Reporting stays, because it is the general answer for the next character.

**What it costs beyond bytes.** A fifth face in the FontBook, a README row, and a licence file. The
`SourceSans3-It` episode is the precedent for taking font changes seriously: a wrong assumption
about what a font file contained cost a live bug that no test could see.

## Exit criteria

1. ✅ Coverage measured exhaustively rather than by eye — 21, not 2.
2. ✅ Every uncovered character reported, so `INV-4` holds — the part that survives any font.
3. ✅ Rendered and looked at: ✅ ❌ 🤔 🔴 all draw their own glyph; the test document is now clean.
4. ✅ A coverage gate naming every character the corpus uses.
6. ✅ `verify.sh` green.
7. ✅ **Noto Emoji bundled** — 21 uncovered → **0**, and the substitution retired itself.

## Doubts — audited

### D1 · Was rewriting the author's characters acceptable? — **moot, and it did not survive the day**

Substitution changed what the document said, which is the thing a converter should be most careful
about. It was defensible only while the original could not be drawn, and the test written to detect
that condition ending fired within hours. **The lesson worth keeping is that the retirement was
designed in before it was needed** — a fallback with no exit condition becomes permanent.

### D2 · Does U+FE0F matter? — **[assumed] no, but it is in the count**

Variation Selector-16 appears in 21 documents and has no glyph, but it is a zero-width formatting
character that renderers are not expected to draw. It is listed above for honesty about what the
scan found; **[assumed]** it is not producing a visible box. Not verified, because it always
accompanies an emoji that *is* producing one.

### D3 · Should this be its own CompromiseKind? — **[assumed] no, not yet**

`UnsupportedConstruct { construct }` already means "we could not render this faithfully" and carries
a description. A dedicated kind would touch the domain, the census labels and the CLI for a
distinction nobody has asked to filter on. Revisit if the attention list gets crowded.
