# Plan — the floors, chosen by eye (T26c)

**Written:** 2026-08-23 · **Follows:** T26b/T29c, which changed what the floors actually do.
**Closes or re-scopes:** flag **F4**. **Reference:** `compromise-mechanism.md` §9 — every ladder
change re-measures the baseline.

Claims marked **[measured]** or **[assumed]**, per *Planning discipline* in `design/invariants.md`.

---

## The task is smaller than its name

T26c was scheduled as *"floors chosen against the corpus"*, back when the ladder had one kind of
floor and five of them looked equally live. **Four of the five have no observable effect.**

| Floor | Default | Consulted at | Does it change output? |
|---|---|---|---|
| `prose_pt` | 9.0 | `for_class`, probe.rs:122 | **never** |
| `code_pt` | 7.0 | `for_class`, probe.rs:122 | **never** |
| `table_pt` | 7.0 | probe.rs:122, render.rs:74 | only above the comfort floor, plus the clip marker |
| `image_scale` | 0.25 | probe.rs:115 | yes, but nothing in the corpus reaches it |
| **`table_comfort_pt`** | **9.0** | probe.rs:146 | **yes — this is the whole task** |

**[measured]** `for_class` is called from exactly one place: the probe's text-atomic branch. That
branch runs only for atomic classes, `is_atomic` is `Table | Image`, and `Image` takes the scale
branch instead. **So `for_class` only ever receives `Table`** — `prose_pt` and `code_pt` are dead
parameters, and have been since `Code` was classified as wrappable.

**[measured]** `table_pt` is inert while it sits at or below the comfort floor (**F4**): any size it
would permit below 9.0pt is turned into a reflow before it can be used. It becomes observable again
only if raised *above* the comfort floor, and it still sets the clip marker's text size.

**[measured]** no image in the corpus reaches the ladder — all 198 compromised elements are tables —
so `image_scale` cannot be judged against real documents at all.

**This is the shape of the task: one number to choose by eye, and four to resolve honestly.**

---

## Part 1 · Choose the comfort floor

The only question that changes output: **at what size does wrapping read better than shrinking?**

**[measured]** the portrait shrink distribution, and what each candidate does to it:

| Chosen size | count | at floor 8.0 | at **9.0** | at 9.5 |
|---|---|---|---|---|
| 7.0pt | 13 | reflow | reflow | reflow |
| 7.5pt | 15 | reflow | reflow | reflow |
| 8.0pt | 24 | **shrink** | reflow | reflow |
| 8.5pt | 16 | **shrink** | reflow | reflow |
| 9.0pt | 27 | **shrink** | **shrink** | reflow |
| 9.5pt | 19 | **shrink** | **shrink** | **shrink** |
| | | 86 shrunk / 112 reflowed | 46 / 152 | 19 / 179 |

### The evidence to produce

**Three pairs, each a real corpus table, each rendered both ways it could go.** One pair per
candidate boundary — a table that shrinks to 8.0pt, one to 8.5pt, one to 9.0pt — shown shrunk at
that size beside the same table reflowed at full size.

That is the actual choice, stated as a question a person can answer by looking: *at this size, would
you rather it wrapped?* The boundary is wherever your answer flips.

**These must be re-rendered, not reused.** The earlier T26b sheets compared against a reflow with
equal `1fr` columns and no break opportunities — three commits and two defects ago. **[measured]**
that alternate lost to deep shrinking; the current one may not, and judging from the old sheets
would be judging a rendering that no longer exists.

## Part 2 · Resolve the four dead floors

Not tuning — deciding what they are. **[assumed]** throughout; these are judgment calls, not
measurements.

- **`prose_pt`, `code_pt`** — remove, or keep as the contract for a future wrappable-but-atomic
  class? They are currently fields that a reader will reasonably assume do something. Recommend
  **removing them and saying why in the type**, since `Floors::for_class` can then stop pretending
  to dispatch. A template author tuning a dead number is worse than a missing feature.
- **`table_pt`** — recommend **keeping**, renamed or re-documented as what it is: the size below
  which the *clip marker* is drawn, and the lower bound of the probe's scan. It is not "how small a
  table may get" any more, and the doc comment still says so.
- **`image_scale`** — recommend **leaving at 0.25 and marking it untuned**, because there is no
  honest way to choose it here. It could be judged against synthetic fixtures, but that would be
  tuning a number against invented documents and calling it evidence.

## What was chosen — 2026-08-24

**`table_comfort_pt`: 9.0 → 10.0.**

Five pairs rendered from real corpus tables, each shown shrunk and reflowed:

| Would shrink to | Shrunk | Reflowed at full size | Reads better |
|---|---|---|---|
| 8.0pt | legible but a third off the base | wraps to two lines per row | **reflow** |
| 9.0pt | compact, one line per row | full size, two rows wrap | **reflow** |
| 10.0pt | clean, one line per row | half the rows wrap | **shrink** |
| 11.0pt | indistinguishable from body | wraps a one-row table for nothing | **shrink** |

The turnover sits between 9.0 and 10.0, so the floor is 10.0.

**It moved because the base moved.** T30 raised `base_size_pt` to 12pt, and comfort is relative
even though the floor is absolute: 9.0 was a tenth off a 10pt base and is a quarter off a 12pt one.
`plan-base-size.md` D2 predicted exactly this and left it here. **[assumed]** 9.5 is defensible on
the same evidence — one step wide, and a judgement about reading rather than a measurement.

**Two pairs beyond the plan's three**, because the plan's candidate boundaries (8.0/8.5/9.0) were
chosen at the 10pt base and all four of those now reflow. The question had moved up with the base.

### It found a defect in T31a, and only by looking

The 9.0 reflow put `integration.destination.verified` **over its column border**. The min-content
token was being measured as plain text and drawn in mono, which is wider — so the column came out
slightly too narrow. Fixed by carrying the token's markup, not its text, into the measurement.

**No count would have caught it.** The oracle saw no ink past the margin (**F11**), the census was
unchanged, every test was green. This is the second defect in two tasks found by rendering a page.

## Exit criteria

1. ✅ Pairs rendered from real corpus tables and looked at; floor chosen.
2. ✅ Census regenerated — unchanged, after re-sizing the boundary fixture (below).
3. ✅ **Overflow did not rise** — checked with the oracle at the new floor.
4. ✅ F4 closed: `prose_pt` and `code_pt` removed, `table_pt` re-documented as a bound rather than
   a policy, `image_scale` marked untuned with the reason.
5. ✅ `verify.sh` green. Goldens **unmoved** — no golden fixture's table sits in the 9.0–10.0 band.

### The boundary fixture had to be re-sized

`shrink-to-comfort-floor.md` was sized to land at exactly 9.0 and pin the `>=` boundary. At a 10.0
floor it reflowed — and it is **the only fixture that shrinks**, so `the_census_covers_every_
compromise_kind` went red and the census lost sight of a whole rung. Two characters came off each
cell to put it back on the boundary at 10.0. The name still describes what it does.

---

## Doubts — audited

### D1 · Does a higher comfort floor increase overflow? — **measured: no**

More reflows means more elements on the rung that can still overflow — **[measured]** 1 of 152
today, after T29c. At floor 8.0 there would be 112 reflows instead of 152, which is *fewer*; at 9.5,
179, which is more.

So the direction depends on which way you move, and the count is small enough that a single new
overflow would be a meaningful percentage change. Exit criterion 3 checks it with the oracle at
whichever value is chosen.

### D2 · Do the break limits interact with the comfort floor? — **no** **[measured]**

Break limits come from column *weights*, computed at emit time from cell content. They know nothing
about font size, and the comfort floor changes only which rung is chosen. The two are independent.

### D3 · Is one pair per boundary enough to judge? — **not verified**

**[assumed]** three real tables is enough to see where the turnover is. The risk is picking three
unrepresentative tables — a table with one prose column reflows beautifully, a table of seven
similar columns less so.

Mitigation: choose the three with *different shapes*, and say which shape each is when presenting
them, so a judgment made on one shape is not silently applied to all.

### D4 · Should the dead floors be removed rather than documented? — **your call, recorded either way**

Removing `prose_pt` and `code_pt` is a breaking change to `Floors`, which 3e will load from
`template.toml`. Doing it **before** the template format exists is free; doing it after means a
migration for a field that never did anything.

**[assumed]** that makes now the right time — the gate test in `invariants.md`: cheap now, lossy
later.
