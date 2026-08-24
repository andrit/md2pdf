# Plan — stop estimating text width (T31a)

**Written:** 2026-08-23 · **Raised by:** the operator — *"do the typeset move before T26c"*, after
T30 closed the fourth defect of one shape.
**Closes:** flag **F8**, and the defect class behind T29 / T29b / T29c / T30. **Blocks:** nothing;
T26c follows.

Claims marked **[measured]** or **[assumed]**, per *Planning discipline* in `design/invariants.md`.
Options are laid out in full and scored against the goal, per *Cost is not a reason to decline an
option* — the rule this plan exists because of.

---

## The goal, stated first

Everything below is scored against this, not against effort:

> **A reflowed table never runs off the page, and no text is broken that did not have to be.**

Two halves, and the four previous attempts each traded one for the other. Being generous with break
opportunities overflows nothing but chops ordinary words; being stingy keeps words whole and
overflows. Every fix so far has moved the dial rather than removing it.

## Why there is a dial at all

`md2pdf-convert` decides where a long token may wrap. To decide well it must know how wide the text
will be — and it cannot, because Typst does. So it estimates, and **[measured]** four consecutive
tasks have each added a missing term to that estimate:

| | The term the estimate was missing | Found by |
|---|---|---|
| T29 | a run with no break opportunity cannot wrap at all | rendering, counting pixels |
| T29b | an `auto` column refuses to shrink | rendering, counting pixels |
| T29c | a lopsided `fr` spec gives columns unequal room | rendering, counting pixels |
| T30 | the base size, then the 5pt-per-side cell inset | rendering, counting pixels |

**Not one was found by a test.** The estimate is a model of Typst's layout maintained by hand, and
the next omission will be found the same way as the last four.

---

## The finding that reframes this plan — measured 2026-08-23

**[measured]** a spike, ~5 minutes: offer a break opportunity after **every character** of a token
already judged over-long, instead of counting to a limit. Nothing else changed.

```
before:  423 tables can reflow, 152 do, 1 of those overflows   (F8, 2pt)
spike:   423 tables can reflow, 152 do, 0 of those overflow
```

**The residual overflow that four tasks could not close, closes.** And it closes because the spike
stops predicting where the line will end and lets Typst — which knows — choose.

The reason this is safe is worth stating precisely, because it is what makes the cheap option
credible rather than reckless:

1. **`U+200B` is an *opportunity*, not a break.** Typst uses one only when the line is full, so extra
   opportunities inside a token that fits change nothing.
2. **The set of affected tokens does not grow.** Only tokens longer than the limit are touched, exactly
   as today. What changes is the granularity *within* those tokens — and those are tokens that must
   break somewhere regardless.
3. **It applies only to the reflow alternate**, never the body — `breakable_cells` is the sole caller.
   Prose, headings and code are untouched, and the body is what the probe measures to pick a rung.

So the estimate does not need to be *right*; it needs to be **generous**. Its remaining job is "is
this token at risk", where erring long is free, rather than "where exactly will the line end", where
erring long overflows the page.

---

## The options

### Option 1 · Break anywhere inside an over-long token — *delete the estimate's hard job*

Replace the counting fallback in `break_word` with an opportunity after every character. Separator
preference (F9) is kept, so `user_organization_roles` still prefers its underscores; the difference
is only what happens between separators.

- **What it changes:** ~10 lines in `emit.rs`. `break_limits` survives as a risk filter and can be
  made deliberately generous; the inset term stays because it still keeps the filter honest.
- **What it makes impossible:** an overflow caused by a mis-estimated break position — the entire
  T29/T29b/T29c/T30 family. Typst places the break, so the estimate cannot be wrong about it.
- **What it does not fix:** a *column* too narrow for its content is still a column-sizing question,
  and `column_weights` still estimates that. Overflow is closed; ugly-but-contained stays possible.
- **Cost:** hours. All seven goldens move. **[measured]** corpus overflow 1 → 0.
- **Risk:** more mid-token breaks inside already-over-long tokens in reflowed table cells. **[assumed]**
  acceptable — those tokens break somewhere in every design; must be confirmed by eye before merging.

### Option 2 · Measure in the probe, feed the widths back to convert — *the literal typeset move*

`md2pdf-typeset`'s ProbePass already runs Typst and can call `measure()`. Add a pass that measures
each table's cells, returns real per-column widths, and have `convert` re-emit the alternate against
measured widths instead of estimated ones.

- **What it changes:** a new probe stage, a new type crossing the seam (measured widths per cell), a
  re-emit entry point on `convert`, and orchestration in `md2pdf-engine` to run convert → probe →
  re-convert. Three crates.
- **What it makes impossible:** the same family as Option 1, **plus** column-sizing error —
  `column_weights`'s content-length heuristic is replaced by measurement, so lopsided and
  many-column tables are sized correctly rather than approximately.
- **Cost:** ~2–3 days of work. The runtime cost is **[measured]**, not assumed — an upper bound of
  **+1.6s on a 29.7s batch, ~5%**; see below. Interacts with 3f's recompile loop and with F3.
- **Risk:** the convert → probe → convert loop puts a cycle in a pipeline that is currently one-way,
  which is a real architectural cost. INV-9 and the crate boundaries need re-reading before committing.

### Option 3 · Move break insertion into `md2pdf-typeset` entirely — *what plan-base-size declined*

The alternate stops being markup produced by `convert` and becomes something `typeset` assembles,
so the layer that measures is also the layer that decides.

- **What it changes:** the seam itself. `Element.reflow` stops being opaque `Markup` and becomes
  structured cell data; `typeset` gains markup assembly, which today lives entirely in `convert`
  alongside escaping.
- **What it makes impossible:** the whole class, permanently — there is no estimate left anywhere.
- **Cost:** ~1 week. Touches the domain type every crate depends on, and moves escaping-adjacent
  code across the boundary that `check-boundaries.sh` exists to police.
- **Risk:** highest. **[assumed]** it also weakens the reason `convert` is testable without Typst,
  which is the property that makes 76 of its unit tests fast and mock-free.

---

## What Option 2 actually costs and buys — measured 2026-08-23

Both numbers were `[assumed]` when this plan was written. The operator asked for them, and they
change the shape of the option in both directions.

**What it costs.** Measuring every cell of every reflow-capable table, release build:

```
375 reflowed tables, 1081 columns, 1636ms of probing
```

Against the 29.7s corpus batch in `compromise-mechanism.md` §6 that is **+5.5%, and it is an upper
bound**: the harness builds a fresh `Typesetter` per table, which a real implementation would not,
and Option 2 needs only the 152 tables actually on the reflow rung rather than all 375. **The
"noticeable batch slowdown" I warned about is not there.**

**What it buys.** How far the estimated column share sits from the measured one, in percentage
points of the table's width:

```
p50  3.6      p90  9.1      p99  17.4      max  31.4
columns off by more than 10 points: 77 of 1081 (7%)
```

**Both halves matter.** The estimator is *usually close* — half of all columns land within 3.6
points, which is why reflowed tables have looked acceptable all along. But the tail is real: 7% of
columns are off by more than a tenth of the table's width, and the worst is 31 points — a column
that should take a third of the table taking almost none of it. **[assumed]** the tail is what a
reader notices, and it is what Option 2 removes.

So Option 2 is **cheaper than I priced it, and buys less than "correct column sizing" implies**. It
is not the difference between broken and working; it is the difference between usually-right and
right. Both of those corrections came from measuring, and both were mine to have measured first.

Reproduce:

```bash
cargo test --release -p md2pdf-engine --test walking_skeleton \
    how_wrong_is_the_column_estimate -- --ignored --nocapture
```

### A fourth possibility, found while measuring — **[measured], not settled**

Typst can size the columns *itself*, inside the document, with `#context` and `measure()`. It
compiles and renders. **But the probe then reports the element's natural width as `0.0`** —
`#context` defers to layout, so there is nothing to measure at probe time, and the ladder picks its
rung from exactly that number.

That disqualifies it for an Element *body*. It may still hold for the **alternate**, which is
reached only after the body has failed and is never measured itself. If it holds, it removes the
estimate with **no cycle, no extra probe pass, and no crate boundary crossed** — strictly better
than Option 2 on every axis costed here. If it does not, it is a dead end.

**Not pursued further, deliberately.** The oracle would settle it in about an hour. Recorded rather
than chased, because the plan already offers three options and none has been chosen — and starting a
fourth unasked is how the last four tasks happened.

## Option 4, spiked 2026-08-23 — **it works, and it is not the win**

Built it: the alternate emits `#context` and computes its own column widths from
`measure()`. Results, in the order they arrived.

**The mechanism is sound.** Census unchanged — the ladder still picks every rung it picked,
confirming the alternate is never measured for rung choice, which was the open question. Corpus
overflow **1 → 0**. And it is **free**: min-of-5 on a 56-document subset holding 54 reflowed tables,
release build, `base 6636ms · opt4 6744ms` — inside the noise.

**Then I looked at the page, and it was unreadable.** `measure()` returns a cell's *unbroken*
natural width, so a prose cell measures as one enormous line and raw proportionality handed it ~95%
of the table. The three numeric columns collapsed to nothing and **overprinted each other** —
`Phase 3C`, `79`, `145`, `14` printed on top of one another.

> **The oracle reported zero overflows for that page.** Overprinting *inside* a table puts no ink
> past the right margin, so the instrument cannot see it. Recorded as **F11**. This weakens
> "corpus overflow is 0" as evidence for *every* option here, including Option 1 — it is necessary,
> not sufficient, and only looking closes the gap.

**Applying the existing policy to measured widths gives output identical to today's.** The reason is
arithmetic: `column_spec` quantises every weight into six buckets via `WIDEST_WEIGHT`, so a column
must be wrong by roughly a sixth — ~17 points — before the emitted spec moves at all. The measured
error is **3.6 points at the median**. Corpus-wide, **[measured]** the spec changes for **148 of 375
tables (39%)**, so measuring is not a no-op — but on the table with the single worst error in the
corpus (31.4 points, `project-state.md`) it changed nothing visible.

### What the page says the real defect is

The rendered table is legitimately bad, and **not for any reason this plan addresses**. Column 1
holds `Microservices + Docs` and `Capability Registry API` and is broken mid-word — `Micros/ervices`,
`Capabi/lity`, `Regist/ry` — while three columns holding `79`, `145` and `14` sit at the same width
it has. That is `column_weights`' floor: `.max(1)` gives a numeric column the same minimum share as a
text column that needs far more, and quantisation cannot tell them apart.

**So the lever is the weighting *policy*, not the width *source*.** Options 2, 3 and 4 all replace
the source. None of them touches the floor that is actually disfiguring this page.

## Scored against the goal

| | Never overflows | Breaks nothing needless | Column sizing correct | Cost | Reversible |
|---|---|---|---|---|---|
| **1 · break anywhere** | **yes** [measured] | within over-long tokens only | no — still estimated | hours | easily |
| **2 · measure, feed back** | yes [assumed] | yes | **yes** — worth 3.6pp median [measured] | 2–3 days · +5% batch [measured] | moderately |
| **3 · move the seam** | yes [assumed] | yes | **yes** — same 3.6pp median | ~1 week | hard |

**Recommendation, revised after the spike: Option 1, then a policy task — and 2/3/4 are not next.**
Option 1 closes overflow for hours of work. The spike shows 2, 3 and 4 all buy the same thing (a
better width source) and that its visible effect is small, because quantisation absorbs most of it.
The page says the money is in `column_weights`' floor and bucket count. That is a *new*, cheap task
against a defect I can now point at, and it should be argued on its own before any seam moves.

**Superseded reasoning, kept:** Not because it is cheapest — that is the
reasoning this plan exists to correct — but because it is the only one whose effect on the goal is
**already measured**, and because it changes what the remaining question is. With overflow at zero,
Options 2 and 3 are no longer buying *correctness*; they are buying *column-sizing quality*, which is
a different and much weaker case that should be argued on its own evidence.

**The operator's call, and the honest statement of it:** you asked for the typeset move, and Option 1
is not it. If the goal is "stop overflowing", Option 1 measurably achieves it today. If the goal is
"remove the estimate from the architecture", only Option 3 does, and it is a week. Option 2 is the
middle and carries an architectural cost — a cycle in a one-way pipeline — that I would want to argue
about before paying.

## Exit criteria

Whichever option is chosen:

1. Corpus overflow is **0**, by the oracle; F8 closed rather than re-tabulated.
2. A reflowed table rendered and **looked at** — the project's own rule; the count is not the quality.
3. Census regenerated; any rung movement explained before it is committed.
4. Goldens regenerated deliberately; `verify.sh` green.
5. If Option 2 or 3: batch time re-measured against the 29.7s baseline in `compromise-mechanism.md` §6.

## Doubts — audited

### D1 · Does "break anywhere" read badly? — **must be looked at, not counted**

**[measured]** the affected set does not grow; only granularity within it. **[assumed]** that makes it
acceptable, and that assumption is exactly what a rendered page settles. Exit criterion 2.

### D2 · Does the estimate really survive as a risk filter? — **yes, and it should be loosened**

Option 1 leaves `break_limits` deciding which tokens are at risk. **[assumed]** it should then be made
deliberately generous — erring long is now free, where it used to overflow. Not done in the same
change: one behaviour change at a time, or the measurement is uninterpretable.

### D3 · Is F8 really closed, or moved? — **measured closed, on this corpus**

**[measured]** 0 of 152 on 146 real documents. **[assumed]** a document outside the corpus could still
overflow via column sizing rather than break position, which Option 1 does not address — which is
precisely why F8 should close and a *new*, narrower flag open if it recurs.
