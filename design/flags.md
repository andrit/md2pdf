# Flags — the things worth noticing, typed

**Convention (from 2026-08-22):** anything worth flagging gets flagged **here**, with a type, one
line of what it is, the evidence, and where it is tracked. Prose buried in a plan is not a flag —
plans are read once, when the task is built.

A flag is not a task. It is the record that something was *noticed*; a task is the decision to act.
Flags graduate to the ledger, or they sit here until they matter, or they get closed as wrong.

| Type | Means |
|---|---|
| `DEFECT` | wrong output, known, unfixed |
| `DEBT` | deliberate shortcut with a ceiling — pairs with an inline `ponytail:` |
| `INSTRUMENT` | missing logging, tracking or reporting; we cannot see something we need to see |
| `CONSTRAINT` | an environmental limit found by measurement, not by assumption |
| `DECISION` | needs the operator; blocked on judgment, not on work |

Ladder-specific failure modes live in `compromise-mechanism.md`'s risk register (`R1`–`R5`) — that
is a deeper analysis of one mechanism, and it stays there. This is everything else.

---

## Open

### F11 · `INSTRUMENT` — the oracle cannot see a table overprinting itself

**[measured] 2026-08-23**, while spiking Option 4 of `plan-typeset-move.md`. A column spec that
starves narrow columns makes their contents print *on top of each other* — `Phase 3C`, `79`, `145`
and `14` in one illegible stack — while the table's outer edge stays inside the margin.

`overflow()` looks for ink to the right of the text area. There is none. **It reported the page
clean.**

This is R3's "silent degradation" with a second face: not only does a recorded `Reflowed` look
handled, the instrument built to check it agrees. Every "corpus overflow is 0" claim in
`plan-typeset-move.md` therefore proves *necessary, not sufficient* — including Option 1's.

**What would close it:** a check for ink overlapping a cell boundary, or more cheaply, comparing a
cell's measured natural width against the width its column was actually given. The second is nearly
free once anything measures cells — which is what Options 2/3/4 do.

**Until then:** `look.rs` renders a page to PNG so a human can check. Numbers do not settle this one.

**Tracked:** `plan-typeset-move.md`, Option 4 section. Interacts with **R3**.

### F10 · `DEFECT` — the base size was never chosen · **CLOSED 2026-08-23 (T30)**

> **Closed.** The base is 12pt, with the reason in the type. `CHARS_ACROSS` is gone — break limits
> now come from the Template, and from each column's own share *after* its own inset. Measured:
> flagged 48% → 64%, shrinks 46 → 122, reflow unchanged at 152, corpus overflow unchanged at 1.
> Nothing renders smaller than it did; the reporting became honest. **T26c is unblocked.**
>
> The consequences below were all worked through, and one prediction was wrong — fixture sizes did
> *not* move, because the ladder's chosen size is absolute. See `plan-base-size.md` step 3.

`Template::default()` sets `base_size_pt: 10.0`, and **[measured]** nothing in the repository records
why. It appears in the font spike as a specimen setting — *"10pt body"* — and was carried forward as
though it were a decision.

Two things follow, and the second is the serious one.

**The template is called `github-print` and GitHub's body text is 16px ≈ 12pt.** The fidelity target
and the base disagree, in the one dimension a reader notices first.

**Every Compromise has been measured against a baseline that is itself a compromise.** The whole
mechanism reports departures from `base_size_pt`, and treats sitting *at* it as clean. If the optimal
body size is 12pt, then rendering at 10pt is a concession that no Diagnostic has ever recorded — 146
documents converted "cleanly" at a size nobody chose.

**Consequences to work through**, not yet measured:

- The shrink distribution and the flagged percentage will both move. More content will not fit at
  12pt, so **R1 (48% flagged) probably gets worse before it gets better** — and that is the honest
  number, not a regression.
- `CHARS_ACROSS = 96` in `emit.rs` is *"A4 minus margins at 10pt"* — a hardcoded constant that is
  wrong at any other base. It should be derived, or the ponytail's ceiling has been reached.
- **T26c cannot be settled first.** Choosing a comfort floor against a 10pt base is choosing it
  against the wrong target; the three pairs must be re-rendered at 12pt.

**Tracked:** **T30** — done.

### F1 · `INSTRUMENT` — no overflow oracle · **BUILT 2026-08-22 (T29)**

> Built as `md2pdf-engine/tests/walking_skeleton/overflow.rs`. **It found a real overflow in a
> committed fixture on its first run** — `below-comfort-reflows.md`, spilling 21pt, which no other
> test could see. Kept open as a flag because the CLI `--check-overflow` half is not built.


Typst does not report overflow — it lays content out and anything too wide simply extends past the
margin. The only way to answer the question is to render the page and look at the pixels, and that
had been written as throwaway code **four times** before it was kept.

It matters because **152 of 198 compromised elements sit on the reflow rung**, and reflow can
overflow *silently*: the ladder records `Reflowed`, which every consumer reads as handled. No golden
hash helps — a hash says the bytes changed, not that the output is wrong. This is the only check in
the project that judges the *result* rather than the decision.

**Remaining:** the `--check-overflow` CLI mode, so the question can be asked of a real directory
without writing a test. Left open for that reason.

### F2 · `INSTRUMENT` — the corpus cannot be triaged without throwaway code

A `Compromise` carries an `ElementId` and a kind — not the element's class, content, or page. Every
question of the form *"what did this happen to?"* has required a one-off probe: which elements were
clipped (T26a), which are tables (T26b), which would overflow (T26b).

This is `R4` seen from the tooling side. The attention gate in 3f cannot say *"the table on page 4"*
either, so this instrument and that feature are the same underlying gap.

**Tracked:** `R4`; feature work lands in 3f. Proposed interim: keep `triage_the_real_corpus` as an
ignored test taking its directory from an env var.

### F3 · `DEFECT` — nothing evicts the `comemo` cache · **four sightings, mechanism found**

| When | What was running |
|---|---|
| 2026-08-22 | T26b exposure — ~200 renders through one `Typesetter` |
| 2026-08-22 | same, after switching to one instance per element — still killed |
| 2026-08-23 | T30 base-size comparison — two *probes* per document, 146 documents |
| **2026-08-23** | **the same, with a fresh `Typesetter` per probe — killed anyway** |

**The fourth sighting found the mechanism, and retired the workaround.** Constructing a fresh
`Typesetter` per probe did not help, because **`comemo`'s cache is process-global**, keyed by
memoised call rather than held by the `World`. Dropping a `Typesetter` frees nothing. Both earlier
"fixes" were placebo — they changed how the work was scoped, not what was retained, and the runs that
survived did so because they did less work, not because they released anything.

**[measured]** `comemo` 0.5.1 is in the dependency graph via typst, and **nothing in this project
ever calls `comemo::evict`**. Typst's own CLI calls it between compilations; md2pdf does not.

That reclassifies this from `CONSTRAINT` to `DEFECT`: it is not an environmental limit we work
around, it is a missing call in our own code.

**It is information about the product.** Phase 3f's recompile loop is designed around a long-lived
`World` precisely so `comemo` can memoise across compilations. That is the right design for speed and
it is the same shape as the loop that has now died four times. A user adjusting an override on a
large document, recompiling repeatedly, is not obviously different from what was running here.

**Scheduled: T31.** The likely fix is a `comemo::evict` at the right point in `Typesetter`, which
makes it a product change rather than test scaffolding. Open questions for the inspection: where the
call belongs (per compilation? per Job? per batch?), what eviction age preserves the memoisation the
long-lived `World` exists for, and whether the batch path is already accumulating silently — **[measured]**
146 documents complete in release without dying, which may mean the bound is simply higher there
rather than absent.

Before 3f, whose recompile loop is the same shape as the loop that has now died four times.

### F4 · `DEBT` — `Floors::table_pt` is now inert for any table with an alternate · **CLOSED 2026-08-24 (T26c)**

> **Closed by resolving all five floors, not just this one.**
>
> - **`prose_pt`, `code_pt` — removed.** **[measured]** never read: `for_class` is called from one
>   place, that branch runs only for atomic classes, and of those only `Table` reaches it. Removed
>   while `Floors` is still a Rust type; after 3e loads it from `template.toml` the same removal is a
>   migration for fields that never did anything.
> - **`table_pt` — kept, re-documented.** It is not "how small a table may get"; the comfort floor
>   decides that and sits above it. It bounds the probe's scan and sets the clip marker's text size,
>   and the doc comment now says so.
> - **`table_comfort_pt` — 9.0 → 10.0, chosen by eye** against five rendered pairs from real corpus
>   tables. It moved because the base moved: 9.0 was a tenth off a 10pt base and a quarter off 12pt.
> - **`image_scale` — left at 0.25 and marked untuned**, because no image in the corpus reaches the
>   ladder. Tuning it against invented fixtures would be choosing a number and calling it evidence.
>
> **[measured]** the result: shrinks 122 → 76, reflows 152 → 198, **0 of the 198 overflow**.



After T26b a table shrinks only while it stays at or above the 9pt comfort floor; anything below
wraps instead. So the 7pt hard floor is never consulted for a table that can reflow. It still governs
the clip marker and atomic text with no alternate.

Not dead, but far narrower than it looks, and a reader tuning `table_pt` expecting an effect will
find none.

**Tracked:** T26c should decide whether it stays, merges with the comfort floor, or is documented as
image-and-clip-only.

### F5 · `CONSTRAINT` — the §6 baseline cannot be reproduced from the repository

`compromise-mechanism.md` §6 measures 146 documents in `documents/`, which is untracked, borrowed
from other projects, and may be deleted at any time. The fixture census (T27) is reproducible
forever; the real-corpus baseline is not.

Accepted deliberately — the two answer different questions — but it means every §6 number is a
historical claim rather than a re-runnable one.

**Tracked:** `plan-census.md` D6; interacts with the open `documents/` decision.

### F6 · `DECISION` — `documents/` is still untracked and unresolved

152 files of other projects' artifacts, appearing in every `git status`, load-bearing for every
corpus measurement taken so far. Gitignore, delete, or commit.

**Tracked:** roadmap, *Decisions open for the operator*.

### F8 · `DEFECT` — tables that run off the page · **CLOSED 2026-08-24 (T31a) — 0, from 7**

| | count | worst spill |
|---|---|---|
| before T29 | 7 of 152 | — |
| T29 · break long runs | 4 | 55pt |
| T29b · weighted `fr` columns | 4 | 9pt |
| **T29c · per-column break limits** | **1** | **2pt** |
| T30 · per-column inset | **1** | **2pt** |
| **T31a · min-content columns** | **0** | **—** |

> **Closed.** The last overhang went when columns stopped being allowed to be narrower than
> their own longest word. Every earlier fix moved *where* text was broken; this one removed the
> reason it had to be. **[measured]** 423 tables can reflow, 152 do, 0 overflow — and separately,
> the ordinary words we were breaking went 579 → 0.
>
> **Read with F11.** Zero overflow is necessary, not sufficient: the oracle cannot see a table
> overprinting itself. The rendered page was checked by eye as well (`plan-typeset-move.md`).

Each step was diagnosed rather than guessed, and each diagnosis contradicted the previous plan's
expectation:

- **T29** — long runs cannot wrap, so give them break opportunities. Fixed the cases above the
  threshold; the threshold itself turned out to be the next problem.
- **T29b** — `auto` columns *size to content and refuse to shrink*, so several summed past the page
  while the lone `1fr` took negative space. Replaced with weighted fractional columns, which divide
  the width and therefore cannot exceed it.
- **T29c** — `fr` bounds the *table*, not the *cell*. A single break threshold assumed every column
  had equal room, which stopped being true the moment the spec became deliberately lopsided. Each
  column's limit now comes from its own weight.

**The remaining one:** `CLAUDE.md`, 2pt. **[assumed]** the break limit clamps at a floor of 6
characters, so a column with a very small share still cannot break its content finely enough. Not
diagnosed — 2pt is one character's overhang and the next step down starts trading against
readability, so it is recorded rather than chased.

**Tracked:** open, unscheduled. Worth revisiting only if the count grows.

### F9 · `DEBT` — breaks land mid-identifier · **FIXED 2026-08-23**

**Observed** in the T26c pairs (`design/evidence/t26c/pair-8-5pt-p0.png`): a reflowed narrow column
renders `user_organiz / ation_roles` and `submissions. / content_flag / s`. T29c's break limits
insert an opportunity every *N* characters, so they land wherever the count falls rather than
anywhere meaningful.

Anticipated as doubt D2 in `plan-t29.md` — *"a break inserted mid-identifier is ugly"* — and now
seen rather than supposed. It is a readability cost paid by the fix that stopped tables running off
the page, so it is a trade rather than a regression.

**Upgrade, and it looks like a good one:** break at existing separators first — `_`, `.`, `/`, `-`
— and only fall back to counting when a run has none. `user_organization_roles` would wrap as
`user_ / organization_ / roles`, which is how a reader would break it anyway.

**Fixed.** Breaks now go after `_ . / - : \\ ,` first, and fall back to counting only where a run has
none. `user_ / organization_ / roles`, `submissions. / content_ / flags`. The counting fallback also
moved from `limit / 2` to `limit` — half a column's width chopped words that would have fitted.
Overflow stayed at 1 of 152, so the looser breaking cost nothing.

### F7 · `DEBT` — zero-width spaces reach the PDF text layer

T29 inserts `U+200B` into long runs in the reflow alternate, so a path or identifier copied out of a
reflowed table pastes with invisible characters in it.

Accepted: the alternative is text running off the page, and the alternate is only rendered when the
table would not otherwise fit. But someone will eventually paste a broken path and have no idea why,
so it is written down rather than discovered.

**Tracked:** unscheduled. A fix would strip breaks from the PDF's copy text while keeping them for
layout — Typst offers no obvious hook for that today.

---

## Closed

*(none yet — closed flags stay here with the reason, rather than being deleted.)*
