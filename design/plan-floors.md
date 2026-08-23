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

## Exit criteria

1. Three pairs rendered from real corpus tables and looked at; the comfort floor chosen **by you**.
2. Whatever changes: `compromise-mechanism.md` §6 re-measured, census regenerated, goldens
   deliberately updated.
3. **The overflow count does not rise** — checked with the oracle, not assumed.
4. F4 closed: the four dead floors resolved in code and in their doc comments.
5. `verify.sh` green.

---

## Doubts — audited

### D1 · Does a lower comfort floor increase overflow? — **must be measured, not argued**

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
