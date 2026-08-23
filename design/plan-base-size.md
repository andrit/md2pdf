# Plan — the base size is 12pt (T30)

**Written:** 2026-08-23 · **Raised by:** the operator — *"12pt is optimal; sometimes it must be
smaller to fit; we should not lose sight of truly optimal."*
**Closes:** flag **F10**. **Blocks:** T26c.

Claims marked **[measured]** or **[assumed]**, per *Planning discipline* in `design/invariants.md`.

---

## The finding this rests on

**`base_size_pt: 10.0` was never chosen.** **[measured]** it has no doc comment, and nothing in the
repository records a reason for it. It appears once, in the font spike, as a specimen setting —
*"10pt body"* — and was carried forward as though it were a decision.

Two independent lines of evidence say it is wrong, which matters more than one line twice.

### 1 · The fidelity target and the base disagree

The shipped template is named **`github-print`**, and the product's stated goal is GitHub fidelity.
GitHub's body text is **16px ≈ 12pt**. The one dimension a reader notices before anything else is
the one where we depart from the thing we are imitating, silently.

### 2 · 10pt produces lines that are too long **[measured]**

`Template::default()` gives 483pt of text width. At the measured average character width:

| Base | Characters per line |
|---|---|
| **10pt** | **96** |
| 11pt | 87 |
| **12pt** | **80** |
| 13pt | 74 |

The typographic comfort band is roughly **45–75 characters**. At 10pt the body is far outside it; at
12pt it is at the top edge. So the current setting is not merely small — it also produces a measure
that is tiring to read, and enlarging the type fixes both at once.

**[assumed]** this suggests the margins deserve attention too: at 12pt with wider margins the line
would land nearer 70, comfortably inside the band. That is template design and belongs to 3e, not
here — noted so it is not lost.

### 3 · Every Compromise so far was measured against a compromised baseline

This is the serious one. The mechanism reports departures from `base_size_pt` and treats sitting *at*
it as clean. If the optimal body size is 12pt, then **146 documents "converted cleanly" at a size
nobody chose**, and the shortfall was never recorded as a concession at all.

`R1`'s 48%-flagged figure was therefore flattering. The honest number is expected to be worse, and
that is the point of changing it.

## What the corpus says it costs — measured 2026-08-23

423 tables, probed at both bases:

| | 10pt (today) | 12pt |
|---|---|---|
| fits as authored | 225 | **149** |
| shrunk | 46 | **122** |
| reflowed | 152 | **152** |

Shrink targets at 12pt: `9.0pt × 27 · 9.5pt × 19 · 10.0pt × 20 · 10.5pt × 21 · 11.0pt × 17 ·
11.5pt × 18`.

**76 more tables are compromised, and not one of them renders smaller than it does today.** More
than half the new shrinks land at **10.0–11.5pt** — at or above the base we currently ship. A table
that fits at 10pt will, at a 12pt base, shrink to *at least* 10pt, because that is the size at which
it fits. **The output is identical or larger everywhere.**

Reflow is unchanged at 152, which follows from the comfort floor: tables still reach 9.0pt before
wrapping, so the set that cannot fit at 9.0pt is the same set.

### This inverts how R1 should be read

Element-level compromise goes **47% → 65%**, and it is **not a regression**. Every one of those
elements renders at least as large as it does today; what changed is that the shortfall is now
*reported*. The old figure was low because sitting at an unchosen 10pt counted as clean.

**So R1's target must change with it.** "Reduce the flagged percentage" was always the wrong goal if
the baseline was wrong; the goal is to reduce the *distance from optimal*, and the flagged count is
only a proxy for that when the baseline is honest.

## The change

1. **`Template::default().base_size_pt` → 12.0**, with a doc comment recording *why*, so the next
   reader is not left where this one was.
2. **`CHARS_ACROSS` must stop being a constant.** `emit.rs` holds `const CHARS_ACROSS: usize = 96`
   with a ponytail reading *"96 is A4 minus margins at 10pt … upgrade: pass the Template into
   conversion"*. **That ceiling has now been reached** — at 12pt the true figure is 80, and every
   break limit derived from it is 20% too generous.

   Options, and the recommendation is the second:

   | | Cost |
   |---|---|
   | Recompute the constant to 80 | free, and wrong again at the next template |
   | **Give `SourceContext` the `Template`** | **`md2pdf-domain` is already a dependency of convert, so no new coupling; break limits become correct by construction** |
   | Move break insertion into `md2pdf-typeset` | most correct, largest change; the width is known there |

   **[assumed]** the second. `emit` already computes column proportions, which is layout reasoning,
   because that is where the cell content is; giving it the template is an admission of what it is
   already doing rather than a new compromise.

3. **Re-tune the fixture corpus.** **[measured]** several fixtures were sized against 10pt —
   `shrink-slight.md` sits at exactly 9.0pt to pin the `>=` boundary, `below-comfort-reflows.md` at
   7.0pt. Both will move, and their names will lie again if they are not re-measured. The census
   will show it.

4. **Every golden moves.** All seven, because every rendering changes size. That is expected and
   must be regenerated deliberately, in this commit, with the reason recorded.

## Exit criteria

1. Base is 12pt, with the reason in the type.
2. `CHARS_ACROSS` derived from the template rather than assumed.
3. Fixtures re-tuned to the sizes their names claim; census regenerated.
4. `compromise-mechanism.md` §6 re-measured — **including the flagged percentage, whichever way it
   goes**, and R1 updated to say the old figure was measured against the wrong baseline.
5. **Overflow count does not rise** — checked with the oracle. Larger text in the same width is
   exactly the change that could push tables off the page.
6. All goldens regenerated deliberately; `verify.sh` green.

---

## Doubts — audited

### D1 · Does 12pt push more tables off the page? — **must be measured**

Larger text in the same width means every table needs more room, and reflow is already the rung
carrying 152 of 198 compromises. **[measured]** overflow is 1 of 152 today. Exit criterion 5 checks
it rather than hoping.

### D2 · Does the comfort floor still mean the same thing? — **no, and T26c must re-decide**

At a 10pt base, shrinking to 9pt is a 10% reduction. At 12pt it is a 25% reduction — a visibly
bigger step for the same absolute size. **[assumed]** the floor is better understood in absolute
terms (is 9pt text comfortable?) than relative, but that assumption is exactly what T26c tests, and
it must be tested against the new base.

### D3 · Should the page or margins change too? — **not now**

**[assumed]** out of scope. Raising the base is one change with one measurable effect; changing the
page geometry at the same time would make the baseline movement uninterpretable. 3e owns the
template.

### D4 · Is 12pt right, or is it the first honest guess? — **the operator's call, and recorded as such**

**[measured]** it matches GitHub's 16px and lands inside the comfort band, which is more support than
10pt ever had. But *optimal* is a judgment about reading, and the same evidence would tolerate 11.5
or 12.5. Recorded as a decision made deliberately, so the next person finds a reason rather than an
accident.
