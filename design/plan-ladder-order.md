# Plan — the order of the rungs (T26b), and a tripwire for the ladder (T27)

**Written:** 2026-08-21 · **Follows:** T26a, which added `Reflow` and exposed the question.
**Reference:** `design/compromise-mechanism.md` — risk **R2**, and the baseline this must re-measure.

---

## The problem, stated plainly

The ladder shrinks **before** it reflows. A table needing 90% of its width is squeezed to ~9pt
rather than being reflowed at full size, and **41 elements are currently rendered at 7.0–7.5pt**.

The comparison rendered while building T26a says that preference is backwards:

- `columns: 5` shrunk to 7pt — cramped, legible only just.
- `columns: (1fr × 5)` at 10pt — full-size text, wrapped cells, clearly easier to read.

**Reflow went last because it was the conservative code change**, not because it produces the worst
outcome. That is the shape of the failure this whole document family exists to catch: every step
defensible, the sequence wrong.

## What is actually being asked

Not "is reflow good" — T26a settled that. The question is **how much shrinking is worth accepting
before wrapping is preferable**, and that is a judgment about reading, not a measurement.

---

## The options

### O1 · Reflow immediately — an alternate form always wins

```
fits -> None
has an alternate -> Reflow
otherwise -> shrink / rotate / clip
```

Tables never shrink and never rotate. Simplest rule in the codebase, and the easiest to explain to
a user: *"tables wrap, everything else scales."*

**Cost:** a table needing 98% of the width reflows rather than shrinking imperceptibly to 9.8pt.
Column proportions the author chose are discarded for a problem nobody would have seen.

### O2 · Reflow past a comfort threshold — **recommended**

```
fits                          -> None
shrink, if >= comfort floor   -> Shrink        (a small, unnoticeable adjustment)
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
you rather the table wrapped" is a question a person can look at two pages and answer.

### O3 · Reflow before rotating, after shrinking

```
fits -> shrink to floor -> Reflow -> rotate -> clip
```

Tables reflow rather than rotate; rotation becomes an image-only rung. Rotation is the most
disruptive outcome in the ladder — a landscape page interrupts reading order — so avoiding it for
anything with an alternate has real appeal.

**Cost:** still shrinks all the way to 7pt first, which is the outcome the evidence already
questions. Fixes the wrong half.

---

## Recommendation

**O2**, with the comfort floor as the tunable and `Reflow` placed before rotation as well — so a
table wraps rather than taking a landscape page of its own.

Expected effect on the baseline, to be confirmed by measurement rather than asserted:

- shrinks below the comfort floor become reflows — up to **41** of the current 163 at 7.0–7.5pt
- **84 rotations** should fall sharply, since most rotated elements are tables
- the flagged percentage should **not** move much: reflow is still a Compromise, and R1 stays open

If flagged drops a lot, something is being hidden rather than fixed — see R1.

---

## Evidence to produce before deciding

I produce these; **the call is yours** — it is a judgment about what reads well in your documents.

1. **A comparison sheet**: one real wide table rendered five ways — 9.5pt, 9pt, 8pt, 7pt, and
   reflowed at full size — on one page, labelled.
2. **The same for a hostile case**: the widest table in the corpus (582-character row, and the
   12-column case), where reflow squeezes columns hardest. This is where reflow looks *worst*, and
   it is the case that decides whether O1 is safe.
3. **Counts per option**: run the corpus under each candidate comfort floor (none / 9pt / 8pt) and
   report the resulting distribution against the baseline in `compromise-mechanism.md` §6.

Item 2 matters most: **O2's whole justification is that reflow beats deep shrinking.** If a
12-column reflow is unreadable, the answer is a *column-count* condition rather than a size one, and
the plan changes.

## Exit criteria

1. The order is decided, by you, from rendered evidence rather than reasoning.
2. `compromise-mechanism.md` §6 baseline re-measured and updated; R2 closed or re-scoped.
3. `verify.sh` green.

---

# T27 · A tripwire for the ladder

**Answering the standing question: is there a mechanism to check against the Compromise
documentation, and should the interaction be logged?**

Today: **no, and it is honour-system.** §9 of `compromise-mechanism.md` says every ladder change
re-measures the baseline. Nothing enforces it, nothing notices when it does not happen, and the
baseline is a single snapshot with no history — so a slow drift across three changes is invisible.

That is precisely the failure mode being guarded against, left unguarded.

### Why the golden hashes do not cover this

They pin the *bytes* of a rendered PDF. They would catch a ladder change — as an opaque hash
mismatch, on a fixture that happens to contain a wide table. They cannot say *"rotations went from
84 to 12"*, which is the thing worth seeing.

### What to build

**A ladder census** — golden hashes for *decisions* rather than bytes.

- **A committed fixture corpus**, ~10 documents, one per rung and edge: a table that fits, one that
  shrinks slightly, one that shrinks to the floor, one that rotates, one that reflows, a 12-column
  monster, an oversized image, a missing image, a remote image, raw HTML.
  **Committed, unlike `documents/`**, which is untracked, borrowed and may be deleted — a baseline
  that cannot be reproduced is not a baseline.
- **A test asserting the exact distribution** it produces: *N shrunk, N rotated, N reflowed, N
  clipped*. Any ladder change turns it red with a readable diff, and the failure message says what
  the numbers were and what they became.
- **A committed census file** regenerated by the same command. Its **git history is the log** —
  every ladder change leaves a diff showing exactly which kinds moved, alongside the reasoning
  already captured in `commit-log.md`. No separate log format to maintain.

### Why this is worth building rather than trusting discipline

The gate test in `invariants.md` asks: cheap now, and lossy later? **Yes to both.** The fixture
corpus and census are perhaps an hour; and information not measured at the time of a change cannot
be recovered afterwards — you cannot go back and ask what the distribution was three commits ago
unless something recorded it.

It also converts the mechanism's central risk from a rule people must remember into a build failure.

### Sequencing

**T27 before T26b.** The census is the instrument that measures whether the reordering worked;
building the instrument after the change means the change is evaluated by the thing it altered.
Build the tripwire, take the baseline, *then* move the rungs.
