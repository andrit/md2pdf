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

### F1 · `INSTRUMENT` — no overflow oracle

**Nothing in the product or the test suite can answer "did this element run off the page".**

Typst does not report overflow, so the only way to know is to render, raster, and look for ink past
the right margin. That has now been written as throwaway code **three times** — the T26a clip triage,
the T26a2 column-spec experiment, and the T26b exposure measurement — and thrown away each time.

It is the single most valuable missing instrument, because after T26b **152 of 198 compromised
elements sit on the reflow rung**, and reflow is the rung that can overflow silently while reporting
`Reflowed`. We would not know it had got worse.

**Evidence:** 4 elements in the corpus overflow today; found only by rendering. A textual proxy
(unbreakable token length) put the count at 85 — wrong by 12×.

**Tracked:** unscheduled. Natural home is a test beside `describe_the_fixtures`, plus a
`--check-overflow` mode on the CLI. **Worth building before T29**, since it is how T29 gets verified.

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

---

## Closed

*(none yet — closed flags stay here with the reason, rather than being deleted.)*
