# Plan — let Typst break the lines; stop starving the columns (T31a)

**Written:** 2026-08-23 · **Rewritten:** 2026-08-24, after spiking three of the four options and
looking at the rendered page.
**Raised by:** the operator — *"do the typeset move before T26c"*, then, on seeing the output,
*"we should only break on a full word"* and *"let typst do what it does."*
**Closes:** flag **F8**, and the defect class behind T29 / T29b / T29c / T30.
**Blocks:** nothing; T26c follows.

Claims marked **[measured]** or **[assumed]**, per *Planning discipline* in `design/invariants.md`.
Options are laid out in full and scored against the goal, per *Cost is not a reason to decline an
option*.

> **This plan was numbered 1–4 in its first draft, and the numbering is gone.** Options 2 and 3 were
> measured out; Option 1 was contradicted by the operator's direction; Option 4's mechanism survives
> with a different allocation rule. Keeping the labels invited *"so it's 1 and 4?"* when the answer
> is one task. The superseded comparison is at the end, because the reasoning was acted on.

---

## The goal

> **A reflowed table never runs off the page, and no word is broken that a reader would not break.**

The second half is the correction. The original goal said *"no text is broken that did not have to
be"*, which quietly permitted breaking `Executio|n` on the grounds that the column was narrow —
begging the question, since the column being that narrow is itself the defect.

## What is actually wrong — measured 2026-08-23/24

**1 · We shred ordinary words.** Across the corpus, of the tokens that receive break opportunities:

```
579  ordinary words        Executio|n   Conformi|st   Analytic|s   Integrat|ion
712  identifiers, paths, hashes
```

**45% of every break we insert lands inside a normal English word.**

**2 · It is not Typst doing it.** Typst is a typesetting engine: it tracks the line, measures the
next word, and wraps at the space when it will not fit. That is why body prose wraps cleanly and is
never shredded. We break these words *before Typst ever sees them* — `offer_breaks` inserts a
zero-width space into any word longer than a per-column character limit, and in a narrow column that
limit is 5–9 characters, so `Execution` (9) qualifies.

**3 · The columns are starved below their own longest word.** Rendered `project-state.md` and
looked: column 1 holds `Microservices + Docs` and `Capability Registry API` at the same width as
three columns holding `79`, `145` and `14`. `column_weights` gives every column a `.max(1)` floor
and quantises into six buckets, so a numeric column and a text column are indistinguishable to it.

**These are one defect seen twice.** A column narrower than its longest word must either break the
word or overflow. We chose to break, and then blamed the breaking.

## The change

**Let Typst break the lines. Make sure the columns are wide enough for it to.**

### 1 · Never make a column narrower than its longest word

The standard table algorithm, and what browsers call **min-content**: every column is allocated at
least the width of its widest unbreakable token; whatever remains is distributed in proportion to
demand. Then Typst breaks at spaces — which *is* "only break on a full word" — and `Execution` is
never touched.

Computed in-document with `#context` and `measure()`, so **no width is estimated and none crosses a
crate boundary**. **[measured]** the mechanism works: it renders, the census is unchanged (the
alternate is never measured for rung choice), and it costs nothing — `base 6636ms · opt4 6744ms`,
min-of-5 over a 56-document subset holding 54 reflowed tables.

**[measured]** the naive form of this failed, and the failure is why min-content is specified rather
than "measure the cells": sizing columns in proportion to *max-content* gave a prose cell ~95% of the
table, collapsed the numeric columns, and **overprinted them**. Max-content is a cell's width with no
wrapping at all, which for prose is meaningless.

### 2 · Break only what has nowhere else to break

`offer_breaks`'s threshold stops being "longer than this column's share" and becomes "cannot fit any
column this table could give it". Separator preference (F9) is unchanged and already handles paths
well. The 712 identifiers and hashes keep their opportunities; the 579 words lose them.

### 3 · The fallback, stated so it is not discovered later

When the min-contents sum past the page — a genuinely unfittable table — something must give. Fall
back to today's behaviour **for the offending columns only**: break their tokens. **[assumed]** rare;
the corpus has no table wider than 7 columns.

## Exit criteria

1. **Ordinary words broken: 579 → 0**, by `how_often_do_we_break_an_ordinary_word`.
2. Corpus overflow **0** by the oracle — necessary, not sufficient (**F11**).
3. **`project-state.md` rendered and looked at**, via `look.rs`. F11 means no count settles this.
4. Census regenerated; any rung movement explained before it is committed.
5. Goldens regenerated deliberately; `verify.sh` green.

## Doubts — audited

### D1 · Does min-content make tables too wide to fit? — **must be measured**

A column pinned to its longest word cannot shrink, which is exactly how `auto` broke the fitting
guarantee in T29b. The difference is that min-content is the *longest word*, not the *longest cell* —
much smaller — but "much smaller" is not "small enough". Exit criterion 2 checks it.

### D2 · Is `#context` safe here? — **[measured] yes for the alternate, no for a body**

The probe reports a `#context` element's natural width as `0.0`, because evaluation defers to
layout. That is disqualifying for a body, whose measurement picks the rung. The alternate is reached
only after the body has failed and is never measured — and the census confirms no rung moved.

### D3 · Does this finally close the estimate? — **no, and the honest scope is smaller**

Break *position* moves to Typst and column *width* becomes measured. What remains estimated is the
threshold in change 2 — "could any column hold this token" — where erring generous is free.

---

## Superseded: the four-option comparison, 2026-08-23

Kept because it was acted on, and because two of the four were removed by measurement rather than by
argument.

| | Verdict |
|---|---|
| **1 · break anywhere inside an over-long token** | **Contradicted.** Measured 1 → 0 overflows for hours of work, and I recommended it. But `Execution` is already "over-long" in a narrow column, so it would break ordinary words at *every character* — the opposite of the direction. It scored well only because it was scored against overflow, which **F11** then showed cannot see the damage. |
| **2 · measure in the probe, feed widths back to convert** | **Measured out.** Costs +5% on the batch (not the slowdown assumed) but buys less than it appears: the estimate sits 3.6 points from measured at the median, and `WIDEST_WEIGHT` quantises into six buckets, so ~17 points of error are needed before the spec moves. Changes 148 of 375 specs; changed nothing visible on the corpus's worst case. Also puts a cycle in a one-way pipeline. |
| **3 · move the seam into `md2pdf-typeset`** | **Measured out with 2.** Same width-source benefit, a week of work, and it weakens the property that makes `convert`'s 78 unit tests fast and mock-free. |
| **4 · Typst sizes its own columns** | **Mechanism kept, rule replaced.** Proportional-to-max-content overprinted the page. Min-content is the correction. |

**The lesson worth keeping.** Every option replaced the width *source*. The page said the defect was
the allocation *policy* — a floor that cannot tell `79` from `Microservices`. Four options, none of
which addressed it, because all four were designed from the estimate's history rather than from the
rendered output. The picture cost an hour and moved the plan further than three spikes did.
