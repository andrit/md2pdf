# Plan — a reflow alternate worth choosing (T26a2)

**Written:** 2026-08-22 · **Follows:** the T26b comparison sheets, which withdrew the reordering
recommendation. **Blocks:** T26b.

Every claim is marked **[measured]** or **[assumed]**, per *Planning discipline* in
`design/invariants.md`.

---

## The defect

`Element.reflow` emits `columns: (1fr, 1fr, …)`. **`1fr` means equal shares**, so a column holding
"P1" gets the same width as a column holding a paragraph. On real tables that is close to the worst
possible allocation, and it is why deep shrinking currently beats reflow on every sheet rendered.

## The rule

**A column shares the slack if it is at least half as wide as the widest column in the same table;
otherwise it sizes to its content.**

```
columns: (auto, auto, 1fr, auto, 1fr)
```

Relative, not absolute — it adapts to each table's own shape rather than needing a corpus-tuned
constant. **[measured]** the corpus offers no natural absolute threshold anyway: per-column max cell
length runs p10 = 8, p50 = 29, p90 = 84 chars over 1215 columns in 434 tables, a smooth spread with
no gap to cut at.

The widest column always qualifies (it is half of itself), so at least one column always absorbs the
slack and the alternate never degenerates into the all-`auto` body.

## What was measured before choosing it

Synthetic tables, three shapes, four specs, rendered and checked for ink past the right margin
(`design/evidence/t26a2/`):

| Case | all-auto (body) | all-1fr (today) | mixed, widest only | **relative 50%** |
|---|---|---|---|---|
| realistic (6 cols, one prose) | 484.8pt, fits | fits | 204.5pt, fits | **204.5pt, fits** |
| two prose columns | 563.6pt, fits | fits | 309.4pt, fits | **47.3pt, fits** |
| unbreakable 40-char tokens | 1706pt, **overflows** | **overflows** | **overflows** | **overflows** |

Three facts fell out, all **[measured]**:

1. **`1fr` measures as 0.0pt in infinite space.** Fractional columns collapse when unconstrained, so
   the probe *cannot* measure the alternate — it is structurally assumed to fit. That is why no
   measurement guards this rung today.
2. **The relative rule fits everywhere the current one fits**, and renders the layout the sheets
   said was wanted: narrow columns narrow, prose column taking the rest at full size, one line per
   row (`realistic-relative 50% rule.png`).
3. **Today's "always fits" guarantee is already false.** With unbreakable content, all-`1fr`
   overflows *and* the cells overprint each other into unreadable mush that runs off the page —
   while the ladder records `Reflowed` and calls it handled.

## The guarantee, stated honestly

`Reflow` sits immediately before `Clip` because the alternate is supposed to always fit. **It does
not, and did not before this change.** T26a2 does not fix that and does not worsen it — the failure
is unbreakable *content*, not column *proportion*.

**[measured]** the case is live, not theoretical: **261 table cells** across the corpus contain a
token of 30+ characters with no break opportunity — URLs and absolute file paths, mostly.

Raised as **T29** (insert break opportunities into long tokens in the alternate). The guarantee is
restored only when both are done, and the roadmap says so rather than implying the rung is sound.

## Implementation

One file. `emit.rs` already keeps `table: Option<(usize, String)>` for building the alternate; it
gains the per-column widths it needs to build a better one.

- `Emitter` gains `cell_widths: Vec<usize>` and `cell_index: usize`, reset when a `Tag::Table`
  opens.
- `Tag::TableCell` records the cell's length against its column, `cell_index % columns`.
- The alternate is built from `cell_widths` instead of `vec!["1fr"; columns]`.

**[assumed]** measuring the *emitted markup* rather than the rendered text is close enough. A header
cell carries a constant `#strong[…]` wrapper, which cancels out across columns; a cell containing a
link carries its URL, which inflates it. The rule is relative and errs toward granting slack, so the
failure mode is a narrow column being given `1fr` — harmless. Recorded as a `ponytail:` rather than
solved.

## Predicted census diff — written before building

**[assumed]** — this is the prediction the build tests. The census records *kinds*, and this changes
no rung, so:

| Fixture | Predicted |
|---|---|
| every line | **unchanged** |

A reflow is still a reflow; only its appearance improves. **If the census moves, the change did
something it was not supposed to do.** The one golden that holds a wide table
(`the_escalation_ladder_is_unchanged`) *will* move, because the emitted markup changes — that is
expected and gets regenerated deliberately.

## Exit criteria

1. The alternate emits mixed `auto`/`1fr`, by the relative rule. ✅
2. A unit test pins the rule: narrow columns `auto`, wide columns `1fr`, at least one `1fr`. ✅ — five
3. The census is **unchanged** ✅; the ladder golden regenerated — **did not move, see below**.
4. Sheets re-rendered and looked at — the point of the task is how it *looks*. ✅
5. `verify.sh` green. ✅

---

## Built — 2026-08-22

### The outcome reverses the sheets that motivated it

`design/evidence/t26a2/after-sheet1-p0.png` puts today's 7pt shrink above the new reflow of the same
table. The reflow is 10pt, one line per row but for one, and plainly easier to read. `after-sheet3-p2.png`
is the O2a/O2b pair: `Step` and `Actor` are now narrow, `Action` takes the rest, and 10 of 12 rows fit
on one line — **in portrait, with no page turn**, which is what rotation was buying.

So **reflow now beats deep shrinking**, which is what R2 claimed all along. R2 was right about the
conclusion and wrong about the reason, and acting on it before fixing the alternate would have made
the output worse. **T26b is unparked**, and the ordering question is now worth asking.

### The prediction about the golden was wrong

The plan said `the_escalation_ladder_is_unchanged` would move. **It did not, and no golden did.**

Its table gets `Portrait / Shrink 7.5pt` [measured] — it never reaches Reflow, so the alternate is
built and thrown away. **No golden covered the reflowed rendering at all**, which means this change
could have landed silently despite being entirely about how a reflowed table looks.

Closed by `a_reflowed_table_is_unchanged`, and by `golden_at`, which asserts *which rung was chosen*
before hashing. A golden hash cannot say what it covers; now this one can, and it fails loudly if the
fixture ever stops exercising the rung it was written for — the same defect as the census coverage
test that was satisfied by its own filenames.

### The census was unchanged, as predicted

No rung moved, so no census line moved. The prediction that the census would *not* move was as much
the point as the ones that expected change: it says the improvement stayed inside the rung.

---

## Doubts — audited before building

### D1 · Does the relative rule beat "widest column only"? — **VERIFIED by measurement**

Both are identical on the realistic case. They differ when two columns are both prose: widest-only
gives one of them `auto`, which then sizes to its full content. The relative rule gives both `1fr`
and the table fits with room to spare (47.3pt vs 309.4pt natural). Relative is better, and never
worse, because it is a superset of widest-only.

### D2 · Is 50% the right fraction? — **not verified, and deliberately not tuned**

**[assumed]** Nothing measured says 50% rather than 40% or 60%. It is a starting point, and the
sheets in criterion 4 are how it gets judged. Tuning it against the corpus before looking at output
would be optimising a number nobody has seen the effect of — the T26a mistake exactly.

Kept as a named constant so it is one edit and one census run to change.

### D3 · Could a table now be *worse* than today? — **not verified, argued**

`auto` columns can exceed the available width where `1fr` would not, so in principle a table with
many medium-width columns could overflow under the new rule but not the old. **[measured]** neither
of the two fitting cases came close, and the unbreakable case already fails under both.

Not provable in advance without an overflow oracle. Criterion 4 (look at re-rendered sheets) and the
census are the checks. If it happens, the fallback is to lower the fraction, which grants `1fr` to
more columns and converges on today's behaviour.

### D4 · Should this wait for T29? — **no**

They are independent: proportion and breakability. Shipping the proportion fix now makes every
table that *does* fit read properly, and leaves the overflow case exactly as bad as it already is —
recorded, scheduled, and no longer believed to be handled.
