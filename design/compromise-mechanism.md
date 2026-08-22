# The Compromise mechanism — how it works, and how it could quietly go wrong

**Written:** 2026-08-21 · **Status:** living. Re-measure when the ladder changes.

A Compromise is md2pdf deciding something on the user's behalf. That is the product's central idea
and its central risk: **every individual decision can be defensible while the whole is wrong**, and
because each one is recorded, the system looks like it is handling things.

This document is the mechanism, the decisions that produced it, the numbers as they stand, and the
ways it could fail without anyone noticing.

---

## 1 · What a Compromise is

> An Element does not fit. md2pdf does something about it rather than failing, and **records what it
> did**.

The recording is the product (`INV-4`). An empty Diagnostic means the document converted cleanly and
that must be *true*, not merely likely — because the attention gate (`INV-5`) fires only where a
Compromise exists, and everything downstream trusts it.

**Recorded is not the same as handled.** A Compromise says a judgment call was made. It says nothing
about whether the result is any good.

## 2 · Where they come from

Two places that never meet, joined by `Diagnostic::seal`:

| Origin | When | Kinds |
|---|---|---|
| **Conversion** | before any compilation exists | `ImageMissing`, `ImageSkipped`, `UnsupportedConstruct` |
| **ProbePass** | measuring against the Template | `ShrunkToFloor`, `Scaled`, `Rotated`, `Reflowed`, `Clipped` |

Until `seal` existed, the conversion half had no route into a Diagnostic at all — INV-4 held inside
the engine and broke on the way out. It was found by a run reporting *"70 need your attention"* that
could explain two.

## 3 · The kinds

| Kind | Cause | What the reader gets | Content lost? |
|---|---|---|---|
| `ShrunkToFloor { size_pt }` | text-bearing Atomic too wide | smaller text, down to 7pt | no |
| `Scaled { factor }` | image too wide | smaller image | no |
| `Rotated` | did not fit portrait | its own landscape page | no |
| `Reflowed` | table too wide even in landscape | wrapping cells, full-size text | no |
| `Clipped` | no alternative left | truncated, with a red marker | **yes** |
| `ImageMissing` | file not on disk | bordered placeholder + alt text | the image |
| `ImageSkipped` | remote URL (`INV-1`) | bordered placeholder + alt text | the image |
| `UnsupportedConstruct` | raw HTML, `data:` URI, unknown format | placeholder or nothing | usually |

Only `Clipped` destroys content that md2pdf *could* have rendered. That is why T26a exists.

## 4 · The ladder

Two independent axes (T14), not four steps:

- **Orientation** — Portrait or Landscape. Rotation exists *only* to change the width a reduction is
  measured against; landscape offers ~1.5× more.
- **Reduction** — None, Shrink, Scale, Reflow, Clip.

Decided entirely in the ProbePass, because the RenderPass never measures:

```
wrappable                          -> Portrait, None
fits portrait                      -> Portrait, None
shrinks, and stays >= comfort      -> Portrait, Shrink | Scale
has an alternate form              -> Portrait, Reflow      (tables — T26b)
otherwise re-measure in landscape:                          (images only, in practice)
    fits at base                   -> Landscape, None
    reduces within floor there     -> Landscape, Shrink | Scale
    otherwise                      -> Clip
```

## 5 · How we got here

| When | Decision | Why |
|---|---|---|
| T13 | `Reduction::Scale` split from `Shrink` | "8pt" and "40%" are different facts; the gate must not reverse-engineer which a number is |
| T13 | `#scale(reflow: true)` | Typst's default scale is visual only — the overflow stayed exactly where it was |
| T13 | `floors.image_scale` added | without a floor, shrink always succeeds and rotation never fires |
| T14 | Orientation and Reduction split into axes | a flat enum cannot express "rotated *and* shrunk to 8pt", and the Element Overrides the design specifies are three independent toggles |
| T14 | landscape **re-measure** | the GLOSSARY required it and the code never did it; a rotated element rendered at natural size and hoped |
| T14 | `Clip` made reachable | it was unreachable code — elements too wide for landscape overflowed silently |
| T26a | `Reflow` added before `Clip` | 25 elements across 9 real documents were losing columns, and every one was a table |

## 6 · Baseline — 146 real documents, 2026-08-22

Release build, `documents/`, `Template::default()` (10pt base, 9/7/7pt floors, **9pt table comfort
floor**, 0.25 image scale). Re-measured after T26b, as §9 requires.

```
146 converted, 146 valid PDFs, 25.7s
 70 flagged (48%)

 152  reflowed          (was 25)
  46  shrunk to floor   (was 163)
   0  rotated           (was 84)
   2  unsupported construct
   0  clipped
```

**Shrink targets** now sit at 9.0pt × 27 and 9.5pt × 19 — every deeper squeeze became a reflow, which
is what the comfort floor is for. **[measured]** this matches the simulation in
`plan-ladder-order.md` exactly, on every count.

**Rotation is zero and that is load-bearing to notice** — see R5. Tables no longer rotate at all, so
the rung survives only for images, none of which reach the ladder in this corpus.

### The previous baseline, 2026-08-21 — before T26b

Kept so the movement is visible rather than asserted.

```
146 converted, 146 valid PDFs, 26.8s
 70 flagged (48%)

 163  shrunk to floor
  84  rotated
  25  reflowed
   2  unsupported construct
   0  clipped          (was 25 before T26a)
```

**Shrink targets** — where text actually lands:

```
 7.0pt  19      at or below 7.5pt: 41 of 163 (25%)
 7.5pt  22
 8.0pt  32
 8.5pt  22
 9.0pt  36
 9.5pt  32
```

**Compromises per flagged document** — a long tail:

```
  1: 30 docs      6: 5      15: 1
  2: 10           7: 1      19: 1
  3:  5          10: 2      22: 1
  4:  9          11: 2      28: 1
  5:  2
```

### Reproduce

```bash
md2pdf documents -o /tmp/out --json > /tmp/run.jsonl     # release build
# then count `diagnostic_sealed` events by kind
```

---

## 7 · Risk register

The mechanism's failure modes are not crashes. They are *plausible-looking* outcomes.

### R1 · Compromise inflation — the gate stops meaning anything · **LIVE**

**48% of documents are flagged.** The design's own illustration is *"47 converted cleanly, 3 need
your attention"* — roughly 6%. At one in two, a user learns to skip the list, and INV-5's promise
that the gate fires *where a decision was made* becomes technically true and practically useless.

**Watch:** the flagged percentage. **It should fall as the ladder improves, not rise.**

### R2 · The order of the rungs · **CLOSED 2026-08-22 by T26b**

> **Rendered and looked at (`design/plan-ladder-order.md`, evidence section).** Deep shrinking beats
> the current reflow on every real table tried. The founding comparison used a *synthetic* table
> whose columns were all the same width — the one shape for which `columns: (1fr, …)` is right.
>
> **Closed.** It took two commits, in this order, because the first attempt got the diagnosis wrong.
>
> The defect was in the **alternate**, not the order: equal `1fr` shares gave a "P1" column the same
> width as a prose column, which is why reflow *looked* worse than deep shrinking. **T26a2** fixed
> that — narrow columns size to content, wide ones share the slack. Only then did T26b reorder the
> rungs, with a 9pt comfort floor.
>
> **Result, measured:** 163 shrinks → 46, 25 reflows → 152, 84 rotations → 0. The 41 elements at
> 7.0–7.5pt that started this risk are gone.
>
> **The lesson worth keeping** is the sequencing. Reordering first, on this risk's original
> reasoning, would have moved 117 elements onto a rung that rendered them badly — and the census
> would have recorded it as progress.
>
> **The alternate still does not always fit.** With unbreakable content — 261 real table cells hold a
> 30+ character token — it overflows the page *and* the cells overprint each other, while the ladder
> records `Reflowed`. That was true before T26a2 and is unchanged by it. **T29.**
>
> The text below is kept as written, because it is the reasoning that was acted on and the record of
> how a defensible-looking inference went wrong.


The ladder shrinks **before** it reflows. So a table needing 90% of its width is squeezed to ~9pt
rather than being reflowed at full size — and 41 elements are currently rendered at 7.0–7.5pt.

But the comparison rendered while building T26a showed the opposite of that preference:

- `columns: 5` shrunk to 7pt — legible only just, and cramped.
- `columns: (1fr × 5)` at 10pt — full-size text, wrapped cells, clearly easier to read.

**Reflow was placed last because it was the conservative code change**, not because it produces the
worst outcome. On the evidence it may be the *best* one for tables, in which case up to 163 shrinks
and 84 rotations are worse output than necessary.

**This is the concrete instance of "each step reasonable, the whole thing wrong."**

**Open question — needs a decision:** should a table with an alternate form reflow *before* being
shrunk, or before being rotated? Rotation especially: a landscape page for a table that could simply
wrap is a large disruption to the reading order for no gain.

### R3 · Silent degradation — a recorded Compromise still looks handled · **PARTLY ANSWERED 2026-08-22**

> The 12-column case below is hypothetical: **no table in the corpus has more than 7 columns**
> [measured]. And the answer to "nobody has looked at a 7pt table" is now: someone has, and
> **it reads well** — one line per row, columns sized to content (`sheet-2`, panel B).
>
> The degradation is real but it is in the *reflowed* form, not the shrunk one — the opposite
> of what this risk predicted. Column proportion, not column count, and it shows at three
> columns as clearly as at seven. What remains true is the general point: **fitting and being
> readable are different properties, and only one of them is automated.**

`Reflowed` and `ShrunkToFloor` both report success. Neither has been judged for readability at the
extremes:

- A 12-column table reflowed squeezes to ~40pt per column — every cell wrapping one word per line.
  Lossless, and possibly unreadable.
- A table at 7.0pt — 19 elements are there right now. Nobody has looked at one.

**The mechanism cannot detect this**, because "fits" is the only thing it measures. Fitting and being
readable are different properties, and only one of them is automated.

**Watch:** render the extremes and look. Specifically the 7.0pt shrinks and the widest reflow.

### R4 · A Compromise cannot say what it happened *to* · **LIVE**

`Compromise` carries an `ElementId` and a kind — not the element's class, content, or page.
Establishing that all 25 clips were tables required a throwaway probe that re-converted every
document and matched ids by hand.

An adapter cannot do that. So the attention gate in 3f can say *"element 37 was clipped"* and not
*"the table on page 4"*, which is the thing a user can act on. `page` is `Option<u32>` and is always
`None`, because sealing happens before pagination.

### R5 · Reflow removed a symptom; confirm it did not remove the signal · **SHARPER after T26b**

Clipping was the loudest thing the ladder could report, and it is now zero. That is a genuine
improvement — content is no longer destroyed — but it also means **the ladder's alarm no longer
rings**, and a table that is unreadable-but-lossless reports the same way as one that reflowed
beautifully.

**Watch:** if a future change makes output worse, would anything catch it? Golden hashes would catch
*a change*; nothing yet judges *quality*.

**T26b makes this sharper, not milder.** Rotation is now **zero** and clipping is zero, so two of the
ladder's three loud signals are silent, and **152 of 198 compromised elements sit on a single rung**.
Reflow is now where almost everything lands, which means a defect in reflow is a defect nearly
everywhere — and `Reflowed` is reported identically whether the table wrapped beautifully or ran off
the page (**T29**, which happens to 4 elements today).

The fixture census is now the only place `rotated` is observed at all, via the image fixtures.

---

## 8 · What would make this safer

Not scheduled — recorded so the options are on the table when the time comes.

- **A severity on Compromise.** "Shrunk to 9.5pt" and "shrunk to 7pt" are not the same event; the gate
  could fire only on the second. The cheapest fix for R1.
- **Element class and a page number on Compromise.** Closes R4 and is a precondition for 3f.
- **A readability floor distinct from a fitting floor.** The floor currently answers "how small before
  we give up", not "how small before it is unpleasant".
- **Reordering the ladder** so an alternate form is preferred over deep shrinking — R2.

---

## 9 · The rule this document exists to enforce

**Every ladder change re-measures the baseline in §6 and updates it here.**

**Enforced since T27** — it was honour-system until then, which is the thing it warns about.

`design/ladder-census.txt` records what the ladder decides for each fixture in
`crates/md2pdf-engine/tests/corpus/`, and `the_ladder_still_decides_what_it_decided` regenerates it
and fails on any difference. **The census file's git history is the log**: every ladder change
leaves a diff naming which kinds moved, beside the reasoning in `commit-log.md`. There is no
separate log format to maintain.

```bash
# after changing the ladder, take the new reading and commit it with the change:
cargo test -p md2pdf-engine --test walking_skeleton regenerate_the_census -- --ignored --nocapture

# to see sizes and factors, which the census deliberately omits:
cargo test -p md2pdf-engine --test walking_skeleton describe_the_fixtures -- --ignored --nocapture
```

Two baselines, answering different questions. The **fixture census** measures *the mechanism* — one
clean case per rung, committed, reproducible forever. The §6 numbers measure *reality* — the mix of
documents a person actually has, which is what R1 is about, and which cannot be reproduced if
`documents/` is deleted. Both stay; only the first is automated.

**The census records kinds, not sizes.** A shrink moving 8.5pt → 8.0pt is tuning; a shrink becoming
a rotation is a change of behaviour. Only the second should turn the build red.

A change that reduces one kind of Compromise while quietly increasing another, or that lowers the
flagged count by making the gate less honest, is a regression — and the only way to see it is to
keep the numbers side by side.
