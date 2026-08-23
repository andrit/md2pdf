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

### F3 · `CONSTRAINT` — a long-lived `Typesetter` OOMs after a few hundred renders

Rendering ~200 elements through one `Typesetter` gets the process **SIGKILLed** on this 4 GB machine.
Hit twice on 2026-08-22; fixed in the throwaway by constructing a fresh `Typesetter` per element.

**This bears directly on phase 3f.** The recompile loop's whole design rests on a long-lived `World`
so `comemo` can memoise across compilations. That is the right design for speed and it has an
unmeasured ceiling — memoisation grows and nothing evicts it. A session that recompiles a large
document repeatedly is the same shape as the loop that died here.

**Tracked:** unscheduled. **3f must measure this**, alongside the incremental-recompile timing it
already owes.

### F4 · `DEBT` — `Floors::table_pt` is now inert for any table with an alternate

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

### F8 · `DEFECT` — tables that run off the page · **1 left, from 7**

| | count | worst spill |
|---|---|---|
| before T29 | 7 of 152 | — |
| T29 · break long runs | 4 | 55pt |
| T29b · weighted `fr` columns | 4 | 9pt |
| **T29c · per-column break limits** | **1** | **2pt** |

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
