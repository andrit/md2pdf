# Product invariants — the fixed reference

**Written:** 2026-08-20 · **Stability:** this document changes rarely and deliberately.

## What this is for

"Decide against the finished product" is a good instinct and a bad daily practice: the finished
product is unbounded, so re-deriving it at every decision invites speculation and slows the build.

This is the bounded version. **The invariants below are the model of the goal.** A decision cites
one, or it is not a design decision — it is just code, and should be written the simplest way that
works.

Two documents, two jobs:

- `roadmap.md` — **sequence**: what we build, in what order. Changes constantly.
- `invariants.md` — **target**: what must be true when we are done. Changes rarely.

---

## The invariants

**INV-1 · No network, ever.**
No accounts, no backend, no telemetry, no fetching. Remote resources are skipped and recorded, not
retrieved. This is what makes running cost ~zero, which is what makes freeware or a cheap product
possible at all.

**INV-2 · md2pdf never edits markdown.**
Permanent, and a *stack* constraint rather than a product preference: egui's weakest surface is
text input, and md2pdf is safe on egui precisely because it never exercises it. Treat any editing
request as a framework decision.

**INV-3 · Output is never silently overwritten or silently lost.**
Collisions are resolved by asking. Content that cannot be represented is replaced by something
visible, never dropped quietly.

**INV-4 · Every compromise is recorded and addressable.**
Anything md2pdf decides on the user's behalf — a shrink, a rotation, a clip, a skipped image, an
unsupported construct — produces a `Compromise` that names an `Element` the user can act on. An
empty Diagnostic means the document converted cleanly, and that must be true rather than merely
likely.

**INV-5 · The gate fires where a decision was made, not on every artifact.**
Batch does not ask about fifty files; it asks about the three where something was compromised.

**INV-6 · Preview is the output.**
Fidelity is guaranteed by construction — the same compilation produces both — not by two renderers
agreeing.

**INV-7 · Identical output on every platform.**
Fonts are bundled, not resolved from the system. A document converts to the same PDF on macOS,
Windows and Linux.

**INV-8 · The engine does not know what a window is.**
Commands in, events out, as plain serializable data. **An adapter may run out-of-process**, so
anything an adapter needs must travel in the event stream — a Rust return value is invisible across
that boundary. The CLI is adapter #1 and exists to keep this honest.

**INV-9 · One door to the filesystem.**
All path access goes through `md2pdf-paths`. A future sandboxing requirement must be a change
inside that crate, not a rewrite across the tree.

**INV-10 · Only `md2pdf-typeset` links the typst crate.**
Everything crossing that boundary is a domain type. Upgrading Typst touches one crate.

**INV-11 · Templates are swappable config, discovered from a directory.** *(true since 3e)*
Not compiled in. A user-authored template is first-class; the shipped one is also the reference
example. Three roots — an explicit `--templates`, the user's config directory, then beside the
binary — merged, with the first to supply a name winning. `Template::default()` survives only as the
fallback when nothing is on disk, and `the_file_that_ships_is_the_rust_default` pins the two
together so the same document cannot render differently depending on whether a directory existed.

**INV-12 · Batch output mirrors the source tree.**
Never flattened.

**INV-13 · The pure core stays pure.**
Conversion and typesetting decisions are values in, values out. Compilation, rasterising and file
access live in the shell. The escalation ladder must be testable without a filesystem or a window.

**INV-14 · Element identity is stable by construction.**
md2pdf generates the markup, so it assigns the ids; they are never inferred back out of Typst's
tree. A persisted Override that no longer matches its content is dropped, not misapplied.

---

## The gate test

When a decision could be made simply-now or future-proof-now, **pre-build for the future only when
both hold**:

1. **It is cheap now**, and
2. **not doing it is expensive or lossy later** — information destroyed, an invariant broken, or a
   crate boundary moved.

If changing it later is a mechanical refactor the compiler will find for you, **it is not
expensive later**. Build the simple thing.

### Worked examples, from decisions already made

| Decision | Cheap now? | Lossy later? | Verdict |
|---|---|---|---|
| Failure travels as an Event (INV-8) | yes | **yes** — an out-of-process adapter never sees a `Result`, so the guarantee is broken, not deferred | **pre-build** |
| Conversion carries Compromises (INV-4) | yes | **yes** — information not emitted cannot be recovered | **pre-build** |
| `write_new` vs `overwrite` (INV-3) | yes | **yes** — a default that overwrites is a guarantee you never had | **pre-build** |
| `PathBroker` as a struct with state later (INV-9) | yes | **yes** — adding state to a unit value touches every call site | **pre-build** |
| `EventSink` trait vs a closure | yes | **no** — swapping the sink type is a mechanical refactor | **build the simple thing** |
| Correlation ids before batch exists | yes | no | **build the simple thing** |
| `#[non_exhaustive]` on the contract | yes | no — every consumer is in this repo | **build the simple thing** |

The bottom three are the ones this test exists to stop. They are all defensible in isolation, and
all of them are speculative generality.

---

## Planning discipline — measured or assumed

**Added 2026-08-21**, from a pattern across T7, T21, T26a and T27: plans keep changing mid-build,
and the changes are not all the same kind of thing.

Four classes, and only three are worth preventing:

| Class | Example | Prevent? |
|---|---|---|
| **A · Reasoned where it could have measured** | "most rotations are tables" *(it was 100%)*; fixture widths that turned out not to shrink; the clip threshold before D1 measured it | **yes** — and it is the common one |
| **B · The plan was right; the build drifted** | `plan-census.md` said the census must be a *module*; it was first written as a fourth test binary | **yes** — re-read the plan at build start |
| **C · Genuine discovery only building could produce** | Typst's escape set — 51 of 113 candidates broke; the italic font axis; table width is not linear in font size | **no — this is the work** |
| **D · A claim outrunning its evidence** | "VERIFIED" written against D1/D2 before the checks were run; "staged" said from a handover note | **yes — most serious** |

**The rule, for class A:** a plan may not assert a fact that a short measurement could settle.
Anything about Typst's behaviour or about the corpus is either measured before the plan is finished,
or **marked as an assumption in the text**. Assumptions are not forbidden; going unmarked is, because
an unmarked assumption reads exactly like a measured fact to whoever builds from it — including me,
a week later.

**Why the doubts audit did not already catch this.** The audit reviews the doubts I *noticed*. Every
class-A miss was a claim that felt like a fact at the time, so it never entered the list. The list is
self-selected, and self-selection is the failure. So the audit is now two passes:

1. **The doubts I have** — the existing ritual.
2. **A sweep of the plan's own sentences** for quantitative or behavioural claims, each one marked
   `[measured]` or `[assumed]`. Mechanical, and it does not depend on my having felt uncertain.

`plan-ladder-order.md` is the first plan written this way, and the sweep immediately contradicted two
of its own claims: rotation "for no gain" was false for 10 tables, and O1 turned out to delete a rung
rather than reorder one.

## Cost is not a reason to decline an option

**Added 2026-08-23 by the operator**, after T30 closed the fourth defect of one shape: *"do not
decline a plan based solely on cost. Surface plans that appear to be costly and lay them out with
alternatives, with a detailed description of each and a comparison lined against the actual goal. I
can catch poorly visioned plans and help decide if the cost is worth it."*

**What went wrong.** `plan-base-size.md` §"The change" listed three options for `CHARS_ACROSS` and
declined the third — *move break insertion into `md2pdf-typeset`* — with the single note *"most
correct, largest change"*. That is a cost, written by me, decided by me, in one line. T29, T29b, T29c
and T30 then each fixed the same defect: `convert` estimating a width that `typeset` could measure.
Four tasks bought what the declined option would have closed once.

**The rule.** An option may be *recommended against*, never *silently priced out*. Where a plan
weighs options:

1. **Every option gets a real description** — what it changes, what it makes impossible, what it
   costs. A one-line dismissal is not a description, and "largest change" is not an argument.
2. **Each is scored against the actual goal**, not against effort. The goal here was never "derive a
   constant"; it was *break limits that cannot be wrong*. Option 3 met that goal and option 2 only
   narrowed the gap — which the option table did not say.
3. **A recurring defect re-opens the comparison.** The second time one shape of bug is fixed, the
   option that would have prevented it is re-costed in the open, not left as settled.
4. **The operator decides.** Cost is real and may well win; it is simply not mine to weigh alone.
   Recommend, show the working, and put the choice where it belongs.

**The distinction that matters:** declining on *cost* is the operator's call. Declining on
*correctness* — this option does not achieve the goal — stays mine, and gets its reason in the text.

**What this deliberately does not do:** class C is the reason to build things. Measuring earlier does
not suppress discovery — it moves the *knowable* half before the code, leaving the genuinely
unknowable half where it belongs. If a plan stops changing entirely, that is a warning sign, not
success: it means the build stopped being allowed to teach anything.

### Class C, worked — T26b, 2026-08-22

The clearest instance so far, and the reason the class exists.

T26b was planned to reorder the ladder's rungs, on the strength of R2: *reflow reads better than deep
shrinking*. The plan was measured where it could be — the corpus triaged by element class, comfort
floors simulated, the predicted census diff written down in advance. It recommended O2.

Then the three comparison sheets were rendered and **looked at**, and the recommendation was wrong.
Not mis-tuned — wrong at the premise. Deep shrinking beat reflow on every real table, because the
reflow alternate emits `columns: (1fr, …)`, equal shares, giving a column holding "P1" the same width
as a column holding a paragraph. R2 had been founded on a *synthetic* table whose columns were all
the same width, the one shape where equal shares are right.

**Two documents were confidently wrong in the same direction** — the plan and the risk register — and
the risk register was the artefact written specifically to catch this kind of error. No amount of
further planning would have found it: every input to the reasoning was correct, and the conclusion
still did not survive contact with a rendered page.

The rule this supports is the project's oldest habit, and it keeps earning its place: **the italic
bug, the 25 clipped tables, and this all came from rendering the output and looking at it.** Text
assertions cannot see any of them. When a decision is about how something *reads*, no measurement
substitutes for the page — so produce the page, look, and let it overrule the plan.

Corollary, and the reason to write this down rather than feel good about it: **it is not a failure
of planning that the plan changed here.** The measured half was worth measuring — it is what made the
sheets buildable and what made the contradiction legible when it arrived. The plan did its job by
being specific enough to be proved wrong.

---

## How to use this

- A plan's decisions should **cite an invariant** (`INV-8`) or explicitly say "no invariant — simple
  thing." Both are fine. What is not fine is re-arguing the future from scratch each time.
- If a decision seems to need a *new* invariant, that is a real finding: raise it, do not smuggle
  it into a task plan. Invariants are the stable layer, and stability is their whole value.
- If an invariant turns out to be wrong, change it **here**, dated, and note which decisions were
  made under the old one. That has already happened twice by accident (the `ImageProbe` location,
  `clear_files`' rationale); doing it deliberately is cheaper.
