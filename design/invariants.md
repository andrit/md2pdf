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

**INV-11 · Templates are swappable config, discovered from a directory.**
Not compiled in. A user-authored template is first-class; the shipped one is also the reference
example.

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

## How to use this

- A plan's decisions should **cite an invariant** (`INV-8`) or explicitly say "no invariant — simple
  thing." Both are fine. What is not fine is re-arguing the future from scratch each time.
- If a decision seems to need a *new* invariant, that is a real finding: raise it, do not smuggle
  it into a task plan. Invariants are the stable layer, and stability is their whole value.
- If an invariant turns out to be wrong, change it **here**, dated, and note which decisions were
  made under the old one. That has already happened twice by accident (the `ImageProbe` location,
  `clear_files`' rationale); doing it deliberately is cheaper.
