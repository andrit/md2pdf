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

## What was built

**1 · Substitute where there is an honest equivalent.** `md2pdf-convert::glyphs` maps `✅ → ✓` and
`❌ → ✗` — the same marks without emoji presentation. That is 36 of the 146 documents, the two
commonest by a wide margin. Colour is lost, so it is **recorded as a Compromise**, not done quietly.

**2 · Report everything else.** The engine asks the Typesetter which characters have no glyph and
raises a Compromise for each. **This is the part that generalises**: it needs no table, no font, and
covers a character nobody has seen yet. `INV-4`'s promise is that an empty Diagnostic means the
document converted cleanly — 19 characters rendering as boxes while the Diagnostic stayed empty was
a straightforward breach of it.

**3 · Tests that cannot drift.** `convert` holds the editorial decision (*what does ✅ mean?*) and
has no fonts by design; `typeset` holds the typographic fact (*can anything draw it?*). The seam is
checked from `typeset`'s contract tests: every substitution target must be covered, and the
substituted originals must still be the ones that need it — so a FontBook that gained emoji turns a
test red and the rewrite gets retired rather than lingering.

## What was **not** built: a bundled fallback font

**This is the real fix, and it is blocked here rather than declined.** **[measured]** there is no
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
2. ✅ The two commonest substituted, and the substitution **reported** rather than silent.
3. ✅ Every remaining uncovered character reported, so `INV-4` holds.
4. ✅ Rendered and looked at: `✓` and `✗` draw correctly, 🤔 and 🔴 still box **and now say so**.
5. ✅ Anti-drift tests across the `convert`/`typeset` seam.
6. ✅ `verify.sh` green.
7. ☐ **Bundle Noto Emoji** — needs the file. Operator's call; the analysis is above.

## Doubts — audited

### D1 · Is rewriting the author's characters acceptable? — **only because it is recorded**

Substitution changes what the document says, which is the one thing a converter should be most
careful about. It is defensible here because the alternative is a box that says nothing at all, and
because the Diagnostic names it. **[assumed]** it stops being defensible the moment a bundled font
makes it unnecessary, which is why the test that would retire it was written now.

### D2 · Does U+FE0F matter? — **[assumed] no, but it is in the count**

Variation Selector-16 appears in 21 documents and has no glyph, but it is a zero-width formatting
character that renderers are not expected to draw. It is listed above for honesty about what the
scan found; **[assumed]** it is not producing a visible box. Not verified, because it always
accompanies an emoji that *is* producing one.

### D3 · Should this be its own CompromiseKind? — **[assumed] no, not yet**

`UnsupportedConstruct { construct }` already means "we could not render this faithfully" and carries
a description. A dedicated kind would touch the domain, the census labels and the CLI for a
distinction nobody has asked to filter on. Revisit if the attention list gets crowded.
