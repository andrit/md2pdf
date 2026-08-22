# Plan — the order of the rungs (T26b)

**Written:** 2026-08-21 · **Follows:** T26a, which added `Reflow` and exposed the question.
**Revised:** 2026-08-21, after T27 built the census and the corpus was triaged by element class.
**Reference:** `design/compromise-mechanism.md` — risk **R2**, and the baseline this must re-measure.

Every claim below is marked **[measured]** or **[assumed]**. Assumptions are not forbidden; going
unmarked is. See `design/invariants.md` — *Planning discipline*.

*(T27, the census, was planned here originally and has moved to `design/plan-census.md`. It is
built: `design/ladder-census.txt` + `the_ladder_still_decides_what_it_decided`.)*

---

## The problem, stated plainly

The ladder shrinks **before** it reflows. A table needing 90% of its width is squeezed rather than
reflowed at full size. **[measured]** 41 of 163 shrinks land at 7.0–7.5pt.

The comparison rendered during T26a says that preference is backwards **[measured, by eye]**:

- `columns: 5` shrunk to 7pt — cramped, legible only just.
- `columns: (1fr × 5)` at 10pt — full-size text, wrapped cells, clearly easier to read.

**Reflow went last because it was the conservative code change**, not because it produces the worst
outcome. That is the shape of failure this document family exists to catch: every step defensible,
the sequence wrong.

---

## What the corpus actually contains — measured 2026-08-21

The earlier draft of this plan assumed *"most rotated elements are tables."* Triaged by element
class across all 146 documents, the truth is stronger and simpler **[measured]**:

```
table / portrait   shrink   114          198 compromised elements
table / landscape  shrink    49          every one of them a TABLE
table / landscape  reflow    25          zero images reach the ladder
table / landscape  none      10
```

Cross-checks against the §6 baseline: rotations `10 + 25 + 49 = 84` ✓, shrinks `114 + 49 = 163` ✓,
reflows `25` ✓.

Three consequences, none of which the original plan accounted for:

**1. The ladder is, in practice, a table mechanism.** `Code` wraps and `Image` scales, so the
text-atomic branch only ever sees a `Table` **[measured — `ElementClass::is_atomic` is
`Table | Image`, and `Image` takes the scale branch]**. Every `ShrunkToFloor` in the product comes
from a table.

**2. All 84 rotations are tables, and every one can reflow.** Not "most". Under any option that
prefers an alternate form to rotation, **table rotation goes to zero**, and since no image in the
corpus reaches the ladder, rotation disappears from this corpus entirely.

**3. The 10 `landscape/none` tables are a counter-example to this plan's own argument.** The
earlier draft called rotation *"a large disruption to the reading order for no gain."* For these 10
that is **false** — they rotated and then fit at full size, keeping the author's column proportions
with nothing given up but the page turn. The sentence has been corrected rather than quietly
dropped, and the case it was wrong about is now a decision point below.

---

## The options

### O1 · An alternate form always wins — **deletes a rung**

```
fits -> None
has an alternate -> Reflow
otherwise -> shrink / rotate / clip
```

Simplest rule in the codebase: *"tables wrap, everything else scales."*

**Cost, and it is larger than the earlier draft recognised.** Since every shrink in the product is a
table **[measured, §above]**, tables never shrinking means `Reduction::Shrink` becomes **unreachable
code for the entire product** — `Floors::table_pt`, `code_pt` and `prose_pt` stop having any effect.
The census would say so out loud: `the_census_covers_every_compromise_kind` fails with *"no fixture
produces `shrunk`"*. That is the guard working, and it is also the clearest possible statement of
what O1 costs: a table needing 98% of the width reflows rather than shrinking imperceptibly to
9.8pt, and the authored column proportions are discarded for a problem nobody would have seen.

### O2 · Reflow past a comfort threshold — **recommended**

```
fits                          -> None
shrink, if >= comfort floor   -> Shrink        (small, unnoticeable)
has an alternate              -> Reflow
otherwise                     -> rotate / clip
```

A minor squeeze keeps the authored shape; anything more wraps instead. Introduces a **second floor**
with a different meaning from the existing one:

| Floor | Question it answers |
|---|---|
| `table_pt` (7pt today) | how small before we give up entirely |
| **comfort floor** (~9pt?) | how small before wrapping is *nicer* |

This also **sharpens T26c**. "How small is too small" is nearly unanswerable; "at what point would
you rather the table wrapped" is a question a person can answer by looking at two pages.

**O2 splits on what happens to rotation**, and finding #3 above is what forces the split:

- **O2a — reflow before any rotation.** Table rotation vanishes; rotation becomes an image-only
  rung. Costs the 10 `landscape/none` tables their full-size, correctly-proportioned landscape page.
- **O2b — try landscape at base size first, then reflow.** Keeps those 10 exactly as they are, and
  reflows only where landscape would *also* require shrinking below the comfort floor.

O2b is strictly more conservative and strictly more complex. **[assumed]** the difference matters to
a reader; that assumption is what the evidence below is for.

### O3 · Reflow before rotating, after shrinking

```
fits -> shrink to floor -> Reflow -> rotate -> clip
```

Still shrinks all the way to 7pt first, which is the outcome the evidence already questions. Fixes
the wrong half.

---

## What each comfort floor would do — simulated, not implemented

The probe's `choose()` is a monotonic scan and portrait is decided before anything else, so a
comfort floor `F` changes exactly one thing: a portrait shrink at size `s` stays iff `s >= F`, and
otherwise reflows. That makes the counts computable from the measured distribution without writing
the implementation first **[measured — simulated over the real corpus, O2a]**:

| Comfort floor | shrunk | reflowed | rotated | flagged |
|---|---|---|---|---|
| none *(today)* | 163 | 25 | 84 | 70 (48%) |
| 8.0pt | 86 | 112 | 0 | 70 (48%) |
| **9.0pt** | 46 | 152 | 0 | 70 (48%) |
| 9.5pt | 19 | 179 | 0 | 70 (48%) |

**The flagged count does not move, at any setting.** That is the correct and expected result — the
same elements are compromised, just differently — and it is worth stating plainly because it means
**T26b does nothing for R1**. If a candidate ever *did* drop the flagged count, something is being
hidden rather than fixed.

**Boundary semantics, pinned:** a shrink is kept iff `chosen_size >= comfort_floor`. At `F = 9.0` a
table that shrinks to exactly 9.0pt stays shrunk.

---

## Implementation sketch

Small, and confined to one branch:

- **`Floors` gains one field** — a table-only comfort floor, beside `table_pt`, documented with the
  two-questions table above. **[assumed]** table-only is right: prose and code never reach this
  branch today, so a per-class comfort floor would be three fields of which two are dead.
- **The reorder lives entirely in `probe.rs`'s text-atomic branch.** The image branch is untouched —
  images have no alternate and their ladder is unchanged.
- **`render.rs` needs no change** **[measured — read, not assumed]**. Reflow swaps the body for the
  alternate and orientation places it, with neither step aware of the other, so *reflow in portrait*
  already works: `(Reduction::Reflow, Some(alternate)) => alternate.to_string()`, then a separate
  `match decision.orientation`.
- **One golden hash moves** **[measured]** — `the_escalation_ladder_is_unchanged`, which holds an
  8-column table and is named for exactly this. The other five goldens hold no table. Regenerating
  it is a build step, recorded in the commit with what moved and why.

### Predicted census diff — acceptance criteria, written before the build

Under **O2a, F = 9.0**, `design/ladder-census.txt` should change like this **[assumed — this is the
prediction the build tests]**:

| Fixture | Now | Predicted |
|---|---|---|
| `shrink-floor.md` | 1 shrunk *(7.0pt)* | 1 reflowed |
| `shrink-slight.md` | 1 shrunk *(9.0pt)* | 1 shrunk — **on the boundary, kept** |
| `rotate.md` | 1 rotated, 1 shrunk | 1 reflowed — **name goes stale, rename it** |
| `reflow.md` | 1 reflowed, 1 rotated | 1 reflowed *(portrait now)* |
| `reflow-hostile.md` | 1 reflowed, 1 rotated | 1 reflowed *(portrait now)* |
| image fixtures | unchanged | unchanged — they keep `rotated` and `scaled` covered |

If the real diff differs from this table, **the simulation above was wrong** and its numbers cannot
be trusted for the corpus either. That is the cheap check on the whole analysis, and it costs
nothing because the census is regenerated anyway.

`shrink-slight.md` sitting exactly on the boundary is deliberate: it is the fixture that pins the
`>=` semantics, and it fails loudly if the comparison is ever flipped.

### Also worth building here

**Keep the corpus triage as a real instrument.** The class breakdown at the top of this document
required a throwaway probe, written twice now — once for T26a's clip triage and once for this. That
is **R4** in practice. Propose adding `triage_the_real_corpus` as an ignored test beside
`describe_the_fixtures`, taking its directory from an env var and defaulting to the fixture corpus,
so `documents/` stays untracked and optional.

---

## Evidence — rendered and looked at, 2026-08-22

> **Superseded by T26a2, later the same day.** The withdrawal below was correct about the *alternate*
> and wrong to conclude anything about the *order*: once the alternate emitted proportional columns,
> reflow beat deep shrinking on the same tables. Kept in full, because the reasoning is the record.
> **Read the re-vetting at the end of this document for what is actually being decided.**

**The recommendation above is withdrawn.** Not adjusted — withdrawn. The sheets say the premise
under it is wrong, and the reason is not the one this plan was arguing about.

Sheets in `design/evidence/t26b/`, built by cloning one real table and forcing each copy's decision
through `apply_override`, so each panel is what the product would actually produce at that rung:

| Sheet | Table | Source |
|---|---|---|
| 1 · shrink vs reflow, five ways | 6 columns, 22 rows | `factory/containerization-plan.md` |
| 2 · where reflow looks worst | 7 columns, 16 rows — **the widest in the corpus** | `support-audit.md` |
| 3 · the O2a/O2b pair | 3 columns, 12 rows, rotates and then fits | `design__task-flow.md` |

### What they show — consistently, on all three

**Deep shrinking beats reflow on real tables, and it is not close.**

- **Sheet 2** is the clearest. At **7pt**, all 16 rows fit in half a page, one line each, columns
  sized to their content. **Reflowed**, the same table spans two pages, every cell wrapped to 3–5
  lines — while `Tier` and `Priority`, which hold "Free" and "P1", are given the same width as
  `Path`.
- **Sheet 3** kills O2a outright. Panel A — today's landscape page — is 10pt, one line per row, and
  uses 40% of the page. Panel B gives `Step` and `Actor` a third of the width each to hold "1" and
  "System", and squeezes `Action`, the only column with content, into the remaining third.
- **Sheet 1**'s 9.5pt panel already wraps its long column while keeping every other column narrow —
  which is exactly the layout reflow *should* be producing and does not.

### The actual defect — the alternate, not the order

`Element.reflow` emits `columns: (1fr, 1fr, …)`. **`1fr` means equal shares**, so every column gets
the same width regardless of what it holds. On a table with one prose column and five narrow ones —
which is what real tables look like — that is close to the worst possible allocation.

T26a chose it against a *synthetic* 5-column table whose cells were all the same width, the one case
where equal columns are right. **That is why R2 looked true: the comparison that founded it was not
representative.**

Typst can express the correct thing directly — `columns: (auto, auto, 1fr)` — where narrow columns
size to content and the wide one absorbs the wrapping. That is what a browser does with a `<table>`,
and what the 9.5pt panel of sheet 1 shows Typst doing on its own.

### Consequence for the task

**T26b is premature and is parked.** Reordering the rungs decides *when* to prefer reflow, and the
evidence says the current reflow is worse than the thing it would displace at every setting. Moving
it earlier would make the output worse, and the census would faithfully record the degradation as
152 reflows and call it progress — R5, arriving exactly as written.

**New task, first: T26a2 — proportional reflow alternate.** Give `auto` to columns that do not need
to wrap and `1fr` only to the widest, decided in `emit.rs` from cell content length, which is
available without measuring. Then re-render these three sheets and ask the ordering question again
against an alternate worth choosing.

**Prediction, to be checked and not assumed:** with proportional columns, sheet 3 panel B becomes
close to panel A but in portrait, and the O2a/O2b question stops being obvious in favour of A.

### R3 is answered, and not the way it was posed

`compromise-mechanism.md` R3 worries about a 12-column reflow at ~40pt per column. **No table in the
corpus has 12 columns — the widest has 7** [measured]. Reflow's problem is not column *count* but
column *proportion*, and it shows up at three columns (sheet 3) as clearly as at seven.

### Separate finding — missing glyphs, not a ladder issue

Visible in every sheet-1 panel: the `Dockerfile` and `docker-compose` columns render as **tofu
boxes**. Probed directly (`design/evidence/t26b/glyphs.png`):

| Renders | Tofu |
|---|---|
| ⚠ ✓ ✗ → ▸, box drawing **inside a code fence** | **✅ U+2705, ❌ U+274C** |

Emoji-presentation characters only, and no vendored font covers them. **[measured]** ✅ appears in
28 of 146 documents and ❌ in 8 — so roughly a fifth of real documents render boxes where the author
put a tick. Box drawing in code fences, the case in 46 documents, is fine; box drawing in *prose*
renders but misaligns, and is rare.

Raised as its own task rather than folded in here: it is a font-coverage decision, not a ladder one.

---

## Evidence to produce before deciding

I produce these; **the call is yours** — it is a judgment about what reads well in your documents,
and no measurement settles it.

1. **A comparison sheet**: one real wide table rendered five ways — 9.5pt, 9pt, 8pt, 7pt, and
   reflowed at full size — on one page, labelled.
2. **The hostile case**: the 12-column table, where reflow squeezes columns hardest and looks
   *worst*. This is the case that decides whether O1 is safe.
3. **The rotation pair**, which finding #3 makes necessary: one of the 10 `landscape/none` tables
   rendered as it is today — landscape, full size, authored proportions — beside the same table
   reflowed in portrait. **This one question decides O2a vs O2b.**

Item 2 matters most for the floor; item 3 for the shape of the rule.

## Exit criteria

1. The order is decided, by you, from rendered evidence rather than reasoning.
2. The census diff matches the prediction above, or the discrepancy is explained.
3. `compromise-mechanism.md` §6 re-measured and updated; R2 closed or re-scoped; R5 revisited —
   rotation reaching zero means another signal has gone quiet.
4. `the_escalation_ladder_is_unchanged` regenerated deliberately, with the reason recorded.
5. `verify.sh` green.

---

## Doubts — audited before building

### D1 · Is the simulation valid? — **not verified, and cheap to check later**

The counts assume reordering does not disturb the portrait shrink sizes. Sound in principle: portrait
is measured first and the comfort floor only decides what happens *after* **[measured — read from
`probe.rs`]**. But it is still a model of the code rather than the code.

Verified for free at build time by comparing the predicted census diff against the real one, and the
corpus counts against a fresh run. Recorded rather than resolved because resolving it early costs
more than the check will.

### D2 · Does a comfort floor make the two floors confusable? — **not verified, accepted**

Two numbers on the same class, both in points, answering different questions. A future reader may
well tune the wrong one. Mitigated by naming and by the two-questions table; **[assumed]** that is
enough, and it is the kind of thing only a later reader can disprove.

### D3 · Is 12-column reflow readable? — **unverified, and it gates O1**

`compromise-mechanism.md` R3 says a 12-column reflow squeezes to ~40pt per column, possibly one word
per line. `reflow-hostile.md` exists for this and **nobody has looked at it yet**. Evidence item 2.

If it is unreadable, the answer is a *column-count* condition rather than a size one, and O1 is
unsafe at any floor.

### D4 · Does rotation going to zero lose a signal? — **live, carried forward as R5**

Rotation is currently the loudest thing in the census after clipping, and every option here takes it
to zero for tables. **[measured]** no image in the corpus reaches the ladder, so the `rotated` rung
would be exercised only by the fixture corpus.

That is not an argument against the change; it is a note that the fixture census becomes the *only*
place rotation is observed, which is an argument for the fixture corpus existing at all.

---

# Re-vetted — 2026-08-22, after T26a2

**Status: planned and vetted. One gate before building, stated at the end.**

T26a2 gave the alternate proportional columns, and the comparison re-rendered afterwards reverses
the withdrawal above **[measured, by eye]**: today's 7pt shrink beside the new reflow of the same
table, and the reflow is 10pt with one line per row but for one
(`design/evidence/t26a2/after-sheet1-p0.png`). So **R2's original claim stands** — reflow beats deep
shrinking — and the question this plan was written to answer is live again.

## What preferring reflow costs — measured, not estimated

Reflow is the form that overflows the page on unbreakable content (**T29**). So moving elements into
it spends against a known defect, and the honest question is not *"is reflow nicer"* but *"how many
of the elements it would newly claim would break?"*

Every table in the corpus was rendered reflowed in portrait, with ink past the right margin detected
in the raster — an actual overflow check, not a proxy **[measured]**:

| Rung today | Count | Long token 30+ | **Overflows if reflowed** |
|---|---|---|---|
| **already reflows** | 25 | 13 | **4 — today, in shipped output** |
| landscape + shrink | 49 | 24 | **2** |
| landscape, fits at full size | 10 | 2 | 0 |
| portrait shrink 9pt+ | 46 | 19 | 0 |
| portrait shrink below 9pt | 68 | 27 | **1** |

Three things fall out.

**1. T29 is not a risk this plan would introduce. It is already shipping.** Four elements in the
corpus render *today* with text running off the page, reported as `Reflowed` — the mechanism's own
words for "handled". That is R3's silent degradation, live, in committed behaviour.

**2. T26b would newly break 3 elements.** At a 9pt comfort floor with reflow preferred over
rotation: 1 from the portrait-below-9pt group, 2 from landscape+shrink. Out of 117 elements moved,
so **~2.6%** — small, real, and avoidable.

**3. The cheap proxy is nearly useless.** 85 elements contain a 30+ character unbreakable token;
**7** actually overflow. Reasoning from token length would have overstated the exposure roughly
twelvefold and, on that basis, this plan would have been shelved. Only rendering answers it —
the same lesson as the sheets, in a case where the plausible shortcut was available and wrong.

## The order

**O2, with a 9.0pt comfort floor, and reflow preferred over rotation.**

- **Not O1** (an alternate always wins). Unchanged from the analysis above: every shrink in the
  product is a table, so O1 makes `Reduction::Shrink` unreachable product-wide. It would also
  discard authored column proportions for the 46 elements that shrink imperceptibly to 9.0–9.5pt,
  where **[measured]** nothing overflows and nothing looks wrong.
- **Not O3** (shrink all the way first). Fixes the wrong half; the 7.0–7.5pt outcomes are the ones
  the evidence objects to.
- **9.0pt**, so a squeeze of half a point either way keeps the authored shape and anything deeper
  wraps at full size. A shrink is kept iff `chosen_size >= comfort_floor` — 9.0pt exactly stays
  shrunk, which `shrink-slight.md` pins.

### O2a vs O2b — **your call, and the pair is rendered**

They differ over exactly 10 elements: the tables that rotate and then fit at full size.

- **O2a** — reflow before any rotation. Table rotation disappears; rotation becomes image-only.
- **O2b** — try landscape at base size first, then reflow. Those 10 keep their landscape page.

**[measured]** exposure is identical: 0 of the 10 overflow either way. So this is purely about
reading, and `design/evidence/t26a2/after-sheet3-p2.png` is the pair — panel A rotated at full size,
panel B reflowed in portrait. Panel B now reads well and costs no page turn, which is why **O2a is
recommended**, but O2b is defensible and the evidence for choosing is on the page rather than here.

## Expected effect on the baseline

**[measured — simulated from the shrink distribution, which T26a2 did not move]**, at 9.0pt, O2a:

| | shrunk | reflowed | rotated | flagged |
|---|---|---|---|---|
| today | 163 | 25 | 84 | 70 (48%) |
| after | 46 | 152 | 0 | 70 (48%) |

**The flagged count does not move, and that is correct** — the same elements are compromised, just
better. **T26b does nothing for R1**, and if a candidate ever did drop the flagged count, something
would be being hidden rather than fixed.

Rotation reaching zero for tables is **R5** arriving as written: the ladder's second-loudest signal
goes quiet. The fixture census becomes the only place `rotated` is observed — via the image
fixtures, which **[measured]** still produce it.

## Implementation

- `Floors` gains a table-only comfort floor beside `table_pt`, documented with the two-questions
  table above.
- The reorder lives entirely in `probe.rs`'s text-atomic branch; the image branch is untouched.
- `render.rs` needs no change **[measured — read, not assumed]**: the axes compose, so reflow in
  portrait already works.
- Goldens: `a_reflowed_table_is_unchanged` should hold (already reflows); the ladder golden shrinks
  to 7.5pt today and **will move** to a reflow — regenerate deliberately, with the reason recorded.
  This time that prediction is checkable rather than assumed, because `golden_at` asserts the rung.

### Predicted census diff — written before building

| Fixture | Now | Predicted |
|---|---|---|
| `shrink-floor.md` (7.0pt) | 1 shrunk | 1 reflowed |
| `shrink-slight.md` (9.0pt) | 1 shrunk | 1 shrunk — **boundary, kept** |
| `rotate.md` (landscape 9.5pt) | 1 rotated, 1 shrunk | 1 reflowed — **rename, the name goes stale** |
| `reflow.md`, `reflow-hostile.md` | 1 reflowed, 1 rotated | 1 reflowed — portrait now |
| image fixtures | unchanged | unchanged — they keep `rotated` and `scaled` covered |

If the real diff differs, the simulation was wrong and its corpus numbers cannot be trusted either.

## The gate — **T29 first**

**Recommend building T29 before T26b.** Not because T26b is unsound, but because the sequence is
free to get right and expensive to get wrong:

- T29 fixes **4 elements that are broken today** and prevents the **3** T26b would break. Done
  first, T26b lands with zero new damage; done second, three documents regress in between.
- It is contained: give long tokens a break opportunity in the alternate, where the wrapping is.
- **[assumed]** it is small. That assumption is worth testing before committing to the order — if
  T29 turns out to be a large task, building T26b first and accepting three regressions is a
  reasonable trade, and that is a decision to make with the size in hand.

**If you would rather have the reading improvement now, T26b is ready to build as specified** —
the cost is three elements joining four already-broken ones, and both are fixed by T29 afterwards.

## Doubts — audited

### D1 · Is the simulation still valid after T26a2? — **VERIFIED**

T26a2 changed how the alternate *renders*, not which rung is chosen — the census was unchanged
across it **[measured]**. So the shrink distribution the counts are computed from is untouched.

### D2 · Is 9.0pt the right comfort floor? — **not verified, and deliberately not tuned**

**[assumed]**. T26c exists for this, against rendered pages. Kept as one named constant.

### D3 · Does the overflow check have false negatives? — **not verified, bounded**

It detects ink past the right margin at 1 pt/px on page 1 only. A table overflowing solely on a
later page would be missed. **[assumed]** rare — overflow comes from a wide row, and the widest row
is usually the header or an early one. Worth knowing the number is a floor, not an exact count.

### D4 · Does the census still cover `rotated` after tables stop rotating? — **VERIFIED**

`image-clipped.md` and `image-scaled.md` both decide Landscape **[measured]**, so the rung stays
covered and `the_census_covers_every_compromise_kind` stays honest.
