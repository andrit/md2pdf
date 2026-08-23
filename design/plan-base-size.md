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

   > **Superseded 2026-08-23, on building it. The prediction was wrong: neither size moved.**
   >
   > **[measured]** the ladder's chosen size is *absolute*. A table fits when its natural width at
   > that size fits the page, which is a fact about the content and the page — not about the base
   > it started from. `shrink-slight.md` resolves to 9.0pt at a 10pt base and at a 12pt base alike;
   > `below-comfort-reflows.md` resolves to 7.5pt at both — *derived* from its measured 766.5pt natural
   > width, by the arithmetic the 9.0pt case validates (the "7.0pt" above was already wrong).
   > The census confirms it: **no rung moved on any fixture.**
   >
   > What moved is what the names *mean*. 12 → 9 is a quarter off where 10 → 9 was a tenth, so
   > `shrink-slight.md` is renamed **`shrink-to-comfort-floor.md`** — which is the role it actually
   > plays, pinning the `>=` boundary. Prose in both fixtures now records the size *and* that it is
   > base-invariant, so the next base change does not re-open this.

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

### D1 · Does 12pt push more tables off the page? — **yes, once — and it found a real defect**

Larger text in the same width means every table needs more room, and reflow is already the rung
carrying 152 of 198 compromises. **[measured]** overflow was 1 of 152 before.

> **Answered 2026-08-23 by building it.** The corpus count is **unchanged at 1 of 152** (the same
> 2pt overhang, **F8**). But `reflow-hostile.md` — a fixture, green for eight commits — spilled 4pt,
> and the cause was not the base size.
>
> **`break_limits` never charged a table cell for its inset.** It shared `chars_across` — the
> characters that fit the *full text width* — among the columns, while each cell also pays 5pt of
> padding on each side. A twelve-column table spends 120 of its 483 points on inset, so the limit
> was a quarter too generous; at 10pt there was slack to absorb that, and at 12pt there was not.
>
> Fixed by giving each column its own share **and then** deducting its own inset — deducting first
> spreads the cost in proportion to weight and under-charges the narrow columns, which is the F8
> shape exactly. Pinned by `a_column_is_charged_its_own_inset_not_a_share_of_the_tables`.
>
> **This is the fourth defect in this family** (T29, T29b, T29c, and now T30), and all four are one
> shape: `convert` estimating layout arithmetic that `typeset` could measure exactly. See D5.

### D2 · Does the comfort floor still mean the same thing? — **no, and T26c must re-decide**

At a 10pt base, shrinking to 9pt is a 10% reduction. At 12pt it is a 25% reduction — a visibly
bigger step for the same absolute size. **[assumed]** the floor is better understood in absolute
terms (is 9pt text comfortable?) than relative, but that assumption is exactly what T26c tests, and
it must be tested against the new base.

### D3 · Should the page or margins change too? — **not now**

**[assumed]** out of scope. Raising the base is one change with one measurable effect; changing the
page geometry at the same time would make the baseline movement uninterpretable. 3e owns the
template.

### D5 · Why does this family of defect keep recurring? — **the estimator, and it is now the operator's call**

**[measured]** four consecutive tasks have fixed the same shape of bug, each found only by rendering
a page and counting pixels, never by a test:

| | What the estimate was missing |
|---|---|
| T29 | that a run with no break opportunity cannot wrap at all |
| T29b | that an `auto` column refuses to shrink |
| T29c | that a lopsided spec gives columns unequal room |
| T30 | the base size, and then the cell inset |

`convert` decides where text may break, which requires knowing how wide text will be — and it does
not know, because Typst does. Each fix adds a term to the estimate; the next omission is found the
same way. **[assumed]** the class closes only by moving break insertion into `md2pdf-typeset`, which
`The change` §2 listed as *"most correct, largest change"* and declined on cost. That deferral is
what the last four tasks have been paying for.

**Raised with the operator 2026-08-23**, who scoped it: *"we don't need to worry about a 12 column
table. this is a md file, not an excel file."*

**Decided:** the widest table in the corpus has **7 columns** [measured, R3]. `reflow-hostile.md`'s
twelve are synthetic and stay only as a stress case — **they do not justify further tuning**, and a
future overflow that only a 12-column table can produce is to be flagged, not engineered away. Note
that today's inset fix is *not* a wide-table special case: it corrected every reflowed table's
limits, and the narrow column of a two-column `(1fr, 6fr)` spec moved 10 → 9 characters with it.

### D4 · Is 12pt right, or is it the first honest guess? — **the operator's call, and recorded as such**

**[measured]** it matches GitHub's 16px and lands inside the comfort band, which is more support than
10pt ever had. But *optimal* is a judgment about reading, and the same evidence would tolerate 11.5
or 12.5. Recorded as a decision made deliberately, so the next person finds a reason rather than an
accident.
