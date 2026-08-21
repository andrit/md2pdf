# Plan — the CLI adapter (phase 3d)

**Written:** 2026-08-20 · **Follows:** 3c2. The engine converts directories; nothing drives it.
**Goal:** `md2pdf <dir> -o <out>` works, and the Floors are chosen against real documents.

Decisions cite `design/invariants.md`. Where no invariant applies: **build the simple thing.**

---

## Why this phase is not a throwaway

`md2pdf-cli`'s own doc comment already says it: *"A contract with one implementation is not a
contract."* The CLI is the cheap second adapter — it runs in CI with no display, and it is what
keeps `INV-8` honest by proving the engine really does not know what a window is.

It is also the first time a person can use md2pdf at all.

---

## Split: T25 is engineering, T26 is judgment

The roadmap bundles "build the CLI" with "set the floors by eye". Those are different kinds of
work and should not share a task: one is code with tests, the other is looking at pages and
deciding what reads well. Bundling them means the judgment gets made in passing.

---

## T25 · The binary

### C1 · `pico-args`, not `clap` — and it costs nothing

`pico-args 0.5.0` is **already in the dependency graph**, pulled in transitively by two typst
crates. Declaring it directly adds no crates today.

`clap` would add roughly ten crates to spare us a hand-written help string of about fifteen lines,
for a surface of two commands and three flags. By the gate test that is exactly backwards: swapping
an argument parser later is a mechanical refactor, so paying ten dependencies now to avoid it is
speculative.

*Honest caveat:* it is in the graph because typst uses it. A future typst upgrade could drop it,
leaving us with a direct dependency we did not previously "pay for". That is acceptable — it would
still be one small crate — and it is recorded so the surprise is a known one.

### C2 · The whole binary is a composition root

`main` builds a `PathBroker`, a `Typesetter` and `Template::default()`, turns arguments into a
`Command`, and passes an `emit` closure that renders each `Event` as text or as a JSON line. That
is the entire program.

Saying so here so T25 does not grow a framework: the CLI holds **no** conversion logic, no policy,
and no state beyond what it hands the engine.

### C3 · `--json` makes the CLI the *proof* of INV-8

Default output is a human summary. `--json` emits **line-delimited `Event` JSON on stdout**.

This is more than a convenience flag. INV-8 claims an adapter could run out-of-process reading the
event stream; the contract's round-trip tests only *simulate* that. A test that runs the CLI, takes
its stdout, and parses each line back through `serde_json::from_str::<Event>` exercises the real
boundary end to end for the first time.

**In `--json` mode, stdout carries nothing but JSON.** Progress notes, warnings and errors go to
stderr. A mode that is unparseable by the consumer it exists for is worse than not having it.

### C4 · Exit codes, including the one that will surprise someone

| Situation | Exit |
|---|---|
| Everything converted, some flagged | **0** |
| One or more Sources failed | 1 |
| The Job could not start (unwalkable root, bad arguments) | 2 |

The first row is a **decision**, not an oversight: `INV-5` says flagged is not failed — a document
with a missing image converted successfully and merely needs attention. A script author could
reasonably expect otherwise, which is why it is written down.

*Not built:* `--fail-on-flagged`, for CI that wants to treat any compromise as an error. Mechanical
to add later, so by the gate test it waits for someone to want it.

### C5 · `--on-collision` defaults to `skip`

B2 forbade a default on the *Command* so that every adapter must state its intent. **The CLI
stating `skip` is that intent** — this is not a contradiction, it is an adapter making the choice
B2 required it to make.

`skip` because it is the only default that never destroys anything (`INV-3`). The summary reports
skips prominently, so "produced nothing, quietly" is not a possible outcome.

### C6 · One binary, two shapes

`md2pdf <path> -o <dir>`, dispatching on what `<path>` is: a file becomes `ConvertSource`, a
directory becomes `ConvertBatch`. Users do not think in commands, and the engine already has both.

### Tests

- A single file converts; the PDF lands where expected; exit 0.
- A directory converts with the tree mirrored; exit 0.
- **`--json` output parses back into `Event` values** — the out-of-process contract, exercised for
  real (C3).
- In `--json` mode, **every stdout line is valid JSON** and nothing else appears there.
- A failing Source gives exit 1; an unwalkable root gives exit 2.
- Flagged-but-converted gives exit **0** (C4).
- `--on-collision` maps to the right `BlanketResolution`; the default is `skip`.
- `--help` prints and exits 0.

---

## T26 · The Floors, chosen against real documents

### The corpus, and what it can and cannot settle

`/workspace/documents` holds **146 markdown files, ~37,000 lines** of real project documentation.
Censused before relying on it, rather than assumed:

| Construct | Files |
|---|---|
| Tables | **118** |
| Code fences | 111 |
| Headings | 140 |
| Task lists | 50 |
| Blockquotes | 34 |
| Footnotes | 3 |
| **Images** | **0** |

Widest table row: **582 characters**. Longest code line: **747**. That is a genuine workout for the
Atomic path — tables at that width cannot fit portrait and will exercise shrink → rotate → clip
exactly as intended.

**What it cannot settle: the image scale floor.** There are no images at all, so `floors.image_scale`
must be judged separately against constructed fixtures. Saying so here so a number does not get
picked from evidence that never touched it.

*Corpus use only.* The stack decisions rejected this directory for **knowledge** inheritance — a
different question, and unaffected. It stays untracked; **this does not resolve the standing
gitignore/delete/commit decision**, and the two should not be folded together.

### Triage — done 2026-08-21, and it changed the task

The corpus ran. **146 documents, 22 seconds, 146 valid PDFs.** 70 were flagged:

| Compromise | Count |
|---|---|
| Shrunk to floor | 163 |
| Rotated | 84 |
| **Clipped** | **25**, across 9 documents |
| Unsupported construct | 2 |

Twenty-five clipped elements means **content lost from real documents**. Triaged rather than
tuned around, and the result is that floors are the wrong thing to change first.

#### Every clipped element is a Table, and the cause is how we emit tables

Correlating each `Clip` decision back to its Element:

```
design__bounded-contexts.md   Table  natural=1365pt  avail=730pt  ratio=0.53
design__event-storm.md        Table  natural=1021pt  avail=730pt  ratio=0.71
design__event-storm.md        Table  natural=1113pt  avail=730pt  ratio=0.66
design__task-flow.md          Table  natural=1628pt  avail=730pt  ratio=0.45
```

A table needing **71%** of its natural width is being clipped. That is not a floor being too high —
it is a table that **cannot reflow at all**.

`emit` produces `#table(columns: N)`, which sizes columns to their content. Such a table has one
possible width, so the only lever the ladder has is to shrink the whole thing, and when 7pt is not
enough it clips. **GitHub does not do this** — it wraps cell text and the table always fits.

Measured on one identical table:

```
columns: 5          natural = 1339pt   ->  Landscape + Clip   (content lost)
columns: (1fr x 5)  natural =    0pt   ->  Portrait + None    (fits, no compromise)
```

#### What that implies, including the part that is not free

With fractional columns a table **cannot overflow horizontally** — it fills the width available and
cells wrap. That removes 25 clips and most of the 84 rotations outright.

It also means `measure()` returns **0pt**, because `1fr` has no natural width. So the probe learns
nothing about a table any more: tables stop being Atomic in the sense the spike established
("natural width IS required width") and become effectively Wrappable. The escalation ladder would
simply never run for them.

That is a **product decision, not a repair**, and it is yours:

| Option | Result |
|---|---|
| **A — keep `auto` columns** | Wide tables shrink, rotate, then lose content. Column widths follow content, which reads well when it fits. |
| **B — always `1fr` columns** | Nothing is ever clipped. A 12-column table squeezes to ~40pt per column and wraps hard — ugly, never lossy. |
| **C — `auto`, falling back to `1fr` when it will not fit** | Keeps content-shaped columns for tables that fit; reflows rather than clips for those that do not. More emit logic, and the fallback needs the probe's answer to reach the render pass. |

C looks right — it is what a person would do by hand — but it is more work and it puts a
measurement-dependent choice into emission. **Settle this before tuning floors**, because it decides
what the floors even govern: under B, the table floor stops mattering almost entirely.

#### A second gap the triage exposed

`Compromise` carries an `ElementId` and a kind — not what the element *was*. Finding out that all 25
clips were tables required writing a throwaway probe that re-converted each document and matched ids.

An adapter cannot do that, so the attention gate in 3f cannot yet say *"the table on page 4"* — only
*"element 37"*. Recorded here as input to 3f rather than fixed now.

### Division of labour, stated so "by eye" does not quietly become my eye

- **I produce the evidence:** run the corpus at candidate floor values, report which documents hit
  Shrink, Rotate and Clip at each, and render comparison pages via the raster→PNG route so the
  differences can actually be seen.
- **You make the call.** Whether 7pt table text is readable or merely legible is an aesthetic
  judgment about the product, and the stack decisions deferred it to exactly this moment for that
  reason.

### Then, and only then, the numbers

### Where the numbers live

Chosen values go into `Template::default()`. **3e moves them to `template.toml`**, where the design
says design tokens belong.

Recorded as an accepted temporary home so a later review reads it as a scheduled move rather than
drift: the catalogue does not exist yet, and inventing half of it here would build 3e badly instead
of early.

---

## Open question, one line

The CLI's doc comment says *"The egui adapter is a separate crate"*. Reading that as **a separate
crate in this workspace** — it contrasts with *this crate*, not this repository, and a separate
repository would undercut the workspace-wide guard story. **T24's guard shape follows from the
answer**, so confirming it here unblocks G2 for free.

## Exit criteria

1. `md2pdf <dir> -o <out>` converts the 146-document corpus.
2. `--json` output parses back into `Event` values in a test (`INV-8` proven, not asserted).
3. Whatever the corpus run breaks is fixed or recorded **before** any floor is chosen.
4. Floors and the rotate threshold are chosen from rendered evidence, by the operator, and written
   into `Template::default()` pending 3e.
5. `verify.sh` green; `/phase-audit` run or explicitly waived, as in every phase so far.
